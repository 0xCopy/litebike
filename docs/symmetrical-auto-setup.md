# LiteBike Symmetrical Auto-Configuration

## Overview

LiteBike's **symmetrical auto-configuration** enables near-automatic operation by discovering and configuring both:

- **Parent (Upstream)** - Gateway or proxy server to route through
- **Local (Downstream)** - Services to expose on the LAN

This creates a **bidirectional, self-configuring gateway** that:

1. Discovers parent gateways automatically
2. Detects local services
3. Configures optimal mode automatically
4. Syncs configuration with parent
5. Provides failover to alternatives
6. Maintains state across restarts

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                LiteBike Symmetrical Gateway      │
├─────────────────────────────────────────────────────┤
│                                                   │
│  ┌──────────────┐        ┌──────────────┐         │
│  │   Discovery   │        │   Auto-Sync   │         │
│  │   Service    │        │   Service      │         │
│  └───┬──────────┘        └───┬──────────┘         │
│      │                         │                      │
│      ▼                         ▼                      │
│  ┌─────────┐          ┌─────────────┐             │
│  │  Parent   │          │   Local      │             │
│  │  Gateway  │          │   Services   │             │
│  └────┬─────┘          └──────┬──────┘             │
│       │                        │                      │
│       ▼                        ▼                      │
│  [Upstream Client]    +   [Downstream Server]        │
│       │                        │                      │
│       └──────────┬───────────┘                      │
│                  ▼                                  │
│         [Network Stack]                             │
└─────────────────────────────────────────────────────┘
```

## Modes of Operation

### Auto Mode (Default)

Automatically detects environment and configures optimal mode:

```bash
litebike-auto-setup --mode auto
```

**Detection Logic:**

1. **Parent Gateway Detected + Local Services Found** → Symmetrical mode
2. **Parent Gateway Detected Only** → Upstream mode
3. **Local Services Only** → Downstream mode
4. **Neither Detected** → Downstream mode (fallback)

### Upstream Mode

Connects to parent gateway as client:

```
[LAN Apps] → [LiteBike] → [Parent Gateway] → [Internet]
                  ↓
            HTTP/SOCKS proxy
```

**Use Cases:**
- Behind existing gateway/router
- Mobile network tethering bypass
- Corporate proxy integration
- OpenClaw gateway integration

### Downstream Mode

Exposes services to LAN:

```
[LAN Clients] → [LiteBike] → [Local Services]
                        ↓
                  HTTP/SOCKS/SSDP
```

**Use Cases:**
- Router deployment
- Local gateway
- Service exposure
- Development environment

### Symmetrical Mode

Combines upstream and downstream:

```
[LAN Clients] → [LiteBike] ↔ [Parent Gateway] → [Internet]
                  ↓              ↑
            Expose          Route
          local services   through parent
```

**Use Cases:**
- Bridge between networks
- Multi-homed setup
- Redundant gateway
- Extended LAN

## Integration with OpenClaw

```
[OpenClaw] ← SSDP/mDNS → [LiteBike] ← UPnP/SSDP → [Gateway]
                ↓                              ↓
          Zero-conf LAN                Auto-sync configs
```

### Auto-Sync Behavior

**Syncs Every 60 Seconds:**

1. **Fetch parent manifest** (`/litebike.json`)
   - Get capabilities
   - Get ports
   - Get features

2. **Advertise local capabilities**
   - HTTP/SOCKS proxy
   - Knox bypass
   - Gate types
   - Protocol support

3. **Update configuration**
   - Parent gateway URL
   - Port mappings
   - Feature flags

4. **Health check**
   - Test parent connectivity
   - Trigger failover if needed
   - Log state changes

### Failover Logic

**When parent becomes unreachable:**

1. **Retry** (default: 3 attempts with 3s timeout)
2. **Discover alternatives** via SSDP/Bonjour
3. **Switch to backup** parent if available
4. **Fall back** to downstream-only mode
5. **Retry primary** periodically

## Usage Examples

### Basic Auto-Configuration

```bash
# Auto-detect and configure
./scripts/auto-setup.sh

# With debug logging
DEBUG=true ./scripts/auto-setup.sh

# Specific parent
./scripts/auto-setup.sh --parent http://192.168.1.1:8080

# Specific interface
./scripts/auto-setup.sh --local-iface br-lan

# Disable auto-sync
./scripts/auto-setup.sh --no-auto-sync
```

### Integration with LiteBike

```bash
# Start symmetrical gateway
litebike symmetrical \
    --config /etc/litebike/symmetrical.conf \
    --auto-sync \
    --failover

# Or use auto-setup wrapper
./scripts/auto-setup.sh && litebike symmetrical --auto
```

### Router Installation (GL.iNet)

```bash
# Install on GL.iNet router
ssh root@192.168.1.1

# On router:
cd /root
wget https://your-server/litebike-auto-setup.sh
chmod +x litebike-auto-setup.sh

# Auto-configure for router mode
./litebike-auto-setup.sh --mode downstream --local-iface br-lan

# Enable at boot
cat >> /etc/rc.local <<EOF
/root/litebike-auto-setup.sh --mode auto &
EOF
```

### Integration with OpenClaw

```bash
# 1. Install LiteBike
cargo install --path . litebike

# 2. Auto-configure
./scripts/auto-setup.sh --prefer-gateway http://localhost:18789

# LiteBike will discover OpenClaw via Bonjour
# OpenClaw will discover LiteBike via SSDP
# Auto-sync will exchange capabilities

# 3. Verify
litebike status
# Should show: Parent: http://localhost:18789
```

## Configuration Files

### Upstream Configuration

`/etc/litebike/upstream.conf`:

```toml
# LiteBike Upstream Configuration
PARENT_URL = "http://192.168.1.1:8080"
PARENT_HOST = "192.168.1.1"
PARENT_PORT = 8080

# Connect through parent
DEFAULT_GATEWAY = "PARENT"

# Optional: Parent credentials
# PARENT_USERNAME = "user"
# PARENT_PASSWORD = "pass"
```

### Downstream Configuration

`/etc/litebike/downstream.conf`:

```toml
# LiteBike Downstream Configuration
BIND_ADDR = "192.168.1.1"
HTTP_PORT = 8080
SOCKS5_PORT = 1080

# Expose services
EXPOSE_SERVICES = true
LAN_ONLY = true

# Discovery
ENABLE_SSDP = true
ENABLE_BONJOUR = true
```

### Symmetrical Configuration

`/etc/litebike/symmetrical.conf`:

```toml
# LiteBike Symmetrical Configuration
MODE = "symmetrical"

PARENT_URL = "http://192.168.1.1:8080"
BIND_ADDR = "0.0.0.0"

AUTO_SYNC = true
SYNC_INTERVAL = 60

FAILOVER_ENABLED = true
FAILOVER_THRESHOLD = 3
```

## State Management

### State Files

`/var/run/litebike/auto-setup.state`:
```
MODE=symmetrical
PARENT_CURRENT=http://192.168.1.1:8080
PARENT_PREVIOUS=
SYNC_PID=1234
START_TIME=1707845200
```

`/var/run/litebike/parent.state`:
```
CURRENT=http://192.168.1.1:8080
CAPABILITIES={"proxy":true,"socks5":true}
REACHABILITY=reachable
LAST_SEEN=1707845200
```

`/var/run/litebike/local.state`:
```
INTERFACE=br-lan
HTTP_PORT=8080
SOCKS5_PORT=1080
SERVICES=3
```

## Troubleshooting

### Discovery Fails

```bash
# Check network
ip addr show

# Test multicast
ssbd membership 239.255.255.250

# Test parent connectivity
curl -v http://parent-ip:8080/litebike.json
```

### Auto-Sync Issues

```bash
# Check sync logs
tail -f /var/log/litebike/auto-setup.log

# Test parent manifest
curl http://parent-ip:8080/litebike.json | jq

# Verify state
cat /var/run/litebike/auto-setup.state
```

### Failover Not Triggering

```bash
# Check failover settings
grep FAILOVER /etc/litebike/symmetrical.conf

# Test parent reachability
curl --connect-timeout 3 http://parent-ip:8080

# Trigger manual failover
litebike symmetrical --failover-now
```

## Security Considerations

### Upstream Mode

- Parent gateway must be trusted
- All traffic routed through parent
- Parent sees all destinations
- Verify parent TLS certificates

### Downstream Mode

- Bind to LAN interface only
- Block WAN access to proxy ports
- Use firewall rules:
  ```bash
  iptables -I INPUT -i wwan0 -p tcp --dport 8080 -j DROP
  iptables -I INPUT -i wwan0 -p tcp --dport 1080 -j DROP
  ```

### Symmetrical Mode

- Combine upstream + downstream security
- Monitor both directions
- Rate limit connections
- Log all proxy access

## API Reference

### Script Options

`litebike-auto-setup`:
- `--mode <auto|upstream|downstream|symmetrical>`
- `--parent <url>`
- `--local-iface <iface>`
- `--prefer-gateway`
- `--auto-sync`
- `--no-auto-sync`
- `--help`

### Environment Variables

- `LITEBIKE_CONFIG_DIR` (default: `/etc/litebike`)
- `LITEBIKE_STATE_DIR` (default: `/var/run/litebike`)
- `LITEBIKE_LOG_DIR` (default: `/var/log/litebike`)
- `DEBUG` (default: `false`)
- `DISCOVERY_TIMEOUT` (default: `5`)
- `CONNECT_TIMEOUT` (default: `3`)
- `MAX_RETRIES` (default: `3`)

## See Also

- [LiteBike README](../README.md)
- [Integrated Architecture](integrated-architecture.md)
- [Gateway Configuration](../docs/gateway/configuration.md)
- [OpenClaw Integration](../docs/openclaw/integration.md)
