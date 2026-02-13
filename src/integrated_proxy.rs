// Integrated Proxy Architecture - Combines all litebike components
// Channel management + Gate routing + Knox awareness + P2P subsumption

use crate::channel::{ChannelManager, ChannelType, ProxyChannel};
use crate::gates::{LitebikeGateController, GateError};
use crate::knox_proxy::KnoxProxyConfig;
use crate::rbcursive::{RBCursive, ProtocolDetection};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::net::{TcpListener, TcpStream};
use std::collections::HashMap;
use std::time::Instant;

/// Integrated proxy server combining all litebike components
pub struct IntegratedProxyServer {
    channel_manager: Arc<RwLock<ChannelManager>>,
    gate_controller: Arc<LitebikeGateController>,
    rbcursive: Arc<RBCursive>,
    config: IntegratedProxyConfig,
    start_time: Instant,
    active_connections: Arc<tokio::sync::RwLock<HashMap<String, ConnectionInfo>>>,
}

/// Integrated proxy configuration combining all component configs
#[derive(Debug, Clone)]
pub struct IntegratedProxyConfig {
    pub bind_addresses: Vec<String>,
    pub knox_config: KnoxProxyConfig,
    pub enable_p2p_subsumption: bool,
    pub enable_pattern_matching: bool,
    pub enable_gate_routing: bool,
    pub max_connections: usize,
    pub connection_timeout_seconds: u64,
}

impl Default for IntegratedProxyConfig {
    fn default() -> Self {
        Self {
            bind_addresses: vec![
                "0.0.0.0:8080".to_string(),  // HTTP proxy
                "0.0.0.0:1080".to_string(),  // SOCKS5 proxy
            ],
            knox_config: KnoxProxyConfig::default(),
            enable_p2p_subsumption: true,
            enable_pattern_matching: true,
            enable_gate_routing: true,
            max_connections: 1000,
            connection_timeout_seconds: 300,
        }
    }
}

/// Connection information for monitoring
#[derive(Debug, Clone)]
struct ConnectionInfo {
    peer_addr: std::net::SocketAddr,
    protocol: String,
    channel: String,
    gate: String,
    start_time: Instant,
    bytes_transferred: u64,
}

impl IntegratedProxyServer {
    /// Create new integrated proxy server
    pub fn new(config: IntegratedProxyConfig) -> Self {
        let mut channel_manager = ChannelManager::new();
        
        // Register proxy channel with Knox integration
        let proxy_channel = ProxyChannel::with_knox_config(config.knox_config.clone());
        channel_manager.register_channel(
            "knox_proxy".to_string(), 
            Box::new(proxy_channel)
        );
        
        let gate_controller = Arc::new(LitebikeGateController::new());
        
        // Enable Knox mode if configured
        if config.knox_config.enable_knox_bypass {
            gate_controller.enable_knox_mode();
        }
        
        Self {
            channel_manager: Arc::new(RwLock::new(channel_manager)),
            gate_controller,
            rbcursive: Arc::new(RBCursive::new()),
            config,
            start_time: Instant::now(),
            active_connections: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }
    
    /// Start the integrated proxy server
    pub async fn start(&self) -> Result<(), IntegratedProxyError> {
        println!("🚀 Starting Integrated LiteBike Proxy Server");
        println!("   📊 Configuration:");
        println!("     - Addresses: {:?}", self.config.bind_addresses);
        println!("     - Knox bypass: {}", self.config.knox_config.enable_knox_bypass);
        println!("     - Pattern matching: {}", self.config.enable_pattern_matching);
        println!("     - Gate routing: {}", self.config.enable_gate_routing);
        println!("     - P2P subsumption: {}", self.config.enable_p2p_subsumption);
        
        // Initialize channels
        self.initialize_channels().await?;
        
        // Start listeners for each bind address
        let mut listener_handles = Vec::new();
        
        for bind_addr in &self.config.bind_addresses {
            let listener = TcpListener::bind(bind_addr).await
                .map_err(|e| IntegratedProxyError::BindFailed(bind_addr.clone(), e.to_string()))?;
                
            println!("✅ Listening on {}", bind_addr);
            
            // Spawn listener task
            let handle = self.spawn_listener(listener, bind_addr.clone()).await;
            listener_handles.push(handle);
        }
        
        // Print status information
        self.print_status().await;
        
        // Wait for all listeners (they run indefinitely)
        for handle in listener_handles {
            let _ = handle.await;
        }
        
        Ok(())
    }
    
    /// Initialize proxy channels
    async fn initialize_channels(&self) -> Result<(), IntegratedProxyError> {
        let mut channel_manager = self.channel_manager.write().await;
        
        // Open Knox proxy channel
        channel_manager.open_channel("knox_proxy", ChannelType::Knox).await
            .map_err(|e| IntegratedProxyError::ChannelFailed(format!("Knox channel: {}", e)))?;
        
        println!("📡 Channels initialized successfully");
        Ok(())
    }
    
    /// Spawn listener task for a specific address
    async fn spawn_listener(&self, listener: TcpListener, bind_addr: String) -> tokio::task::JoinHandle<()> {
        let channel_manager = self.channel_manager.clone();
        let gate_controller = self.gate_controller.clone();
        let rbcursive = self.rbcursive.clone();
        let active_connections = self.active_connections.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            println!("🎧 Listener started for {}", bind_addr);
            
            while let Ok((stream, peer_addr)) = listener.accept().await {
                // Check connection limits
                let current_connections = active_connections.read().await.len();
                if current_connections >= config.max_connections {
                    println!("⚠ Connection limit reached, rejecting {}", peer_addr);
                    continue;
                }
                
                // Spawn connection handler
                let conn_id = format!("{}_{}", peer_addr, Instant::now().elapsed().as_millis());
                let handler = IntegratedConnectionHandler {
                    conn_id: conn_id.clone(),
                    stream,
                    peer_addr,
                    bind_addr: bind_addr.clone(),
                    channel_manager: channel_manager.clone(),
                    gate_controller: gate_controller.clone(),
                    rbcursive: rbcursive.clone(),
                    active_connections: active_connections.clone(),
                    config: config.clone(),
                };
                
                tokio::spawn(async move {
                    if let Err(e) = handler.handle().await {
                        println!("❌ Connection {} failed: {}", peer_addr, e);
                    }
                });
            }
        })
    }
    
    /// Print server status
    async fn print_status(&self) {
        println!("\n📈 Server Status:");
        
        // Channel status
        let channel_manager = self.channel_manager.read().await;
        let active_channels = channel_manager.list_active_channels();
        println!("   📡 Active channels: {}", active_channels.len());
        for (name, channel_type) in active_channels {
            println!("     - {} ({:?})", name, channel_type);
        }
        
        // Gate status  
        let gates = self.gate_controller.list_gates().await;
        println!("   🚪 Available gates: {}", gates.len());
        for gate in gates {
            let status = if gate.is_open { "🟢" } else { "🔴" };
            println!("     {} {} (priority: {}, children: {})", 
                status, gate.name, gate.priority, gate.children_count);
        }
        
        // Runtime info
        println!("   ⏱ Uptime: {:.1}s", self.start_time.elapsed().as_secs_f64());
        println!("");
    }
    
    /// Get server statistics
    pub async fn get_stats(&self) -> IntegratedProxyStats {
        let active_connections = self.active_connections.read().await;
        let channel_manager = self.channel_manager.read().await;
        let gates = self.gate_controller.list_gates().await;
        
        IntegratedProxyStats {
            uptime_seconds: self.start_time.elapsed().as_secs(),
            active_connections: active_connections.len(),
            active_channels: channel_manager.list_active_channels().len(),
            available_gates: gates.len(),
            knox_enabled: self.config.knox_config.enable_knox_bypass,
            pattern_matching_enabled: self.config.enable_pattern_matching,
            total_bytes_transferred: active_connections.values()
                .map(|c| c.bytes_transferred)
                .sum(),
        }
    }
}

/// Connection handler for integrated proxy
struct IntegratedConnectionHandler {
    conn_id: String,
    stream: TcpStream,
    peer_addr: std::net::SocketAddr,
    bind_addr: String,
    channel_manager: Arc<RwLock<ChannelManager>>,
    gate_controller: Arc<LitebikeGateController>,
    rbcursive: Arc<RBCursive>,
    active_connections: Arc<tokio::sync::RwLock<HashMap<String, ConnectionInfo>>>,
    config: IntegratedProxyConfig,
}

impl IntegratedConnectionHandler {
    async fn handle(mut self) -> Result<(), IntegratedProxyError> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        
        println!("🔗 New connection: {} -> {}", self.peer_addr, self.bind_addr);
        
        // Read initial data for protocol detection
        let mut buffer = vec![0u8; 4096];
        let n = self.stream.read(&mut buffer).await
            .map_err(|e| IntegratedProxyError::ConnectionFailed(format!("Read failed: {}", e)))?;
        
        if n == 0 {
            return Ok(());
        }
        
        buffer.truncate(n);
        
        // Protocol detection using RBCursive
        let protocol = if self.config.enable_pattern_matching {
            match self.rbcursive.detect_protocol(&buffer) {
                ProtocolDetection::Http(_) => "http",
                ProtocolDetection::Socks5 => "socks5",
                _ => "tcp",
            }
        } else {
            "tcp"
        };
        
        println!("🔍 Detected protocol: {} from {}", protocol, self.peer_addr);
        
        // Register connection
        let conn_info = ConnectionInfo {
            peer_addr: self.peer_addr,
            protocol: protocol.to_string(),
            channel: "knox_proxy".to_string(),
            gate: "unknown".to_string(),
            start_time: Instant::now(),
            bytes_transferred: n as u64,
        };
        
        self.active_connections.write().await.insert(self.conn_id.clone(), conn_info);
        
        // Route through gate system if enabled
        let result: Result<Vec<u8>, GateError> = if self.config.enable_gate_routing {
            self.gate_controller.route_by_protocol(protocol, &buffer, Some(self.stream)).await
        } else {
            // Direct channel processing — gate routing disabled, just acknowledge
            Ok(b"Direct channel processing complete".to_vec())
        };
        
        // Handle result
        match result {
            Ok(response) => {
                println!("✅ Connection processed: {} bytes", response.len());
                // Update connection stats
                if let Some(mut conn) = self.active_connections.write().await.get_mut(&self.conn_id) {
                    conn.bytes_transferred += response.len() as u64;
                }
            }
            Err(e) => {
                println!("❌ Connection failed: {}", e);
                return Err(IntegratedProxyError::ConnectionFailed(e.to_string()));
            }
        }
        
        // Cleanup connection
        self.active_connections.write().await.remove(&self.conn_id);
        
        Ok(())
    }
}

/// Integrated proxy statistics
#[derive(Debug, Clone)]
pub struct IntegratedProxyStats {
    pub uptime_seconds: u64,
    pub active_connections: usize,
    pub active_channels: usize,
    pub available_gates: usize,
    pub knox_enabled: bool,
    pub pattern_matching_enabled: bool,
    pub total_bytes_transferred: u64,
}

/// Integrated proxy errors
#[derive(Debug)]
pub enum IntegratedProxyError {
    BindFailed(String, String),
    ChannelFailed(String),
    ConnectionFailed(String),
    ConfigurationError(String),
}

impl std::fmt::Display for IntegratedProxyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntegratedProxyError::BindFailed(addr, reason) => 
                write!(f, "Failed to bind to {}: {}", addr, reason),
            IntegratedProxyError::ChannelFailed(reason) => 
                write!(f, "Channel error: {}", reason),
            IntegratedProxyError::ConnectionFailed(reason) => 
                write!(f, "Connection error: {}", reason),
            IntegratedProxyError::ConfigurationError(reason) => 
                write!(f, "Configuration error: {}", reason),
        }
    }
}

impl std::error::Error for IntegratedProxyError {}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn integrated_proxy_creation() {
        let config = IntegratedProxyConfig::default();
        let proxy = IntegratedProxyServer::new(config);
        
        let stats = proxy.get_stats().await;
        assert_eq!(stats.active_connections, 0);
        assert!(stats.uptime_seconds < 1);
    }
    
    #[test]
    fn integrated_config_defaults() {
        let config = IntegratedProxyConfig::default();
        
        assert_eq!(config.bind_addresses.len(), 2);
        assert!(config.enable_p2p_subsumption);
        assert!(config.enable_pattern_matching);
        assert!(config.enable_gate_routing);
    }
}