// LITERBIKE Gate System (AGPL Licensed) 
// Enhanced hierarchical gating for protocols, crypto, and Knox integration
// Integrated with ../literbike gate patterns

use std::sync::Arc;
use parking_lot::RwLock;
use async_trait::async_trait;
use tokio::net::TcpStream;

pub mod shadowsocks_gate;
pub mod crypto_gate;
pub mod htx_gate;
pub mod knox_gate;
pub mod proxy_gate;

/// Enhanced gate trait with Knox awareness and connection handling
#[async_trait]
pub trait Gate: Send + Sync {
    /// Check if gate allows passage for this data
    async fn is_open(&self, data: &[u8]) -> bool;
    
    /// Process data through gate (legacy interface)
    async fn process(&self, data: &[u8]) -> Result<Vec<u8>, String>;
    
    /// Enhanced process with connection handling
    async fn process_connection(&self, data: &[u8], _stream: Option<TcpStream>) -> Result<Vec<u8>, GateError> {
        // Default implementation delegates to legacy process method
        self.process(data).await.map_err(|e| GateError::ProcessingFailed(e))
    }
    
    /// Gate identifier
    fn name(&self) -> &str;
    
    /// Child gates
    fn children(&self) -> Vec<Arc<dyn Gate>>;
    
    /// Gate priority (higher = checked first)
    fn priority(&self) -> u8 {
        50 // Default priority
    }
    
    /// Check if gate can handle this protocol
    fn can_handle_protocol(&self, protocol: &str) -> bool {
        // Default implementation checks common protocols
        matches!(protocol, "http" | "https" | "tcp")
    }
}

/// Gate processing errors with Knox-specific error types
#[derive(Debug, Clone)]
pub enum GateError {
    ProtocolNotSupported(String),
    ProcessingFailed(String),
    ConnectionFailed(String),
    Timeout,
    Knox(String),
    Legacy(String),
}

impl std::fmt::Display for GateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GateError::ProtocolNotSupported(proto) => write!(f, "Protocol not supported: {}", proto),
            GateError::ProcessingFailed(reason) => write!(f, "Processing failed: {}", reason),
            GateError::ConnectionFailed(reason) => write!(f, "Connection failed: {}", reason),
            GateError::Timeout => write!(f, "Operation timed out"),
            GateError::Knox(reason) => write!(f, "Knox gate error: {}", reason),
            GateError::Legacy(reason) => write!(f, "Legacy error: {}", reason),
        }
    }
}

impl std::error::Error for GateError {}

/// Enhanced LITEBIKE master gate controller with Knox integration
pub struct LitebikeGateController {
    gates: Arc<RwLock<Vec<Arc<dyn Gate>>>>,
    shadowsocks_gate: Arc<shadowsocks_gate::ShadowsocksGate>,
    crypto_gate: Arc<crypto_gate::CryptoGate>,
    htx_gate: Arc<htx_gate::HTXGate>,
    knox_gate: Arc<knox_gate::KnoxGate>,
    proxy_gate: Arc<proxy_gate::ProxyGate>,
}

impl LitebikeGateController {
    pub fn new() -> Self {
        let shadowsocks_gate = Arc::new(shadowsocks_gate::ShadowsocksGate::new());
        let crypto_gate = Arc::new(crypto_gate::CryptoGate::new());
        let htx_gate = Arc::new(htx_gate::HTXGate::new());
        let knox_gate = Arc::new(knox_gate::KnoxGate::new());
        let proxy_gate = Arc::new(proxy_gate::ProxyGate::new());
        
        let mut gates: Vec<Arc<dyn Gate>> = vec![
            knox_gate.clone() as Arc<dyn Gate>,        // Highest priority for Knox environments
            proxy_gate.clone() as Arc<dyn Gate>,       // HTTP/SOCKS5 proxy handling
            shadowsocks_gate.clone() as Arc<dyn Gate>,
            crypto_gate.clone() as Arc<dyn Gate>,
            htx_gate.clone() as Arc<dyn Gate>,
        ];
        
        // Sort by priority (highest first)
        gates.sort_by(|a, b| b.priority().cmp(&a.priority()));
        
        Self {
            gates: Arc::new(RwLock::new(gates)),
            shadowsocks_gate,
            crypto_gate,
            htx_gate,
            knox_gate,
            proxy_gate,
        }
    }
    
    /// Enhanced routing with connection handling (legacy interface)
    pub async fn route(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        match self.route_with_connection(data, None).await {
            Ok(result) => Ok(result),
            Err(e) => Err(e.to_string()),
        }
    }
    
    /// Route data through appropriate gate with connection support
    pub async fn route_with_connection(&self, data: &[u8], mut stream: Option<TcpStream>) -> Result<Vec<u8>, GateError> {
        // Clone gate Arcs and drop the lock before any .await
        let gates: Vec<Arc<dyn Gate>> = self.gates.read().iter().cloned().collect();

        for gate in gates.iter() {
            if gate.is_open(data).await {
                println!("🚪 Routing through gate: {} (priority: {})", gate.name(), gate.priority());

                match gate.process_connection(data, stream.take()).await {
                    Ok(result) => return Ok(result),
                    Err(GateError::ProcessingFailed(_)) => {
                        if let Ok(result) = gate.process(data).await {
                            return Ok(result);
                        }
                    }
                    Err(e) => {
                        println!("⚠ Gate {} failed: {}", gate.name(), e);
                        continue;
                    }
                }
            }
        }

        Err(GateError::ProtocolNotSupported("No gate could process data".to_string()))
    }

    /// Route by specific protocol
    pub async fn route_by_protocol(&self, protocol: &str, data: &[u8], stream: Option<TcpStream>) -> Result<Vec<u8>, GateError> {
        let gates: Vec<Arc<dyn Gate>> = self.gates.read().iter().cloned().collect();

        for gate in gates.iter() {
            if gate.can_handle_protocol(protocol) && gate.is_open(data).await {
                println!("🎯 Protocol-specific routing: {} -> {}", protocol, gate.name());
                return gate.process_connection(data, stream).await;
            }
        }

        Err(GateError::ProtocolNotSupported(format!("No gate for protocol: {}", protocol)))
    }
    
    /// List all gates with their status
    pub async fn list_gates(&self) -> Vec<GateInfo> {
        let gates: Vec<Arc<dyn Gate>> = self.gates.read().iter().cloned().collect();
        let mut gate_info = Vec::new();

        for gate in gates.iter() {
            let test_data = b"test";
            let is_open = gate.is_open(test_data).await;
            
            gate_info.push(GateInfo {
                name: gate.name().to_string(),
                priority: gate.priority(),
                is_open,
                children_count: gate.children().len(),
            });
        }
        
        gate_info
    }
    
    /// Enable Knox mode for adverse network conditions
    pub fn enable_knox_mode(&self) {
        self.knox_gate.enable();
        println!("🔒 Knox mode enabled for adverse network conditions");
    }
    
    /// Disable Knox mode  
    pub fn disable_knox_mode(&self) {
        self.knox_gate.disable();
        println!("🔓 Knox mode disabled");
    }
    
    /// Add HTX as a downstream consumer (legacy interface)
    pub fn connect_htx_downstream(&self, htx_endpoint: String) {
        self.htx_gate.set_endpoint(htx_endpoint);
    }
    
    /// Add custom gate
    pub fn add_gate(&self, gate: Arc<dyn Gate>) {
        let mut gates = self.gates.write();
        gates.push(gate);
        
        // Re-sort by priority
        gates.sort_by(|a, b| b.priority().cmp(&a.priority()));
    }
}

/// Gate information for status reporting
#[derive(Debug, Clone)]
pub struct GateInfo {
    pub name: String,
    pub priority: u8,
    pub is_open: bool,
    pub children_count: usize,
}

impl Default for LitebikeGateController {
    fn default() -> Self {
        Self::new()
    }
}