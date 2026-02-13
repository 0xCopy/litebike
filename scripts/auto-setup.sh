#!/usr/bin/env bash
# LiteBike Symmetrical Auto-Configuration
# Automatically discovers and configures both upstream (parent) and downstream (local) connections
# Mirrors upstream configs locally and exposes local services symmetrically
#
# Usage:
#   litebike-auto-setup [--mode auto|upstream|downstream|symmetrical]
#                            [--parent <url>] [--local-iface <iface>]
#                            [--prefer-gateway] [--auto-sync]

set -euo pipefail

# ============================================================================
# CONFIGURATION
# ============================================================================

LITEBIKE_VERSION="${LITEBIKE_VERSION:-1.0}"
LITEBIKE_CONFIG_DIR="${LITEBIKE_CONFIG_DIR:-/etc/litebike}"
LITEBIKE_STATE_DIR="${LITEBIKE_STATE_DIR:-/var/run/litebike}"
LITEBIKE_LOG_DIR="${LITEBIKE_LOG_DIR:-/var/log/litebike}"

MODE="${MODE:-auto}"
PARENT_URL="${PARENT_URL:-}"
LOCAL_IFACE="${LOCAL_IFACE:-}"
PREFER_GATEWAY="${PREFER_GATEWAY:-}"
AUTO_SYNC="${AUTO_SYNC:-true}"
SYNC_INTERVAL="${SYNC_INTERVAL:-60}"

# Protocol ports
HTTP_PROXY_PORT="${HTTP_PROXY_PORT:-8080}"
SOCKS5_PROXY_PORT="${SOCKS5_PROXY_PORT:-1080}"
SSDP_MULTICAST="${SSDP_MULTICAST:-239.255.255.250:1900}"

# Timeout and retry settings
DISCOVERY_TIMEOUT="${DISCOVERY_TIMEOUT:-5}"
CONNECT_TIMEOUT="${CONNECT_TIMEOUT:-3}"
MAX_RETRIES="${MAX_RETRIES:-3}"

# State tracking
STATE_FILE="${LITEBIKE_STATE_DIR}/auto-setup.state"
PARENT_STATE_FILE="${LITEBIKE_STATE_DIR}/parent.state"
LOCAL_STATE_FILE="${LITEBIKE_STATE_DIR}/local.state"

# ============================================================================
# LOGGING
# ============================================================================

log() {
    local level="$1"
    shift
    local message="$*"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    echo "[${timestamp}] [${level}] ${message}" | tee -a "${LITEBIKE_LOG_DIR}/auto-setup.log"
}

info() { log "INFO" "$@"; }
warn() { log "WARN" "$@"; }
error() { log "ERROR" "$@"; }
debug() { [[ "${DEBUG:-false}" == "true" ]] && log "DEBUG" "$@" || :; }

# ============================================================================
# UTILITY FUNCTIONS
# ============================================================================

ensure_directories() {
    mkdir -p "${LITEBIKE_CONFIG_DIR}"
    mkdir -p "${LITEBIKE_STATE_DIR}"
    mkdir -p "${LITEBIKE_LOG_DIR}"
}

detect_environment() {
    info "Detecting environment..."

    # Detect if running on Termux
    if [ -n "${TERMUX_VERSION:-}" ]; then
        export IS_TERMUX="true"
        info "Termux environment detected"
    fi

    # Detect if on GL.iNet router
    if [ -f /etc/openwrt/release ] || grep -qi "gl\.inet" /etc/banner 2>/dev/null; then
        export IS_ROUTER="true"
        info "Router environment detected (GL.iNet)"
    fi

    # Detect available interfaces
    detect_interfaces

    # Detect if LiteBike binary exists
    if ! command -v litebike &>/dev/null; then
        error "litebike binary not found in PATH"
        info "Install LiteBike: cargo install --path . litebike"
        exit 1
    fi

    local version=$(litebike --version 2>/dev/null || echo "unknown")
    info "LiteBike version: ${version}"
}

detect_interfaces() {
    info "Scanning network interfaces..."

    # Get all active interfaces
    if command -v ip &>/dev/null; then
        AVAILABLE_IFACES=$(ip -o link show | awk '/state UNKNOWN/ {next} /state UP/ {print $2}' | tr '\n' ' ')
    else
        AVAILABLE_IFACES=$(ls /sys/class/net/ 2>/dev/null | xargs -I {} basename {})
    fi

    info "Available interfaces: ${AVAILABLE_IFACES}"

    # Classify interfaces
    for iface in ${AVAILABLE_IFACES}; do
        case "${iface}" in
            eth*|br-lan|lan*)
                INTERFACE_TYPE="LAN"
                ;;
            wlan*|wlan*|wifi*)
                INTERFACE_TYPE="WIFI"
                ;;
            wwan*|wwan*|rmnet*|ccmni*|usb*)
                INTERFACE_TYPE="WAN"
                ;;
            tun*|tap*|wg*|utun*)
                INTERFACE_TYPE="VPN"
                ;;
            docker*|veth*)
                INTERFACE_TYPE="CONTAINER"
                ;;
            *)
                INTERFACE_TYPE="UNKNOWN"
                ;;
        esac

        debug "Interface ${iface}: ${INTERFACE_TYPE}"
    done
}

get_default_iface() {
    local type="$1"
    local iface=""

    case "${type}" in
        LAN)
            for iface in ${AVAILABLE_IFACES}; do
                case "${iface}" in
                    br-lan|eth0|eth1|lan0) echo "${iface}"; return ;;
                esac
            done
            # Fallback to first non-WAN interface
            for iface in ${AVAILABLE_IFACES}; do
                case "${iface}" in
                    wwan*|wwan*|rmnet*|ccmni*|tun*|tap*|wg*) continue ;;
                    *) echo "${iface}"; return ;;
                esac
            done
            ;;
        WAN)
            for iface in ${AVAILABLE_IFACES}; do
                case "${iface}" in
                    wwan*|wwan*|rmnet*|ccmni*|usb0) echo "${iface}"; return ;;
                esac
            done
            # Fallback to last interface
            for iface in ${AVAILABLE_IFACES}; do :; done
            echo "${iface}"
            ;;
        *)
            echo "${AVAILABLE_IFACES}" | head -1
            ;;
    esac
}

# ============================================================================
# DISCOVERY FUNCTIONS
# ============================================================================

discover_parent_gateway() {
    info "Searching for parent gateway (upstream)..."

    local discovered_parents=()

    # Method 1: Check configured parent URL
    if [ -n "${PARENT_URL:-}" ]; then
        info "Using configured parent: ${PARENT_URL}"
        if test_parent_connectivity "${PARENT_URL}"; then
            discovered_parents+=("${PARENT_URL}")
        fi
    fi

    # Method 2: SSDP discovery (UPnP IGD)
    info "Scanning for UPnP gateways..."
    local ssdp_parents
    ssdp_parents=$(discover_upnp_gateways)
    for parent in ${ssdp_parents}; do
        debug "Found UPnP parent: ${parent}"
        discovered_parents+=("${parent}")
    done

    # Method 3: Check OpenClaw mDNS/Bonjour
    info "Scanning for OpenClaw gateways..."
    local bonjour_parents
    bonjour_parents=$(discover_bonjour_gateways)
    for parent in ${bonjour_parents}; do
        debug "Found Bonjour parent: ${parent}"
        discovered_parents+=("${parent}")
    done

    # Method 4: Check environment variables
    local env_parent
    if env_parent=$(discover_parent_from_env); then
        debug "Found env parent: ${env_parent}"
        discovered_parents+=("${env_parent}")
    fi

    # Method 5: Check default gateway
    info "Checking default gateway..."
    local gateway_parent
    if gateway_parent=$(get_default_gateway); then
        debug "Found default gateway: ${gateway_parent}"
        discovered_parents+=("${gateway_parent}")
    fi

    # Deduplicate and return
    local unique_parents=$(printf '%s\n' "${discovered_parents[@]}" | sort -u)
    echo "${unique_parents}"
}

discover_upnp_gateways() {
    local timeout="${DISCOVERY_TIMEOUT}"

    # Use litebike's UPnP aggressive discovery if available
    if litebike 2>/dev/null | grep -q "upnp-gateway"; then
        litebike upnp-gateway 2>/dev/null &
        local upnp_pid=$!
        sleep "${timeout}"

        # Parse discovered gateways
        # litebike outputs in format: "Found UPnP gateway at http://ip:port/description.xml"
        kill ${upnp_pid} 2>/dev/null

        # This would be parsed from litebike output
        # For now, return empty
        echo ""
    else
        # Fallback: Use upnpc if available
        if command -v upnpc &>/dev/null; then
            upnpc -l 2>/dev/null | grep -oP 'desc: .*' | sed 's/desc: //g'
        fi
    fi
}

discover_bonjour_gateways() {
    # Use dns-sd or avahi-browse
    if command -v dns-sd &>/dev/null; then
        timeout "${DISCOVERY_TIMEOUT}" dns-sd -B _http._tcp,_https._tcp local. 2>/dev/null | \
            awk '/^._http/ {gsub(/[()]/, ""); print $3}'
    elif command -v avahi-browse &>/dev/null; then
        timeout "${DISCOVERY_TIMEOUT}" avahi-browse -a _http._tcp --all 2>/dev/null | \
            awk '/hostname =/ {gsub(/[";]/, ""); print "http://" $3 ":" $5}'
    fi
}

discover_parent_from_env() {
    local parent_vars=(
        "LITEBIKE_PARENT"
        "LITEBIKE_UPSTREAM"
        "LITEBIKE_GATEWAY"
        "GATEWAY_URL"
        "PARENT_PROXY"
    )

    for var in "${parent_vars[@]}"; do
        if [ -n "${!var:-}" ]; then
            echo "${!var}"
            return
        fi
    done
    return 1
}

discover_local_services() {
    info "Scanning for local services to expose..."

    local services=()

    # Method 1: Check common local services
    local common_ports=(22 80 443 3306 5432 6379 8086 27017)
    for port in "${common_ports[@]}"; do
        if command -v nc &>/dev/null; then
            if timeout 1 nc -z 127.0.0.1 "${port}" 2>/dev/null; then
                local service_name=$(guess_service_from_port "${port}")
                services+=("127.0.0.1:${port}:${service_name}")
                debug "Found local service: ${service_name} on ${port}"
            fi
        elif command -v timeout &>/dev/null; then
            if timeout 1 bash -c "echo >/dev/tcp/127.0.0.1/${port}" 2>/dev/null; then
                local service_name=$(guess_service_from_port "${port}")
                services+=("127.0.0.1:${port}:${service_name}")
                debug "Found local service: ${service_name} on ${port}"
            fi
        fi
    done

    # Method 2: Check systemd services
    if command -v systemctl &>/dev/null; then
        for service in $(systemctl list-units --type=service --state=running --no-legend 2>/dev/null | awk '{print $1}'); do
            local ports=$(systemctl show "${service}" 2>/dev/null | grep -oP 'ListenText=[^\s]+' | sed 's/ListenText=//')
            if [ -n "${ports}" ]; then
                for port in ${ports//,/ }; do
                    services+=("127.0.0.1:${port}:${service}")
                    debug "Found systemd service: ${service} on ${port}"
                done
            fi
        done
    fi

    printf '%s\n' "${services[@]}"
}

guess_service_from_port() {
    local port="$1"

    case "${port}" in
        22) echo "ssh" ;;
        80) echo "http" ;;
        443) echo "https" ;;
        3306) echo "mysql" ;;
        5432) echo "postgresql" ;;
        6379) echo "redis" ;;
        8086) echo "elasticsearch" ;;
        27017) echo "mongodb" ;;
        8080|1080) echo "litebike" ;;
        *) echo "unknown" ;;
    esac
}

# ============================================================================
# CONNECTIVITY TESTING
# ============================================================================

test_parent_connectivity() {
    local parent_url="$1"
    local protocol="${2:-both}"

    info "Testing parent connectivity: ${parent_url}"

    # Extract host and port
    local host=$(echo "${parent_url}" | sed -n 's|.*://\([^/:]*\)\(:[0-9]*\)*.*|\1|p')
    local port=$(echo "${parent_url}" | sed -n 's|.*:\([0-9]*\)/.*|\1|p')
    port="${port:-80}"

    debug "Testing host=${host} port=${port}"

    # Test TCP connection
    local test_result=false
    if command -v timeout &>/dev/null; then
        if timeout "${CONNECT_TIMEOUT}" bash -c "exec 3<>/dev/tcp/${host}/${port}" 2>/dev/null; then
            test_result=true
        fi
    elif command -v nc &>/dev/null; then
        if timeout "${CONNECT_TIMEOUT}" nc -z "${host}" "${port}" 2>/dev/null; then
            test_result=true
        fi
    fi

    if ${test_result}; then
        info "✓ Parent reachable: ${parent_url}"
        return 0
    else
        warn "✗ Parent unreachable: ${parent_url}"
        return 1
    fi
}

test_http_proxy() {
    local proxy_url="$1"
    local test_url="${2:-http://httpbin.org/ip}"

    info "Testing HTTP proxy: ${proxy_url}"

    if command -v curl &>/dev/null; then
        if curl -x "${proxy_url}" -s --connect-timeout "${CONNECT_TIMEOUT}" "${test_url}" >/dev/null; then
            info "✓ HTTP proxy working"
            return 0
        fi
    elif command -v wget &>/dev/null; then
        if wget -e "http_proxy=${proxy_url}" -q -O - --timeout="${CONNECT_TIMEOUT}" "${test_url}" 2>/dev/null; then
            info "✓ HTTP proxy working"
            return 0
        fi
    fi

    warn "✗ HTTP proxy failed"
    return 1
}

test_socks5_proxy() {
    local proxy_url="$1"
    local test_url="${2:-http://httpbin.org/ip}"

    info "Testing SOCKS5 proxy: ${proxy_url}"

    if command -v curl &>/dev/null; then
        if curl --socks5 "${proxy_url}" -s --connect-timeout "${CONNECT_TIMEOUT}" "${test_url}" >/dev/null; then
            info "✓ SOCKS5 proxy working"
            return 0
        fi
    elif command -v wget &>/dev/null; then
        # wget has limited SOCKS support, skip
        warn "wget SOCKS5 test skipped"
    fi

    warn "✗ SOCKS5 proxy failed"
    return 1
}

# ============================================================================
# STATE MANAGEMENT
# ============================================================================

save_state() {
    local state_file="$1"
    shift
    local key="$1"
    local value="$2"

    mkdir -p "$(dirname "${state_file}")"
    echo "${key}=${value}" >> "${state_file}"
    debug "Saved state: ${key}=${value}"
}

load_state() {
    local state_file="$1"

    if [ -f "${state_file}" ]; then
        source "${state_file}"
        debug "Loaded state from ${state_file}"
    fi
}

clear_state() {
    local state_file="$1"
    rm -f "${state_file}"
    debug "Cleared state: ${state_file}"
}

# ============================================================================
# CONFIGURATION GENERATION
# ============================================================================

generate_upstream_config() {
    local parent_url="$1"
    local config_file="${LITEBIKE_CONFIG_DIR}/upstream.conf"

    info "Generating upstream config: ${config_file}"

    # Parse parent URL
    local proto=$(echo "${parent_url}" | sed -n 's|\([^:]*\):.*|\1|p')
    local host=$(echo "${parent_url}" | sed -n 's|.*://\([^/:]*\)\(:[0-9]*\)\{0,1\}.*|\1|p')
    local port=$(echo "${parent_url}" | sed -n 's|.*:\([0-9]*\)/.*|\1|p')

    cat > "${config_file}" <<EOF
# LiteBike Upstream Configuration
# Generated by litebike-auto-setup
# Parent: ${parent_url}

# Upstream proxy configuration
LITEBIKE_PARENT_URL="${parent_url}"
LITEBIKE_PARENT_HOST="${host}"
LITEBIKE_PARENT_PORT="${port:-80}"

# Bind to loopback for upstream client mode
LITEBIKE_BIND_ADDR="127.0.0.1"
LITEBIKE_BIND_PORT="${HTTP_PROXY_PORT}"
LITEBIKE_SOCKS_PORT="${SOCKS5_PROXY_PORT}"

# Connect through parent for unknown protocols
LITEBIKE_DEFAULT_GATEWAY="${parent_url}"

# Enable Knox bypass if on mobile network
LITEBIKE_ENABLE_KNOX_BYPASS="true"
LITEBIKE_ENABLE_TETHERING_BYPASS="true"

# Auto-sync enabled
LITEBIKE_AUTO_SYNC="${AUTO_SYNC}"
LITEBIKE_SYNC_INTERVAL="${SYNC_INTERVAL}"
EOF

    echo "${config_file}"
}

generate_downstream_config() {
    local iface="$1"
    local config_file="${LITEBIKE_CONFIG_DIR}/downstream.conf"

    info "Generating downstream config: ${config_file}"

    # Get interface IP
    local iface_ip=$(ip addr show "${iface}" | awk '/inet / {print $2}' | cut -d/ -f1)
    if [ -z "${iface_ip}" ]; then
        iface_ip="0.0.0.0"
        warn "Could not detect IP for ${iface}, binding to all interfaces"
    fi

    cat > "${config_file}" <<EOF
# LiteBike Downstream Configuration
# Generated by litebike-auto-setup
# Interface: ${iface} (${iface_ip})

# Downstream proxy configuration
LITEBIKE_BIND_ADDR="${iface_ip}"
LITEBIKE_BIND_PORT="${HTTP_PROXY_PORT}"
LITEBIKE_SOCKS_PORT="${SOCKS5_PROXY_PORT}"

# Expose services on LAN
LITEBIKE_EXPOSE_SERVICES="true"
LITEBIKE_LAN_ONLY="true"

# Disable WAN-facing features
LITEBIKE_DISABLE_UPNP_AGGRESSIVE="true"
LITEBIKE_DISABLE_SSDP="false"

# Enable discovery for local clients
LITEBIKE_ENABLE_SSDP="true"
LITEBIKE_ENABLE_BONJOUR="true"
EOF

    echo "${config_file}"
}

generate_symmetrical_config() {
    local parent_url="$1"
    local iface="$2"
    local config_file="${LITEBIKE_CONFIG_DIR}/symmetrical.conf"

    info "Generating symmetrical config: ${config_file}"

    local iface_ip=$(ip addr show "${iface}" | awk '/inet / {print $2}' | cut -d/ -f1)

    cat > "${config_file}" <<EOF
# LiteBike Symmetrical Configuration
# Generated by litebike-auto-setup
# Parent: ${parent_url}
# Interface: ${iface} (${iface_ip})

# Symmetrical mode: act as both upstream client and downstream server
LITEBIKE_MODE="symmetrical"

# Upstream connection
LITEBIKE_PARENT_URL="${parent_url}"
LITEBIKE_UPSTREAM_AUTO_CONNECT="true"
LITEBIKE_UPSTREAM_RETRY="${MAX_RETRIES}"

# Downstream exposure
LITEBIKE_BIND_ADDR="${iface_ip}"
LITEBIKE_BIND_PORT="${HTTP_PROXY_PORT}"
LITEBIKE_SOCKS_PORT="${SOCKS5_PROXY_PORT}"

# Protocol detection and routing
LITEBIKE_PROTOCOL_DETECTION="auto"
LITEBIKE_GATE_ROUTING="enabled"
LITEBIKE_KNOX_GATE="enabled"

# Service discovery
LITEBIKE_ENABLE_SSDP="true"
LITEBIKE_ENABLE_BONJOUR="true"
LITEBIKE_ADVERTISE="true"

# Auto-sync and failover
LITEBIKE_AUTO_SYNC="${AUTO_SYNC}"
LITEBIKE_SYNC_INTERVAL="${SYNC_INTERVAL}"
LITEBIKE_FAILOVER_ENABLED="true"
LITEBIKE_HEALTH_CHECK_INTERVAL="30"
EOF

    echo "${config_file}"
}

# ============================================================================
# INTEGRATION WITH LITEBIKE
# ============================================================================

start_litebike_with_config() {
    local config_file="$1"
    local mode="$2"

    info "Starting LiteBike with config: ${config_file}"

    if [ ! -f "${config_file}" ]; then
        error "Config file not found: ${config_file}"
        return 1
    fi

    # Source the config
    export $(grep -v '^#' "${config_file}" | grep '=' | sed 's/^/export /')

    # Start LiteBike
    local litebike_cmd="litebike integrated-proxy"

    if [ "${mode}" = "upstream" ]; then
        info "Mode: Upstream client (connects to parent)"
        ${litebike_cmd} &
    elif [ "${mode}" = "downstream" ]; then
        info "Mode: Downstream server (exposes services)"
        ${litebike_cmd} &
    elif [ "${mode}" = "symmetrical" ]; then
        info "Mode: Symmetrical (upstream + downstream)"
        ${litebike_cmd} &
    else
        error "Unknown mode: ${mode}"
        return 1
    fi

    local litebike_pid=$!
    save_state "${STATE_FILE}" "LITEBIKE_PID" "${litebike_pid}"
    save_state "${STATE_FILE}" "MODE" "${mode}"
    save_state "${STATE_FILE}" "CONFIG" "${config_file}"
    save_state "${STATE_FILE}" "START_TIME" "$(date +%s)"

    info "LiteBike started (PID: ${litebike_pid})"

    # Wait and monitor
    sleep 2
    if ! kill -0 "${litebike_pid}" 2>/dev/null; then
        error "LiteBike failed to start"
        return 1
    fi

    return 0
}

# ============================================================================
# AUTO-SYNC FUNCTIONALITY
# ============================================================================

auto_sync_with_parent() {
    local parent_url="$1"

    info "Starting auto-sync with parent: ${parent_url}"

    while true; do
        # Test parent connectivity
        if test_parent_connectivity "${parent_url}"; then
            # Parent reachable, sync configs
            info "Parent reachable, syncing..."

            # Pull parent config
            sync_parent_config "${parent_url}"

            # Push local capabilities
            advertise_local_capabilities

        else
            # Parent unreachable, check failover
            warn "Parent unreachable, checking failover..."
            handle_failover "${parent_url}"
        fi

        # Wait for next sync interval
        sleep "${SYNC_INTERVAL}"
    done
}

sync_parent_config() {
    local parent_url="$1"

    info "Syncing configuration from parent..."

    # Fetch parent's litebike.json manifest
    local manifest_url="${parent_url}/litebike.json"
    if ! curl -s --connect-timeout "${CONNECT_TIMEOUT}" "${manifest_url}" > "${LITEBIKE_STATE_DIR}/parent-manifest.json"; then
        debug "Could not fetch parent manifest"
        return 1
    fi

    # Parse parent capabilities
    local parent_caps=$(jq -r '{
        http: .proxy // true,
        socks5: .socks5 // true,
        knox: .knox // true
    }' "${LITEBIKE_STATE_DIR}/parent-manifest.json" 2>/dev/null || echo "{}")

    save_state "${PARENT_STATE_FILE}" "CAPABILITIES" "${parent_caps}"
    debug "Parent capabilities: ${parent_caps}"
}

advertise_local_capabilities() {
    local manifest_file="${LITEBIKE_STATE_DIR}/local-manifest.json"

    info "Advertising local capabilities..."

    # Build capability manifest
    local local_caps=$(cat <<EOF
{
  "name": "litebike",
  "version": "${LITEBIKE_VERSION}",
  "proxy": true,
  "socks5": true,
  "knox": true,
  "http": true,
  "https": true,
  "gates": ["knox", "proxy", "crypto", "htx", "shadowsocks"],
  "ports": {
    "http": ${HTTP_PROXY_PORT},
    "socks5": ${SOCKS5_PROXY_PORT},
    "ssdp": ${SSDP_MULTICAST}
  }
}
EOF
)

    echo "${local_caps}" > "${manifest_file}"
    debug "Local capabilities: ${local_caps}"

    # Serve via HTTP if possible
    # LiteBike integrated proxy already serves this at /litebike.json
}

handle_failover() {
    local failed_parent="$1"

    warn "Failover triggered for parent: ${failed_parent}"

    # Check for alternative parents
    local alternative_parents=$(discover_parent_gateway | grep -v "${failed_parent}")

    if [ -n "${alternative_parents}" ]; then
        info "Found alternative parents, attempting failover..."

        for alt_parent in ${alternative_parents}; do
            info "Testing alternative: ${alt_parent}"
            if test_parent_connectivity "${alt_parent}"; then
                info "✓ Failover successful: ${alt_parent}"
                save_state "${PARENT_STATE_FILE}" "CURRENT" "${alt_parent}"
                return 0
            fi
        done

        error "No alternative parents reachable"
    else
        # No alternatives, enter standalone mode
        warn "No alternative parents, entering standalone mode"
        save_state "${PARENT_STATE_FILE}" "CURRENT" "standalone"
        save_state "${PARENT_STATE_FILE}" "MODE" "standalone"

        # Reconfigure as standalone downstream
        local local_iface=$(get_default_iface "LAN")
        generate_downstream_config "${local_iface}"

        # Restart LiteBike in standalone mode
        # (would need to implement restart logic)
    fi
}

# ============================================================================
# MAIN SETUP FLOW
# ============================================================================

setup_auto() {
    info "=== LiteBike Symmetrical Auto-Setup ==="
    info "Mode: ${MODE}"

    ensure_directories
    detect_environment

    # Discover parent gateway
    local parents
    parents=$(discover_parent_gateway)

    if [ -z "${parents}" ]; then
        warn "No parent gateway discovered"
        if [ "${MODE}" = "auto" ] || [ "${MODE}" = "upstream" ]; then
            warn "Cannot configure upstream without parent gateway"
            warn "Falling back to downstream-only mode"
            MODE="downstream"
        fi
    else
        local parent_count=$(echo "${parents}" | wc -l)
        info "Discovered ${parent_count} parent gateway(s)"
    fi

    # Select parent
    local selected_parent=""
    if [ -n "${PARENT_URL}" ]; then
        selected_parent="${PARENT_URL}"
    elif [ -n "${PREFER_GATEWAY}" ] && echo "${parents}" | grep -q "${PREFER_GATEWAY}"; then
        selected_parent="${PREFER_GATEWAY}"
    elif [ -n "${parents}" ]; then
        selected_parent=$(echo "${parents}" | head -1)
    fi

    if [ -n "${selected_parent}" ]; then
        info "Selected parent: ${selected_parent}"
        save_state "${PARENT_STATE_FILE}" "CURRENT" "${selected_parent}"
    fi

    # Discover local interface
    local local_iface=""
    if [ -n "${LOCAL_IFACE}" ]; then
        local_iface="${LOCAL_IFACE}"
    else
        local_iface=$(get_default_iface "LAN")
    fi

    if [ -z "${local_iface}" ]; then
        # Fallback to any available interface
        local_iface=$(echo "${AVAILABLE_IFACES}" | head -1)
    fi

    info "Selected interface: ${local_iface}"

    # Generate configuration based on mode
    local config_file=""
    case "${MODE}" in
        upstream)
            if [ -z "${selected_parent}" ]; then
                error "Upstream mode requires a parent gateway"
                exit 1
            fi
            config_file=$(generate_upstream_config "${selected_parent}")
            start_litebike_with_config "${config_file}" "upstream"
            ;;

        downstream)
            config_file=$(generate_downstream_config "${local_iface}")
            start_litebike_with_config "${config_file}" "downstream"
            ;;

        symmetrical|auto)
            if [ -n "${selected_parent}" ]; then
                config_file=$(generate_symmetrical_config "${selected_parent}" "${local_iface}")
                start_litebike_with_config "${config_file}" "symmetrical"

                # Start auto-sync if enabled
                if [ "${AUTO_SYNC}" = "true" ]; then
                    info "Starting auto-sync in background..."
                    auto_sync_with_parent "${selected_parent}" &
                    save_state "${STATE_FILE}" "SYNC_PID" "$!"
                fi
            else
                # No parent, downstream only
                config_file=$(generate_downstream_config "${local_iface}")
                start_litebike_with_config "${config_file}" "downstream"
            fi
            ;;

        *)
            error "Invalid mode: ${MODE}"
            show_usage
            exit 1
            ;;
    esac

    # Run verification
    sleep 3
    verify_setup

    info "=== Setup Complete ==="
    info "LiteBike is running in ${MODE} mode"
    info "Config: ${config_file}"
    info "Logs: ${LITEBIKE_LOG_DIR}/auto-setup.log"
}

verify_setup() {
    info "Verifying setup..."

    # Check if LiteBike is responding
    local bind_addr=$(grep 'LITEBIKE_BIND_ADDR=' "${LITEBIKE_CONFIG_DIR}"/*.conf | tail -1 | cut -d= -f2)
    local bind_port=$(grep 'LITEBIKE_BIND_PORT=' "${LITEBIKE_CONFIG_DIR}"/*.conf | tail -1 | cut -d= -f2)

    if [ -n "${bind_addr}" ] && [ -n "${bind_port}" ]; then
        local proxy_url="http://${bind_addr}:${bind_port}"

        if test_http_proxy "${proxy_url}"; then
            info "✓ HTTP proxy verified"
        else
            warn "✗ HTTP proxy verification failed"
        fi
    fi

    # Check SOCKS5
    local socks_port=$(grep 'LITEBIKE_SOCKS_PORT=' "${LITEBIKE_CONFIG_DIR}"/*.conf | tail -1 | cut -d= -f2)
    if [ -n "${socks_port}" ]; then
        local socks_url="socks5://127.0.0.1:${socks_port}"

        if test_socks5_proxy "${socks_url}"; then
            info "✓ SOCKS5 proxy verified"
        else
            warn "✗ SOCKS5 proxy verification failed"
        fi
    fi
}

show_usage() {
    cat <<EOF
LiteBike Symmetrical Auto-Setup
===============================

Automatically configures LiteBike to talk to both parent (upstream) and
local (downstream) services for near-automatic operation.

USAGE:
  litebike-auto-setup [OPTIONS]

MODES:
  --mode auto         Auto-detect best configuration (default)
  --mode upstream    Act as upstream client (connects to parent)
  --mode downstream  Act as downstream server (exposes local services)
  --mode symmetrical Act as both upstream and downstream

OPTIONS:
  --parent <url>     Parent gateway URL
  --local-iface <i>  Network interface for downstream
  --prefer-gateway   Prefer this gateway from discovery
  --auto-sync         Enable auto-sync with parent (default: true)
  --no-auto-sync      Disable auto-sync

ENVIRONMENT VARIABLES:
  LITEBIKE_CONFIG_DIR     Configuration directory (default: /etc/litebike)
  LITEBIKE_STATE_DIR      State directory (default: /var/run/litebike)
  LITEBIKE_LOG_DIR        Log directory (default: /var/log/litebike)
  LITEBIKE_PARENT         Parent gateway URL
  LITEBIKE_UPSTREAM        Upstream URL (same as PARENT)
  DEBUG                   Enable debug logging

EXAMPLES:
  # Auto-configure with discovered parent
  litebike-auto-setup

  # Use specific parent gateway
  litebike-auto-setup --parent http://192.168.1.1:8080

  # Downstream mode only
  litebike-auto-setup --mode downstream --local-iface br-lan

  # Symmetrical mode with auto-sync
  litebike-auto-setup --mode symmetrical --auto-sync

  # Disable auto-sync (manual control)
  litebike-auto-setup --no-auto-sync

SETUP MODES:
  upstream      -> LiteBike connects to parent gateway as client
                  Exposes proxy on loopback for local apps
                  Routes unknown protocols through parent

  downstream    -> LiteBike exposes services to LAN
                  Acts as gateway/proxy for local clients
                  Advertises via SSDP/Bonjour

  symmetrical   -> Combines upstream + downstream
                  LiteBike bridges LAN and parent gateway
                  Auto-syncs configuration and capabilities
                  Failover to standalone if parent unavailable

INTEGRATION:
  OpenClaw: LiteBike acts as router node
    [OpenClaw] <--mDNS--> [LiteBike] <--SSDP/UPnP--> [NAT/Internet]

  Standalone: LiteBike acts as gateway
    [LAN Clients] <--SSDP/Bonjour--> [LiteBike] --> [Internet]

For more information, see: https://docs.openclaw.ai/litebike
EOF
}

# ============================================================================
# MAIN
# ============================================================================

main() {
    # Parse arguments
    while [ $# -gt 0 ]; do
        case "$1" in
            --mode)
                MODE="$2"
                shift 2
                ;;
            --parent)
                PARENT_URL="$2"
                shift 2
                ;;
            --local-iface)
                LOCAL_IFACE="$2"
                shift 2
                ;;
            --prefer-gateway)
                PREFER_GATEWAY="$2"
                shift 1
                ;;
            --auto-sync)
                AUTO_SYNC="true"
                shift
                ;;
            --no-auto-sync)
                AUTO_SYNC="false"
                shift
                ;;
            --help|-h)
                show_usage
                exit 0
                ;;
            *)
                error "Unknown option: $1"
                show_usage
                exit 1
                ;;
        esac
    done

    # Run setup
    setup_auto
}

main "$@"
