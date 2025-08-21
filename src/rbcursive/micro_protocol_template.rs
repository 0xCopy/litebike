// Micro-protocol template for `rbcursive`.
// Keep imports minimal to make micro-protocol crates possible.

use crate::rbcursive::{ProtocolDetection, ProtocolDetection::*};

/// MicroProtocol trait: minimal surface area to integrate into Litebike's rbcursive.
pub trait MicroProtocol: Send + Sync {
    /// Name of the micro-protocol
    fn name(&self) -> &'static str;

    /// Detect whether the provided bytes belong to this micro-protocol
    fn detect(&self, data: &[u8]) -> bool;

    /// Parse a single message from the buffer, returning the consumed length on success
    fn parse_one(&self, data: &[u8]) -> Option<usize>;
}

/// Example: tiny HTTP-like detector implemented with zero allocations
pub struct TinyHttp;

impl MicroProtocol for TinyHttp {
    fn name(&self) -> &'static str { "tiny-http" }

    fn detect(&self, data: &[u8]) -> bool {
        data.starts_with(b"GET ") || data.starts_with(b"POST ")
    }

    fn parse_one(&self, data: &[u8]) -> Option<usize> {
        // Find CRLFCRLF
        let mut i = 0usize;
        while i + 3 < data.len() {
            if &data[i..i+4] == b"\r\n\r\n" {
                return Some(i+4);
            }
            i += 1;
        }
        None
    }
}
