// Enhanced AbstractChannelProvider with async support and error handling
// Evolved from ../literbike channel architecture

use async_trait::async_trait;
use std::fmt;
use tokio::net::TcpStream;

/// Channel error types
#[derive(Debug, Clone)]
pub enum ChannelError {
    ProviderNotFound(String),
    ConnectionFailed(String),
    InvalidConfiguration(String),
    ProtocolError(String),
    Timeout,
    Io(String),
}

impl fmt::Display for ChannelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChannelError::ProviderNotFound(name) => write!(f, "Channel provider not found: {}", name),
            ChannelError::ConnectionFailed(reason) => write!(f, "Connection failed: {}", reason),
            ChannelError::InvalidConfiguration(reason) => write!(f, "Invalid configuration: {}", reason),
            ChannelError::ProtocolError(reason) => write!(f, "Protocol error: {}", reason),
            ChannelError::Timeout => write!(f, "Operation timed out"),
            ChannelError::Io(reason) => write!(f, "I/O error: {}", reason),
        }
    }
}

impl std::error::Error for ChannelError {}

/// Channel capabilities descriptor
#[derive(Debug, Clone)]
pub struct ChannelCapabilities {
    pub supports_http: bool,
    pub supports_https: bool,
    pub supports_socks5: bool,
    pub supports_knox_bypass: bool,
    pub supports_ssh_tunneling: bool,
    pub max_concurrent_connections: usize,
    pub timeout_seconds: u64,
}

impl Default for ChannelCapabilities {
    fn default() -> Self {
        Self {
            supports_http: true,
            supports_https: true,
            supports_socks5: false,
            supports_knox_bypass: false,
            supports_ssh_tunneling: false,
            max_concurrent_connections: 100,
            timeout_seconds: 30,
        }
    }
}

/// Abstract channel provider trait for modular proxy connections
#[async_trait]
pub trait AbstractChannelProvider: Send + Sync {
    /// Open a named channel
    async fn open_channel(&self, name: &str) -> Result<bool, ChannelError>;
    
    /// Close a named channel
    async fn close_channel(&self, name: &str) -> Result<(), ChannelError>;
    
    /// Get channel capabilities
    fn get_capabilities(&self) -> ChannelCapabilities;
    
    /// Handle incoming connection through this channel
    async fn handle_connection(&self, stream: TcpStream, channel_name: &str) -> Result<(), ChannelError>;
    
    /// Check if channel is healthy/active
    async fn health_check(&self, name: &str) -> Result<bool, ChannelError>;
    
    /// Get channel statistics
    async fn get_stats(&self, name: &str) -> Result<ChannelStats, ChannelError>;
}

/// Channel statistics
#[derive(Debug, Clone)]
pub struct ChannelStats {
    pub active_connections: usize,
    pub total_connections: u64,
    pub bytes_transferred: u64,
    pub errors: u64,
    pub uptime_seconds: u64,
}

impl Default for ChannelStats {
    fn default() -> Self {
        Self {
            active_connections: 0,
            total_connections: 0,
            bytes_transferred: 0,
            errors: 0,
            uptime_seconds: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    struct DummyProvider;
    
    #[async_trait]
    impl AbstractChannelProvider for DummyProvider {
        async fn open_channel(&self, _name: &str) -> Result<bool, ChannelError> { 
            Ok(true) 
        }
        
        async fn close_channel(&self, _name: &str) -> Result<(), ChannelError> { 
            Ok(()) 
        }
        
        fn get_capabilities(&self) -> ChannelCapabilities {
            ChannelCapabilities::default()
        }
        
        async fn handle_connection(&self, _stream: TcpStream, _channel_name: &str) -> Result<(), ChannelError> {
            Ok(())
        }
        
        async fn health_check(&self, _name: &str) -> Result<bool, ChannelError> {
            Ok(true)
        }
        
        async fn get_stats(&self, _name: &str) -> Result<ChannelStats, ChannelError> {
            Ok(ChannelStats::default())
        }
    }

    #[tokio::test]
    async fn provider_works() {
        let p = DummyProvider;
        assert!(p.open_channel("test").await.unwrap());
        assert!(p.health_check("test").await.unwrap());
        assert!(p.close_channel("test").await.is_ok());
    }
}
