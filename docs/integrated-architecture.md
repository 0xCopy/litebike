# LiteBike Integrated Proxy Architecture

This document describes the unified architecture created by integrating components from both `./litebike` and `../literbike` codebases to create a comprehensive, Knox-aware, edge-optimized proxy system.

## Architecture Overview

The integrated design combines four key architectural patterns:

1. **Channel/Adapter Pattern** - Modular connection handling
2. **Gate-Based Routing** - Hierarchical protocol processing  
3. **Knox Integration** - Mobile/adverse network awareness
4. **P2P Subsumption** - Self-contained executable design

## Core Components

### 1. Integrated Proxy Server (`IntegratedProxyServer`)

The main orchestrator that combines all subsystems:

```rust
let config = IntegratedProxyConfig::default();
let server = IntegratedProxyServer::new(config);
server.start().await?;
```

**Features:**
- Multi-address binding (HTTP + SOCKS5)
- Automatic protocol detection via RBCursive
- Knox-aware connection handling
- Real-time statistics and monitoring

### 2. Channel Architecture

**Channel Manager** coordinates multiple connection types:

```rust
pub enum ChannelType {
    Http,    // HTTP proxy connections
    Socks5,  // SOCKS5 proxy connections  
    Knox,    // Knox-aware connections
    Ssh,     // SSH tunnel connections
    Quic,    // QUIC protocol connections
    Raw,     // Raw TCP connections
}
```

**Abstract Channel Provider** enables pluggable connection handling:
- Async connection lifecycle management
- Health checking and statistics
- Knox bypass capability reporting
- Error handling with detailed error types

**Proxy Channel** provides Knox-integrated connection handling:
- Tethering bypass integration
- Mobile fingerprint detection
- Connection statistics tracking

### 3. Gate System

**Hierarchical Protocol Routing** with priority-based processing:

```rust
pub trait Gate {
    async fn is_open(&self, data: &[u8]) -> bool;
    async fn process_connection(&self, data: &[u8], stream: Option<TcpStream>) -> Result<Vec<u8>, GateError>;
    fn priority(&self) -> u8;
    fn can_handle_protocol(&self, protocol: &str) -> bool;
}
```

**Available Gates (Priority Order):**
1. **Knox Gate** (Priority: 90) - Handles Knox/mobile patterns
2. **Proxy Gate** (Priority: 80) - HTTP/SOCKS5 processing  
3. **Crypto Gate** (Priority: 50) - Cryptographic operations
4. **HTX Gate** (Priority: 50) - Betanet HTX integration
5. **Shadowsocks Gate** (Priority: 50) - Shadowsocks protocol

**Gate Controller** manages routing logic:
- Protocol-specific routing
- Fallback processing chains
- Knox mode enable/disable
- Dynamic gate addition

### 4. Knox Integration

**Knox Gate** provides mobile/adverse network handling:
- Samsung Knox pattern detection
- Carrier pattern identification  
- Tethering bypass integration
- Mobile interface awareness (rmnet_, wlan0)

**Knox Proxy Config** integrates with existing proxy:
- TTL spoofing configuration
- Packet fragmentation settings
- TCP/TLS fingerprinting
- Maximum connection limits

## Usage Patterns

### Simple Usage (LiteBike Facade)

```rust
use literbike::LiteBike;

// Default configuration
let litebike = LiteBike::new();
litebike.start().await?;
```

### Advanced Configuration

```rust
use literbike::{IntegratedProxyConfig, IntegratedProxyServer, KnoxProxyConfig};

let config = IntegratedProxyConfig {
    bind_addresses: vec![
        "0.0.0.0:8080".to_string(),
        "0.0.0.0:1080".to_string(),
    ],
    knox_config: KnoxProxyConfig {
        enable_knox_bypass: true,
        enable_tethering_bypass: true,
        ttl_spoofing: 64,
        ..Default::default()
    },
    enable_pattern_matching: true,
    enable_gate_routing: true,
    max_connections: 1000,
    ..Default::default()
};

let server = IntegratedProxyServer::new(config);
server.start().await?;
```

### Knox-Specific Usage

```rust
// Enable Knox mode for adverse network conditions
let server = IntegratedProxyServer::new(config);
server.gate_controller.enable_knox_mode();
server.start().await?;
```

## Protocol Detection Flow

1. **Connection Received** → Read initial data
2. **RBCursive Analysis** → Detect protocol type
3. **Gate Selection** → Route to appropriate gate based on:
   - Protocol type
   - Knox patterns
   - Gate priority
   - Gate availability
4. **Channel Processing** → Handle connection through selected channel
5. **Response** → Return processed data

## Knox-Aware Features

### Pattern Detection
- **Knox Headers**: "Knox", "Android" detection
- **Carrier Patterns**: "tether", "hotspot", "mobile" detection  
- **Interface Patterns**: "rmnet_", "wlan0" identification
- **Network Patterns**: Mobile-specific traffic characteristics

### Bypass Techniques
- **TTL Spoofing**: Configurable TTL values
- **Packet Fragmentation**: DPI evasion
- **TCP Fingerprinting**: Mobile device mimicry
- **DNS Override**: Bypass carrier DNS restrictions

### Mobile Optimization
- **Buffer Patterns**: Android-typical buffer sizes
- **Connection Limits**: Mobile-appropriate defaults
- **Timeout Settings**: Network-aware timeouts

## P2P Subsumption Integration

The architecture maintains compatibility with the P2P subsumption hierarchy from `../literbike`:

### Executable Self-Awareness
- Single binary contains all functionality
- Argv[0] dispatch (ifconfig, route, netstat compatibility)
- Self-bootstrap capability  
- P2P cache transfer support

### Command Subsumption
```bash
# Level 0: SysV tool emulation
ifconfig -> litebike (argv[0] = "ifconfig")
route    -> litebike (argv[0] = "route")

# Level 1: Core operations  
litebike ifconfig
litebike proxy-quick 127.0.0.1 8888

# Level 2: Knox operations
litebike knox-proxy --enable-tethering-bypass
litebike carrier-bypass
```

## Error Handling

**Hierarchical Error Types:**
- `IntegratedProxyError` - Top-level server errors
- `GateError` - Gate processing errors  
- `ChannelError` - Channel communication errors
- `Knox-specific errors` - Mobile environment errors

**Error Recovery:**
- Gate fallback chains
- Channel redundancy
- Connection retry logic
- Graceful degradation

## Monitoring and Statistics

**Real-time Statistics:**
```rust
pub struct IntegratedProxyStats {
    pub uptime_seconds: u64,
    pub active_connections: usize,
    pub active_channels: usize,
    pub available_gates: usize,
    pub knox_enabled: bool,
    pub pattern_matching_enabled: bool,
    pub total_bytes_transferred: u64,
}
```

**Gate Status:**
- Individual gate health
- Processing success rates
- Priority assignments
- Protocol handling capabilities

**Channel Metrics:**
- Connection counts
- Bytes transferred
- Error rates
- Response times

## Development Benefits

### Modular Design
- **Channel providers** can be added without core changes
- **Gates** can be developed and tested independently  
- **Protocol detection** is centralized in RBCursive
- **Knox awareness** is isolated but integrated

### Testing Strategy
- Unit tests for individual components
- Integration tests for component interaction
- Knox environment simulation
- Protocol detection validation

### Performance Optimization
- **Priority-based routing** reduces processing overhead
- **SIMD pattern matching** accelerates protocol detection
- **Connection pooling** improves resource utilization
- **Async architecture** maximizes throughput

## Future Extensions

The integrated architecture provides extension points for:

1. **Additional Gates** - Custom protocol handlers
2. **Channel Types** - New connection methods  
3. **Knox Features** - Enhanced mobile optimizations
4. **P2P Capabilities** - Distributed proxy networks
5. **Protocol Support** - New protocol detection patterns

## Migration from Separate Codebases

For users of the original separate codebases:

**From `litebike`:**
- Knox proxy functionality is preserved
- All existing configurations are supported
- Performance improvements through gate routing

**From `../literbike`:**  
- Channel architecture is enhanced with async support
- Gate system includes Knox awareness
- P2P subsumption patterns are maintained

**Combined Benefits:**
- Knox + Channel + Gate integration
- Unified configuration management
- Comprehensive error handling
- Enhanced monitoring capabilities

This integrated architecture provides the best of both codebases while creating a cohesive, Knox-aware proxy system optimized for adverse network conditions.