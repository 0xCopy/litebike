// Knox Gate for LITEBIKE - Handles Knox-aware connections and bypass
// Integrates with existing Knox proxy functionality

use super::{Gate, GateError};
use crate::knox_proxy::KnoxProxyConfig;
use crate::tethering_bypass::TetheringBypass;
use async_trait::async_trait;
use std::sync::Arc;
use parking_lot::RwLock;
use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, AsyncReadExt};

pub struct KnoxGate {
    enabled: Arc<RwLock<bool>>,
    config: Arc<RwLock<KnoxProxyConfig>>,
    tethering_bypass: Arc<RwLock<Option<TetheringBypass>>>,
}

impl KnoxGate {
    pub fn new() -> Self {
        Self {
            enabled: Arc::new(RwLock::new(false)),
            config: Arc::new(RwLock::new(KnoxProxyConfig::default())),
            tethering_bypass: Arc::new(RwLock::new(None)),
        }
    }
    
    pub fn enable(&self) {
        *self.enabled.write() = true;
        
        // Initialize tethering bypass if enabled in config
        let config = self.config.read();
        if config.enable_tethering_bypass {
            let mut bypass = TetheringBypass::new();
            if let Ok(()) = bypass.enable_bypass() {
                *self.tethering_bypass.write() = Some(bypass);
            }
        }
    }
    
    pub fn disable(&self) {
        *self.enabled.write() = false;
        
        // Disable tethering bypass
        if let Some(mut bypass) = self.tethering_bypass.write().take() {
            let _ = bypass.disable_bypass();
        }
    }
    
    pub fn set_config(&self, config: KnoxProxyConfig) {
        *self.config.write() = config;
    }
    
    /// Detect Knox-related traffic patterns
    fn detect_knox_patterns(&self, data: &[u8]) -> bool {
        if data.len() < 4 {
            return false;
        }
        
        // Check for Knox-specific patterns
        // 1. Samsung Knox headers
        if data.windows(4).any(|w| w == b"Knox") {
            return true;
        }
        
        // 2. Android system patterns
        if data.windows(7).any(|w| w == b"Android") {
            return true;
        }
        
        // 3. Carrier-specific patterns
        if self.detect_carrier_patterns(data) {
            return true;
        }
        
        // 4. Mobile network patterns (rmnet_, wlan0)
        if let Ok(data_str) = std::str::from_utf8(data) {
            if data_str.contains("rmnet_") || data_str.contains("wlan0") {
                return true;
            }
        }
        
        false
    }
    
    /// Detect carrier-specific traffic patterns
    fn detect_carrier_patterns(&self, data: &[u8]) -> bool {
        // Common carrier middleware patterns
        let carrier_patterns: &[&[u8]] = &[
            b"tether",
            b"hotspot",
            b"carrier",
            b"mobile",
            b"cellular",
        ];

        for pattern in carrier_patterns {
            if data.windows(pattern.len()).any(|w| w.eq_ignore_ascii_case(pattern)) {
                return true;
            }
        }
        
        false
    }
    
    /// Apply Knox-specific processing
    async fn process_knox_traffic(&self, data: &[u8], stream: Option<TcpStream>) -> Result<Vec<u8>, GateError> {
        // Read config values and drop guard before any .await
        let (enable_knox_bypass, enable_tethering_bypass, ttl_spoofing) = {
            let config = self.config.read();
            (config.enable_knox_bypass, config.enable_tethering_bypass, config.ttl_spoofing)
        };

        println!("🔒 Processing Knox traffic (bypass: {}, tethering: {})",
            enable_knox_bypass,
            enable_tethering_bypass
        );

        // Apply TTL spoofing if tethering bypass is enabled
        if enable_tethering_bypass {
            let has_bypass = self.tethering_bypass.read().is_some();
            if has_bypass {
                println!("📡 TTL spoofing active (TTL: {})", ttl_spoofing);
            }
        }

        // For now, pass through the data with Knox metadata
        let mut processed_data = Vec::with_capacity(data.len() + 16);

        // Add Knox processing metadata
        processed_data.extend_from_slice(b"KNOX:");
        processed_data.extend_from_slice(&(data.len() as u32).to_le_bytes());
        processed_data.extend_from_slice(data);

        // If we have a stream, we could apply additional processing
        if let Some(mut stream) = stream {
            match stream.write_all(&processed_data).await {
                Ok(()) => {
                    let mut response = vec![0u8; 4096];
                    match stream.read(&mut response).await {
                        Ok(n) if n > 0 => {
                            response.truncate(n);
                            return Ok(response);
                        }
                        _ => return Ok(processed_data),
                    }
                }
                Err(e) => return Err(GateError::Knox(format!("Stream write failed: {}", e))),
            }
        }

        Ok(processed_data)
    }
}

#[async_trait]
impl Gate for KnoxGate {
    async fn is_open(&self, data: &[u8]) -> bool {
        if !*self.enabled.read() {
            return false;
        }
        
        // Gate is open if we detect Knox/mobile patterns
        self.detect_knox_patterns(data)
    }
    
    async fn process(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        match self.process_connection(data, None).await {
            Ok(result) => Ok(result),
            Err(e) => Err(e.to_string()),
        }
    }
    
    async fn process_connection(&self, data: &[u8], stream: Option<TcpStream>) -> Result<Vec<u8>, GateError> {
        if !self.is_open(data).await {
            return Err(GateError::Knox("Knox gate is closed or no Knox patterns detected".to_string()));
        }
        
        self.process_knox_traffic(data, stream).await
    }
    
    fn name(&self) -> &str {
        "knox"
    }
    
    fn children(&self) -> Vec<Arc<dyn Gate>> {
        vec![]
    }
    
    fn priority(&self) -> u8 {
        90 // High priority for Knox environments
    }
    
    fn can_handle_protocol(&self, protocol: &str) -> bool {
        // Knox gate can handle most protocols in mobile environments
        matches!(protocol, "http" | "https" | "tcp" | "knox" | "mobile" | "android")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn knox_gate_detection() {
        let gate = KnoxGate::new();
        gate.enable();
        
        // Test Knox pattern detection
        let knox_data = b"Android Knox system";
        assert!(gate.is_open(knox_data).await);
        
        // Test carrier pattern detection
        let carrier_data = b"tether hotspot data";
        assert!(gate.is_open(carrier_data).await);
        
        // Test non-Knox data
        let regular_data = b"regular http request";
        assert!(!gate.is_open(regular_data).await);
    }
    
    #[tokio::test]
    async fn knox_gate_processing() {
        let gate = KnoxGate::new();
        gate.enable();
        
        let test_data = b"Knox test data";
        let result = gate.process_connection(test_data, None).await.unwrap();
        
        // Should have Knox metadata prepended
        assert!(result.starts_with(b"KNOX:"));
        assert!(result.len() > test_data.len());
    }
    
    #[tokio::test]
    async fn knox_gate_disabled() {
        let gate = KnoxGate::new();
        // Gate is disabled by default
        
        let knox_data = b"Android Knox system";
        assert!(!gate.is_open(knox_data).await);
        
        let result = gate.process_connection(knox_data, None).await;
        assert!(matches!(result, Err(GateError::Knox(_))));
    }
}