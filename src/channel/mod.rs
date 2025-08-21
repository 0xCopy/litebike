// Channel abstraction for modular proxy connections
// Inspired by ../literbike channel architecture

pub mod abstract_channel_provider;
pub mod proxy_channel;

pub use abstract_channel_provider::{AbstractChannelProvider, ChannelCapabilities, ChannelError};
pub use proxy_channel::{ProxyChannel, ProxyChannelConfig};

use async_trait::async_trait;
use std::collections::HashMap;
use tokio::net::TcpStream;

/// Channel type enumeration
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChannelType {
    Http,
    Socks5,
    Knox,
    Ssh,
    Quic,
    Raw,
}

/// Channel manager for coordinating multiple proxy connections
pub struct ChannelManager {
    channels: HashMap<String, Box<dyn AbstractChannelProvider>>,
    active_channels: HashMap<String, ChannelType>,
}

impl ChannelManager {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
            active_channels: HashMap::new(),
        }
    }
    
    /// Register a channel provider
    pub fn register_channel(&mut self, name: String, provider: Box<dyn AbstractChannelProvider>) {
        self.channels.insert(name, provider);
    }
    
    /// Open a channel of specified type
    pub async fn open_channel(&mut self, name: &str, channel_type: ChannelType) -> Result<bool, ChannelError> {
        if let Some(provider) = self.channels.get(name) {
            let success = provider.open_channel(name).await?;
            if success {
                self.active_channels.insert(name.to_string(), channel_type);
            }
            Ok(success)
        } else {
            Err(ChannelError::ProviderNotFound(name.to_string()))
        }
    }
    
    /// Close a channel
    pub async fn close_channel(&mut self, name: &str) -> Result<(), ChannelError> {
        if let Some(provider) = self.channels.get(name) {
            provider.close_channel(name).await?;
            self.active_channels.remove(name);
            Ok(())
        } else {
            Err(ChannelError::ProviderNotFound(name.to_string()))
        }
    }
    
    /// List active channels
    pub fn list_active_channels(&self) -> Vec<(String, ChannelType)> {
        self.active_channels.iter()
            .map(|(name, channel_type)| (name.clone(), channel_type.clone()))
            .collect()
    }
    
    /// Get channel capabilities
    pub fn get_capabilities(&self, name: &str) -> Option<ChannelCapabilities> {
        self.channels.get(name)
            .map(|provider| provider.get_capabilities())
    }
}

impl Default for ChannelManager {
    fn default() -> Self {
        Self::new()
    }
}
