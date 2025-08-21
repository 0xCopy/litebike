pub mod adapters;
pub mod channel;
pub mod quic;
pub mod reactor;
pub mod rbcursive;
pub mod syscall_net;
pub mod git_sync;
pub mod tethering_bypass;
pub mod knox_proxy;
pub mod posix_sockets;
pub mod host_trust;
pub mod tcp_fingerprint;
pub mod upnp_aggressive;
pub mod config;
pub mod types;
pub mod gates;
pub mod radios;
pub mod raw_telnet;
pub mod ssh_tools;
pub mod tls_fingerprint;
pub mod universal_listener;
pub mod packet_fragment;

// Integrated proxy architecture combining all components
pub mod integrated_proxy;

// Re-export key integrated components for easy access
pub use integrated_proxy::{IntegratedProxyServer, IntegratedProxyConfig, IntegratedProxyStats};
pub use channel::{ChannelManager, ChannelType};
pub use gates::{LitebikeGateController, GateInfo};

/// LiteBike integrated proxy facade for simple usage
pub struct LiteBike {
    proxy_server: IntegratedProxyServer,
}

impl LiteBike {
    /// Create new LiteBike instance with default configuration
    pub fn new() -> Self {
        let config = IntegratedProxyConfig::default();
        Self {
            proxy_server: IntegratedProxyServer::new(config),
        }
    }
    
    /// Create new LiteBike instance with custom configuration
    pub fn with_config(config: IntegratedProxyConfig) -> Self {
        Self {
            proxy_server: IntegratedProxyServer::new(config),
        }
    }
    
    /// Start the LiteBike proxy server
    pub async fn start(self) -> Result<(), integrated_proxy::IntegratedProxyError> {
        self.proxy_server.start().await
    }
    
    /// Get proxy server statistics
    pub async fn stats(&self) -> IntegratedProxyStats {
        self.proxy_server.get_stats().await
    }
}

impl Default for LiteBike {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        // Simple smoke test to confirm crate builds and modules link
        let _ = adapters::ssh::ssh_adapter_name();
    }
}
