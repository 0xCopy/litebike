// LiteBike Symmetrical Auto-Configuration Module
// Bridges parent (upstream) and local (downstream) services
// Enables near-automatic operation with minimal manual configuration

use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::net::{IpAddr, SocketAddr, Ipv4Addr};
use tokio::net::{TcpListener, UdpSocket, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use log::{info, warn, error, debug};
use serde::{Serialize, Deserialize};

/// Symmetrical operation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymmetricalMode {
    /// Automatically detect and configure best mode
    Auto,
    /// Act as upstream client (connects to parent gateway)
    Upstream,
    /// Act as downstream server (exposes services to LAN)
    Downstream,
    /// Act as both upstream client and downstream server
    Symmetrical,
}

/// Parent gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParentGateway {
    pub url: String,
    pub host: String,
    pub port: u16,
    pub capabilities: GatewayCapabilities,
    #[serde(skip)]
    pub last_seen: Option<Instant>,
    pub connectivity_status: ConnectivityStatus,
}

/// Gateway capabilities (from litebike.json manifest)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GatewayCapabilities {
    pub proxy: bool,
    pub socks5: bool,
    pub knox: bool,
    pub http: bool,
    pub https: bool,
    pub gates: Vec<String>,
}

/// Connectivity status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectivityStatus {
    Unknown,
    Testing,
    Reachable,
    Unreachable,
    Authenticated,
    Failed,
}

/// Local service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalService {
    pub name: String,
    pub protocol: String,
    pub bind_addr: IpAddr,
    pub port: u16,
    pub enabled: bool,
    #[serde(skip)]
    pub last_activity: Option<Instant>,
}

/// Symmetrical configuration state
#[derive(Debug, Clone)]
pub struct SymmetricalConfig {
    pub mode: SymmetricalMode,
    pub parent: Option<ParentGateway>,
    pub local_services: Vec<LocalService>,
    pub auto_sync: bool,
    pub sync_interval: Duration,
    pub failover_enabled: bool,
    pub failover_threshold: u32,
}

impl Default for SymmetricalConfig {
    fn default() -> Self {
        Self {
            mode: SymmetricalMode::Auto,
            parent: None,
            local_services: Vec::new(),
            auto_sync: true,
            sync_interval: Duration::from_secs(60),
            failover_enabled: true,
            failover_threshold: 3,
        }
    }
}

/// Symmetrical gateway - manages parent and local connections
pub struct SymmetricalGateway {
    config: Arc<RwLock<SymmetricalConfig>>,
    parent_client: Option<ParentClient>,
    local_server: Option<LocalServer>,
    discovery: DiscoveryService,
    sync_task: Option<tokio::task::JoinHandle<()>>,
    start_time: Instant,
}

/// Parent gateway client (upstream connection)
struct ParentClient {
    gateway: ParentGateway,
    http_client: reqwest::Client,
    socks5_client: Option<tokio::net::TcpStream>,
    health_check_interval: Duration,
}

/// Local server (downstream exposure)
struct LocalServer {
    services: Vec<LocalService>,
    http_listener: Option<TcpListener>,
    socks5_listener: Option<TcpListener>,
    ssdp_responder: Option<tokio::task::JoinHandle<()>>,
}

/// Discovery service for finding parent gateways
struct DiscoveryService {
    ssdp_socket: Option<UdpSocket>,
    bonjour_browser: Option<tokio::task::JoinHandle<()>>,
}

impl SymmetricalGateway {
    /// Create new symmetrical gateway
    pub fn new(config: SymmetricalConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            parent_client: None,
            local_server: None,
            discovery: Self::create_discovery(),
            sync_task: None,
            start_time: Instant::now(),
        }
    }

    fn create_discovery() -> DiscoveryService {
        DiscoveryService {
            ssdp_socket: None,  // Will be initialized during start
            bonjour_browser: None,  // Will be initialized during start
        }
    }

    /// Start symmetrical gateway in specified mode
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("🚀 Starting LiteBike Symmetrical Gateway");

        let config = self.config.read().await.clone();
        info!("Mode: {:?}", config.mode);

        match config.mode {
            SymmetricalMode::Auto => {
                info!("Auto mode: detecting best configuration...");
                self.start_auto_mode().await?;
            }
            SymmetricalMode::Upstream => {
                info!("Upstream mode: connecting to parent gateway...");
                self.start_upstream_mode().await?;
            }
            SymmetricalMode::Downstream => {
                info!("Downstream mode: exposing local services...");
                self.start_downstream_mode().await?;
            }
            SymmetricalMode::Symmetrical => {
                info!("Symmetrical mode: both upstream and downstream...");
                self.start_symmetrical_mode().await?;
            }
        }

        // Start auto-sync if enabled
        if config.auto_sync {
            self.start_auto_sync().await;
        }

        Ok(())
    }

    /// Auto-detect and configure optimal mode
    async fn start_auto_mode(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Discover parent gateways
        let parents = self.discover_parent_gateways().await?;

        // Discover local services
        let services = self.discover_local_services().await?;

        // Analyze environment
        let mode = if !parents.is_empty() && !services.is_empty() {
            info!("✓ Both parent and local services detected");
            SymmetricalMode::Symmetrical
        } else if !parents.is_empty() {
            info!("✓ Parent gateway detected, no local services");
            SymmetricalMode::Upstream
        } else if !services.is_empty() {
            info!("✓ Local services detected, no parent");
            SymmetricalMode::Downstream
        } else {
            warn!("⚠ No parent or local services detected");
            SymmetricalMode::Downstream  // Default to downstream
        };

        // Update config
        {
            let mut config = self.config.write().await;
            config.mode = mode;
            if !parents.is_empty() {
                config.parent = parents.into_iter().next();
            }
            config.local_services = services;
        }

        // Start in detected mode
        match mode {
            SymmetricalMode::Symmetrical => self.start_symmetrical_mode().await,
            SymmetricalMode::Upstream => self.start_upstream_mode().await,
            SymmetricalMode::Downstream => self.start_downstream_mode().await,
            SymmetricalMode::Auto => unreachable!("Auto mode should never be the final mode"),
        }
    }

    /// Start as upstream client
    async fn start_upstream_mode(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let config = self.config.read().await.clone();

        if let Some(parent) = config.parent {
            info!("📡 Connecting to parent: {}", parent.url);

            // Create HTTP proxy client
            let proxy = reqwest::Proxy::all(&format!("http://{}:{}", parent.host, parent.port))?;
            let http_client = reqwest::Client::builder()
                .proxy(proxy)
                .timeout(Duration::from_secs(5))
                .build()?;

            self.parent_client = Some(ParentClient {
                gateway: parent,
                http_client,
                socks5_client: None,
                health_check_interval: Duration::from_secs(30),
            });

            info!("✓ Upstream mode active - routing through parent");
            Ok(())
        } else {
            warn!("No parent configured, cannot start upstream mode");
            Err("Upstream mode requires parent gateway".into())
        }
    }

    /// Start as downstream server
    async fn start_downstream_mode(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let config = self.config.read().await.clone();

        info!("🌐 Exposing local services");

        // Determine bind address
        let bind_addr: IpAddr = "0.0.0.0".parse().unwrap();
        let http_port = 8080;
        let socks5_port = 1080;

        // Start HTTP proxy listener
        let http_addr = SocketAddr::new(bind_addr, http_port);
        let http_listener = TcpListener::bind(&http_addr).await?;
        info!("✓ HTTP proxy listening on {}", http_addr);

        // Start SOCKS5 proxy listener
        let socks5_addr = SocketAddr::new(bind_addr, socks5_port);
        let socks5_listener = TcpListener::bind(&socks5_addr).await?;
        info!("✓ SOCKS5 proxy listening on {}", socks5_addr);

        self.local_server = Some(LocalServer {
            services: config.local_services,
            http_listener: Some(http_listener),
            socks5_listener: Some(socks5_listener),
            ssdp_responder: None,  // Will be started if needed
        });

        // Start SSDP responder
        let ssdp_responder = self.start_ssdp_responder().await?;
        self.local_server.as_mut().unwrap().ssdp_responder = Some(ssdp_responder);

        info!("✓ Downstream mode active - serving LAN");
        Ok(())
    }

    /// Start as symmetrical (both upstream and downstream)
    async fn start_symmetrical_mode(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Start downstream first
        self.start_downstream_mode().await?;

        // Then upstream
        self.start_upstream_mode().await?;

        info!("✓ Symmetrical mode active - bridging LAN and parent");
        Ok(())
    }

    /// Discover parent gateways via multiple methods
    async fn discover_parent_gateways(&self) -> Result<Vec<ParentGateway>, Box<dyn std::error::Error>> {
        let mut parents = Vec::new();

        // Method 1: Check configured parent
        if let Some(parent) = self.config.read().await.parent.clone() {
            info!("Testing configured parent: {}", parent.url);
            if self.test_parent(&parent).await {
                parents.push(parent);
            }
        }

        // Method 2: SSDP/UPnP discovery
        info!("Scanning for UPnP parent gateways...");
        let upnp_parents = self.discover_upnp().await?;
        parents.extend(upnp_parents);

        // Method 3: Bonjour/mDNS discovery
        info!("Scanning for Bonjour parent gateways...");
        let bonjour_parents = self.discover_bonjour().await?;
        parents.extend(bonjour_parents);

        // Method 4: Check default gateway
        if let Some(gateway_parent) = self.discover_default_gateway().await {
            info!("Found default gateway: {}", gateway_parent.url);
            parents.push(gateway_parent);
        }

        // Remove duplicates
        parents.sort_by(|a, b| a.url.cmp(&b.url));
        parents.dedup_by(|a, b| a.url == b.url);

        Ok(parents)
    }

    /// Discover UPnP gateways
    async fn discover_upnp(&self) -> Result<Vec<ParentGateway>, Box<dyn std::error::Error>> {
        let mut parents = Vec::new();

        // Bind to SSDP multicast
        let socket = UdpSocket::bind("239.255.255.250:1900").await?;
        socket.set_broadcast(true)?;

        // Send M-SEARCH for IGD
        let msearch = b"M-SEARCH * HTTP/1.1\r\n\
            HOST: 239.255.255.250:1900\r\n\
            MAN: \"ssdp:discover\"\r\n\
            MX: 3\r\n\
            ST: urn:schemas-upnp-org:device:InternetGatewayDevice:1\r\n\r\n";

        let multicast_addr: SocketAddr = "239.255.255.250:1900".parse().unwrap();
        socket.send_to(msearch, multicast_addr).await?;

        // Collect responses
        let timeout = Instant::now() + Duration::from_secs(3);
        let mut buf = [0u8; 2048];

        while Instant::now() < timeout {
            if let Ok((len, src)) = socket.recv_from(&mut buf).await {
                if let Ok(response) = std::str::from_utf8(&buf[..len]) {
                    if let Some(parent) = self.parse_upnp_response(response, src) {
                        parents.push(parent);
                    }
                }
            }
        }

        Ok(parents)
    }

    /// Discover Bonjour/mDNS services
    async fn discover_bonjour(&self) -> Result<Vec<ParentGateway>, Box<dyn std::error::Error>> {
        // This would typically use mdns-sd or similar
        // For now, return empty (requires external crate)
        warn!("Bonjour discovery requires mdns-sd crate");
        Ok(Vec::new())
    }

    /// Discover default gateway
    async fn discover_default_gateway(&self) -> Option<ParentGateway> {
        // Get default gateway from routing table
        if let Ok(gateway_ip) = self.get_default_gateway() {
            Some(ParentGateway {
                url: format!("http://{}:8080", gateway_ip),
                host: gateway_ip.to_string(),
                port: 8080,
                capabilities: GatewayCapabilities::default(),
                last_seen: None,
                connectivity_status: ConnectivityStatus::Unknown,
            })
        } else {
            None
        }
    }

    /// Get default gateway IP
    fn get_default_gateway(&self) -> Result<Ipv4Addr, Box<dyn std::error::Error>> {
        // Read /proc/net/route or use equivalent
        // This is a simplified implementation
        Ok("192.168.1.1".parse()?)  // Placeholder
    }

    /// Parse UPnP response
    fn parse_upnp_response(&self, response: &str, src: SocketAddr) -> Option<ParentGateway> {
        let mut location = None;
        let mut server = String::new();

        for line in response.lines() {
            if line.to_lowercase().starts_with("location:") {
                location = Some(line[9..].trim().to_string());
            } else if line.to_lowercase().starts_with("server:") {
                server = line[7..].trim().to_string();
            }
        }

        location.map(|loc| ParentGateway {
            url: loc.clone(),
            host: loc.replace("http://", "").replace(":/litebike.json", "")
                .split(':').next().unwrap_or("unknown").to_string(),
            port: 8080,
            capabilities: GatewayCapabilities::default(),
            last_seen: None,
            connectivity_status: ConnectivityStatus::Unknown,
        })
    }

    /// Discover local services to expose
    async fn discover_local_services(&self) -> Result<Vec<LocalService>, Box<dyn std::error::Error>> {
        let mut services = Vec::new();

        // Check common local services
        let common_ports = [(22, "ssh"), (3306, "mysql"), (5432, "postgresql")];

        for (port, proto) in common_ports {
            if let Ok(addr) = format!("127.0.0.1:{}", port).parse() {
                if self.test_local_service(addr).await {
                    services.push(LocalService {
                        name: format!("local-{}", proto),
                        protocol: proto.to_string(),
                        bind_addr: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                        port,
                        enabled: true,
                        last_activity: None,
                    });
                }
        }
        }

        Ok(services)
    }

    /// Test if parent gateway is reachable
    async fn test_parent(&self, parent: &ParentGateway) -> bool {
        info!("Testing parent: {}", parent.url);

        // Try HTTP connection
        let timeout = Duration::from_secs(3);
        if let Ok(client) = reqwest::Client::builder()
            .timeout(timeout)
            .build()
        {
            if let Ok(_) = client.get(&format!("{}/litebike.json", parent.url))
                .timeout(Duration::from_secs(2))
                .send()
                .await
            {
                info!("✓ Parent reachable: {}", parent.url);
                return true;
            }
        }

        false
    }

    /// Test if local service is available
    async fn test_local_service(&self, addr: SocketAddr) -> bool {
        if let Ok(_) = tokio::time::timeout(
            Duration::from_millis(500),
            TcpStream::connect(addr)
        ).await
        {
            true
        } else {
            false
        }
    }

    /// Start SSDP responder for downstream discovery
    async fn start_ssdp_responder(&self) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>> {
        // This would implement SSDP NOTIFY/RESPONDER
        // For now, return a dummy task
        let handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });

        Ok(handle)
    }

    /// Start auto-sync with parent
    async fn start_auto_sync(&mut self) {
        let config = self.config.read().await.clone();
        let interval = config.sync_interval;

        let config = self.config.clone();
        self.sync_task = Some(tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;

                // Sync with parent
                if let Some(ref parent) = config.read().await.parent {
                    info!("🔄 Syncing with parent: {}", parent.url);

                    // Fetch parent manifest
                    let client = reqwest::Client::new();
                    if let Ok(response) = client.get(&format!("{}/litebike.json", parent.url))
                        .timeout(Duration::from_secs(5))
                        .send()
                        .await
                    {
                        if let Ok(text) = response.text().await {
                            if let Ok(manifest) = serde_json::from_str::<GatewayCapabilities>(&text) {
                                info!("✓ Parent capabilities: {:?}", manifest);
                            }
                        }
                    }
                }

                // Health check
                if config.read().await.failover_enabled {
                    // Check connectivity and trigger failover if needed
                    // This would implement the failover logic
                }
            }
        }));
    }

    /// Get statistics
    pub async fn stats(&self) -> SymmetricalStats {
        let config = self.config.read().await.clone();
        SymmetricalStats {
            mode: config.mode,
            uptime: self.start_time.elapsed(),
            parent_connected: self.parent_client.is_some(),
            local_services: config.local_services.len(),
            last_sync: None,
        }
    }
}

/// Statistics for symmetrical gateway
#[derive(Debug, Clone)]
pub struct SymmetricalStats {
    pub mode: SymmetricalMode,
    pub uptime: Duration,
    pub parent_connected: bool,
    pub local_services: usize,
    pub last_sync: Option<Instant>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symmetrical_mode() {
        let mode = SymmetricalMode::Auto;
        assert_eq!(mode, SymmetricalMode::Auto);
    }

    #[test]
    fn test_config_default() {
        let config = SymmetricalConfig::default();
        assert_eq!(config.mode, SymmetricalMode::Auto);
        assert_eq!(config.auto_sync, true);
    }
}
