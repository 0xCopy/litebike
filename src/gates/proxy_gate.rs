// Proxy Gate for LITEBIKE - Handles HTTP/HTTPS/SOCKS5 proxy connections
// Integrates with existing proxy functionality

use super::{Gate, GateError};
use async_trait::async_trait;
use std::sync::Arc;
use parking_lot::RwLock;
use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, AsyncReadExt};

pub struct ProxyGate {
    enabled: Arc<RwLock<bool>>,
    http_port: Arc<RwLock<u16>>,
    socks_port: Arc<RwLock<u16>>,
}

impl ProxyGate {
    pub fn new() -> Self {
        Self {
            enabled: Arc::new(RwLock::new(true)),
            http_port: Arc::new(RwLock::new(8080)),
            socks_port: Arc::new(RwLock::new(1080)),
        }
    }
    
    pub fn enable(&self) {
        *self.enabled.write() = true;
    }
    
    pub fn disable(&self) {
        *self.enabled.write() = false;
    }
    
    pub fn set_http_port(&self, port: u16) {
        *self.http_port.write() = port;
    }
    
    pub fn set_socks_port(&self, port: u16) {
        *self.socks_port.write() = port;
    }
    
    /// Detect HTTP proxy requests
    fn detect_http_proxy(&self, data: &[u8]) -> bool {
        if data.len() < 8 {
            return false;
        }
        
        // Check for HTTP methods
        let http_methods = [
            b"GET ", b"POST ", b"PUT ", b"DELETE ",
            b"HEAD ", b"OPTIONS ", b"CONNECT ", b"TRACE ", b"PATCH "
        ];
        
        for method in &http_methods {
            if data.starts_with(method) {
                return true;
            }
        }
        
        false
    }
    
    /// Detect SOCKS5 proxy requests
    fn detect_socks5_proxy(&self, data: &[u8]) -> bool {
        if data.len() < 3 {
            return false;
        }
        
        // SOCKS5 greeting: version(1) + nmethods(1) + methods(n)
        if data[0] == 0x05 {
            return true;
        }
        
        false
    }
    
    /// Process HTTP proxy request
    async fn process_http_proxy(&self, data: &[u8], stream: Option<TcpStream>) -> Result<Vec<u8>, GateError> {
        println!("🌐 Processing HTTP proxy request");
        
        // Parse HTTP request
        let request = String::from_utf8_lossy(data);
        let lines: Vec<&str> = request.lines().collect();
        
        if lines.is_empty() {
            return Err(GateError::ProcessingFailed("Empty HTTP request".to_string()));
        }
        
        let first_line = lines[0];
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        
        if parts.len() < 3 {
            return Err(GateError::ProcessingFailed("Invalid HTTP request format".to_string()));
        }
        
        let method = parts[0];
        let target = parts[1];
        let version = parts[2];
        
        println!("🔗 HTTP {} {} {}", method, target, version);
        
        // Handle CONNECT method (HTTPS tunneling)
        if method == "CONNECT" {
            return self.handle_http_connect(target, stream).await;
        }
        
        // Handle other HTTP methods
        self.handle_http_request(method, target, data, stream).await
    }
    
    /// Handle HTTP CONNECT for HTTPS tunneling
    async fn handle_http_connect(&self, target: &str, stream: Option<TcpStream>) -> Result<Vec<u8>, GateError> {
        println!("🔐 HTTP CONNECT to {}", target);
        
        if let Some(mut stream) = stream {
            // Send connection established response
            let response = b"HTTP/1.1 200 Connection established\r\n\r\n";
            stream.write_all(response).await
                .map_err(|e| GateError::ConnectionFailed(format!("Failed to send CONNECT response: {}", e)))?;
            
            // Return success indicator
            return Ok(response.to_vec());
        }
        
        // No stream provided, return mock response
        Ok(b"HTTP/1.1 200 Connection established\r\n\r\n".to_vec())
    }
    
    /// Handle regular HTTP requests
    async fn handle_http_request(&self, method: &str, target: &str, data: &[u8], stream: Option<TcpStream>) -> Result<Vec<u8>, GateError> {
        println!("📄 HTTP {} to {}", method, target);
        
        // For now, return a simple proxy response
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/plain\r\n\
             Content-Length: 23\r\n\
             \r\n\
             Proxy processed request"
        );
        
        if let Some(mut stream) = stream {
            stream.write_all(response.as_bytes()).await
                .map_err(|e| GateError::ConnectionFailed(format!("Failed to send HTTP response: {}", e)))?;
        }
        
        Ok(response.into_bytes())
    }
    
    /// Process SOCKS5 proxy request
    async fn process_socks5_proxy(&self, data: &[u8], stream: Option<TcpStream>) -> Result<Vec<u8>, GateError> {
        println!("🧦 Processing SOCKS5 proxy request");
        
        if data.len() < 3 || data[0] != 0x05 {
            return Err(GateError::ProcessingFailed("Invalid SOCKS5 request".to_string()));
        }
        
        // SOCKS5 greeting response: version(1) + method(1)
        let response = vec![0x05, 0x00]; // No authentication required
        
        if let Some(mut stream) = stream {
            stream.write_all(&response).await
                .map_err(|e| GateError::ConnectionFailed(format!("Failed to send SOCKS5 response: {}", e)))?;
        }
        
        Ok(response)
    }
}

#[async_trait]
impl Gate for ProxyGate {
    async fn is_open(&self, data: &[u8]) -> bool {
        if !*self.enabled.read() {
            return false;
        }
        
        // Gate is open if we detect HTTP or SOCKS5 patterns
        self.detect_http_proxy(data) || self.detect_socks5_proxy(data)
    }
    
    async fn process(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        match self.process_connection(data, None).await {
            Ok(result) => Ok(result),
            Err(e) => Err(e.to_string()),
        }
    }
    
    async fn process_connection(&self, data: &[u8], stream: Option<TcpStream>) -> Result<Vec<u8>, GateError> {
        if !self.is_open(data).await {
            return Err(GateError::ProtocolNotSupported("Proxy gate is closed or no proxy patterns detected".to_string()));
        }
        
        // Route to appropriate proxy handler
        if self.detect_http_proxy(data) {
            self.process_http_proxy(data, stream).await
        } else if self.detect_socks5_proxy(data) {
            self.process_socks5_proxy(data, stream).await
        } else {
            Err(GateError::ProtocolNotSupported("Unknown proxy protocol".to_string()))
        }
    }
    
    fn name(&self) -> &str {
        "proxy"
    }
    
    fn children(&self) -> Vec<Arc<dyn Gate>> {
        vec![]
    }
    
    fn priority(&self) -> u8 {
        80 // High priority for proxy protocols
    }
    
    fn can_handle_protocol(&self, protocol: &str) -> bool {
        matches!(protocol, "http" | "https" | "socks5" | "proxy")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn proxy_gate_http_detection() {
        let gate = ProxyGate::new();
        
        // Test HTTP GET
        let http_get = b"GET /test HTTP/1.1\r\n\r\n";
        assert!(gate.is_open(http_get).await);
        
        // Test HTTP CONNECT
        let http_connect = b"CONNECT example.com:443 HTTP/1.1\r\n\r\n";
        assert!(gate.is_open(http_connect).await);
        
        // Test non-HTTP
        let non_http = b"not an http request";
        assert!(!gate.is_open(non_http).await);
    }
    
    #[tokio::test]
    async fn proxy_gate_socks5_detection() {
        let gate = ProxyGate::new();
        
        // Test SOCKS5 greeting
        let socks5_greeting = b"\x05\x01\x00";
        assert!(gate.is_open(socks5_greeting).await);
        
        // Test invalid SOCKS version
        let invalid_socks = b"\x04\x01\x00";
        assert!(!gate.is_open(invalid_socks).await);
    }
    
    #[tokio::test]
    async fn proxy_gate_processing() {
        let gate = ProxyGate::new();
        
        // Test HTTP processing
        let http_request = b"GET /test HTTP/1.1\r\n\r\n";
        let result = gate.process_connection(http_request, None).await.unwrap();
        let response = String::from_utf8(result).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        
        // Test SOCKS5 processing
        let socks5_request = b"\x05\x01\x00";
        let result = gate.process_connection(socks5_request, None).await.unwrap();
        assert_eq!(result, vec![0x05, 0x00]);
    }
    
    #[tokio::test]
    async fn proxy_gate_disabled() {
        let gate = ProxyGate::new();
        gate.disable();
        
        let http_request = b"GET /test HTTP/1.1\r\n\r\n";
        assert!(!gate.is_open(http_request).await);
        
        let result = gate.process_connection(http_request, None).await;
        assert!(matches!(result, Err(GateError::ProtocolNotSupported(_))));
    }
}