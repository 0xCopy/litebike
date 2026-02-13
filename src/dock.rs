// Lightweight litebike docking via SSDP (UPnP Simple Service Discovery)
//
// No custom ports, no crypto.  We ride the standard SSDP multicast
// group (239.255.255.250:1900) that every LAN already passes.
//
// A litebike instance advertises itself as:
//   ST: urn:litebike:service:proxy:1
//
// A client looking for litebikes sends a standard M-SEARCH with that
// ST and collects LOCATION headers from the replies.
//
// The LOCATION points to a tiny HTTP endpoint that returns a JSON
// manifest (capabilities, ports, instance name).  That endpoint is
// the same TCP listener the proxy is already running — no extra
// server needed.  If the caller can reach the LOCATION, they can
// dock.

use std::io;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};
use log::{debug, info};

// ── Constants ───────────────────────────────────────────────────────

/// Standard SSDP multicast group and port — not ours, everyone uses it.
const SSDP_ADDR: &str = "239.255.255.250:1900";
const SSDP_MULTICAST: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);
const SSDP_PORT: u16 = 1900;

/// Litebike service type URN.  Anything doing M-SEARCH for this
/// will find us; everything else ignores it.
pub const LITEBIKE_ST: &str = "urn:litebike:service:proxy:1";

// ── Discovered peer ─────────────────────────────────────────────────

/// A litebike instance found on the local network.
#[derive(Debug, Clone)]
pub struct DockPeer {
    /// Where the peer says its service lives (from LOCATION header).
    pub location: String,
    /// The peer's self-reported name (from SERVER or X-Litebike-Name header).
    pub name: String,
    /// Source address of the SSDP response.
    pub addr: SocketAddr,
    /// Extra headers we captured (lowercased keys).
    pub headers: Vec<(String, String)>,
}

impl std::fmt::Display for DockPeer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} @ {} ({})", self.name, self.location, self.addr)
    }
}

// ── Scanner (client side) ───────────────────────────────────────────

/// Send an SSDP M-SEARCH for litebike instances and collect replies.
pub fn dock_discover(timeout: Duration) -> io::Result<Vec<DockPeer>> {
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.set_broadcast(true)?;
    sock.set_read_timeout(Some(Duration::from_millis(250)))?;

    // Join the multicast group so we can hear replies sent there.
    let _ = sock.join_multicast_v4(&SSDP_MULTICAST, &Ipv4Addr::UNSPECIFIED);

    let mx = timeout.as_secs().max(1).min(5);
    let msearch = format!(
        "M-SEARCH * HTTP/1.1\r\n\
         HOST: {}\r\n\
         MAN: \"ssdp:discover\"\r\n\
         ST: {}\r\n\
         MX: {}\r\n\
         \r\n",
        SSDP_ADDR, LITEBIKE_ST, mx,
    );

    let dst: SocketAddr = SSDP_ADDR.parse().unwrap();
    sock.send_to(msearch.as_bytes(), dst)?;
    debug!("dock: sent M-SEARCH for {}", LITEBIKE_ST);

    let mut peers = Vec::new();
    let deadline = Instant::now() + timeout;
    let mut buf = [0u8; 2048];

    while Instant::now() < deadline {
        match sock.recv_from(&mut buf) {
            Ok((n, src)) => {
                if let Ok(text) = std::str::from_utf8(&buf[..n]) {
                    if let Some(peer) = parse_ssdp_response(text, src) {
                        info!("dock: found {}", peer);
                        peers.push(peer);
                    }
                }
            }
            Err(ref e)
                if e.kind() == io::ErrorKind::WouldBlock
                    || e.kind() == io::ErrorKind::TimedOut =>
            {
                continue
            }
            Err(e) => return Err(e),
        }
    }

    Ok(peers)
}

/// Parse an SSDP response into a DockPeer if it matches our ST.
fn parse_ssdp_response(text: &str, src: SocketAddr) -> Option<DockPeer> {
    let mut location = None;
    let mut server = String::new();
    let mut name = String::new();
    let mut st_matches = false;
    let mut headers = Vec::new();

    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some((key, val)) = lower.split_once(':') {
            let key = key.trim();
            // use the original-case value
            let val_orig = line.split_once(':').map(|(_, v)| v.trim()).unwrap_or("");
            match key {
                "location" => location = Some(val_orig.to_string()),
                "st" if val.trim() == LITEBIKE_ST.to_ascii_lowercase() => {
                    st_matches = true;
                }
                "server" => server = val_orig.to_string(),
                "x-litebike-name" => name = val_orig.to_string(),
                _ => {}
            }
            headers.push((key.to_string(), val_orig.to_string()));
        }
    }

    if !st_matches {
        return None;
    }

    let location = location?;
    if name.is_empty() {
        name = server.clone();
    }
    if name.is_empty() {
        name = src.ip().to_string();
    }

    Some(DockPeer {
        location,
        name,
        addr: src,
        headers,
    })
}

// ── Responder (server / litebike side) ──────────────────────────────

/// Configuration for the dock responder.
#[derive(Debug, Clone)]
pub struct DockResponderConfig {
    /// The LOCATION URL we advertise (e.g. "http://192.168.1.42:8080/litebike.json").
    /// If empty, we build one from the local IP + service_port.
    pub location: String,
    /// Port the litebike service is actually listening on.
    pub service_port: u16,
    /// Human-readable instance name.
    pub instance_name: String,
}

impl Default for DockResponderConfig {
    fn default() -> Self {
        Self {
            location: String::new(),
            service_port: 8080,
            instance_name: "litebike".to_string(),
        }
    }
}

/// Build the SSDP response we send when someone M-SEARCHes for us.
fn build_ssdp_response(config: &DockResponderConfig, local_ip: Ipv4Addr) -> String {
    let location = if config.location.is_empty() {
        format!("http://{}:{}/litebike.json", local_ip, config.service_port)
    } else {
        config.location.clone()
    };

    format!(
        "HTTP/1.1 200 OK\r\n\
         CACHE-CONTROL: max-age=1800\r\n\
         LOCATION: {}\r\n\
         SERVER: litebike/1.0\r\n\
         ST: {}\r\n\
         USN: uuid:litebike-{}::{}\r\n\
         X-Litebike-Name: {}\r\n\
         \r\n",
        location,
        LITEBIKE_ST,
        // simple instance id from name hash
        simple_hash(&config.instance_name),
        LITEBIKE_ST,
        config.instance_name,
    )
}

/// Also build a NOTIFY ALIVE for periodic re-announce.
fn build_ssdp_notify(config: &DockResponderConfig, local_ip: Ipv4Addr) -> String {
    let location = if config.location.is_empty() {
        format!("http://{}:{}/litebike.json", local_ip, config.service_port)
    } else {
        config.location.clone()
    };

    format!(
        "NOTIFY * HTTP/1.1\r\n\
         HOST: {}\r\n\
         CACHE-CONTROL: max-age=1800\r\n\
         LOCATION: {}\r\n\
         NT: {}\r\n\
         NTS: ssdp:alive\r\n\
         SERVER: litebike/1.0\r\n\
         USN: uuid:litebike-{}::{}\r\n\
         X-Litebike-Name: {}\r\n\
         \r\n",
        SSDP_ADDR,
        location,
        LITEBIKE_ST,
        simple_hash(&config.instance_name),
        LITEBIKE_ST,
        config.instance_name,
    )
}

/// Run the dock responder.  Listens on the SSDP multicast group,
/// answers M-SEARCH requests that match our ST, and periodically
/// sends NOTIFY ssdp:alive.
///
/// Designed for `std::thread::spawn` — blocks forever.
pub fn dock_respond(config: DockResponderConfig) -> io::Result<()> {
    let local_ip = guess_local_ip();
    let bind = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), SSDP_PORT);
    let sock = UdpSocket::bind(bind)?;
    sock.set_broadcast(true)?;
    // Allow port reuse so other SSDP listeners coexist.
    // (set_reuse_address is called before bind on some platforms;
    //  we do our best here — if it fails we keep going.)
    let _ = sock.join_multicast_v4(&SSDP_MULTICAST, &Ipv4Addr::UNSPECIFIED);
    sock.set_read_timeout(Some(Duration::from_secs(30)))?;

    info!(
        "dock: responding on SSDP as \"{}\" location=http://{}:{}/litebike.json",
        config.instance_name, local_ip, config.service_port,
    );

    // Send initial NOTIFY so we're immediately visible.
    let notify = build_ssdp_notify(&config, local_ip);
    let mcast_dst: SocketAddr = SSDP_ADDR.parse().unwrap();
    let _ = sock.send_to(notify.as_bytes(), mcast_dst);

    let mut buf = [0u8; 2048];
    let mut last_notify = Instant::now();

    loop {
        // Re-announce every 60s.
        if last_notify.elapsed() > Duration::from_secs(60) {
            let notify = build_ssdp_notify(&config, local_ip);
            let _ = sock.send_to(notify.as_bytes(), mcast_dst);
            last_notify = Instant::now();
        }

        match sock.recv_from(&mut buf) {
            Ok((n, src)) => {
                if let Ok(text) = std::str::from_utf8(&buf[..n]) {
                    if is_msearch_for_us(text) {
                        debug!("dock: M-SEARCH from {}", src);
                        let response = build_ssdp_response(&config, local_ip);
                        let _ = sock.send_to(response.as_bytes(), src);
                    }
                }
            }
            Err(ref e)
                if e.kind() == io::ErrorKind::WouldBlock
                    || e.kind() == io::ErrorKind::TimedOut =>
            {
                // timeout — loop back to re-announce check
            }
            Err(e) => {
                debug!("dock: recv error: {}", e);
                // transient errors on multicast sockets are common; keep going
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// Check if an incoming SSDP packet is an M-SEARCH for our service type
/// (or the ssdp:all wildcard).
fn is_msearch_for_us(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if !lower.starts_with("m-search") {
        return false;
    }
    // Match our specific ST or the "search everything" wildcard.
    for line in lower.lines() {
        if let Some(val) = line.strip_prefix("st:") {
            let val = val.trim();
            if val == LITEBIKE_ST.to_ascii_lowercase()
                || val == "ssdp:all"
                || val == "upnp:rootdevice"
            {
                return true;
            }
        }
    }
    false
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Best-effort local IPv4 address.  Tries the syscall_net helper
/// first, falls back to 0.0.0.0.
fn guess_local_ip() -> Ipv4Addr {
    crate::syscall_net::get_default_local_ipv4().unwrap_or(Ipv4Addr::UNSPECIFIED)
}

/// Dirt-simple string hash for instance IDs — not crypto, just uniqueness.
fn simple_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325; // FNV offset basis
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3); // FNV prime
    }
    h
}

/// Build the JSON manifest that the LOCATION URL should serve.
/// Callers can embed this in their HTTP handler at `/litebike.json`.
pub fn build_manifest_json(name: &str, service_port: u16, caps: &DockCapabilities) -> String {
    format!(
        r#"{{"name":"{}","port":{},"proxy":{},"knox":{},"socks5":{},"version":"1.0"}}"#,
        name, service_port, caps.has_proxy, caps.has_knox, caps.has_socks5,
    )
}

/// Capabilities advertised in the manifest.
#[derive(Debug, Clone, Copy, Default)]
pub struct DockCapabilities {
    pub has_proxy: bool,
    pub has_knox: bool,
    pub has_socks5: bool,
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msearch_detection() {
        let msearch = format!(
            "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nST: {}\r\nMX: 3\r\n\r\n",
            LITEBIKE_ST
        );
        assert!(is_msearch_for_us(&msearch));
    }

    #[test]
    fn msearch_wildcard() {
        let msearch = "M-SEARCH * HTTP/1.1\r\nST: ssdp:all\r\nMX: 1\r\n\r\n";
        assert!(is_msearch_for_us(msearch));
    }

    #[test]
    fn msearch_wrong_st() {
        let msearch = "M-SEARCH * HTTP/1.1\r\nST: urn:schemas-upnp-org:device:something:1\r\nMX: 3\r\n\r\n";
        assert!(!is_msearch_for_us(msearch));
    }

    #[test]
    fn notify_not_msearch() {
        let notify = "NOTIFY * HTTP/1.1\r\nNT: urn:litebike:service:proxy:1\r\nNTS: ssdp:alive\r\n\r\n";
        assert!(!is_msearch_for_us(notify));
    }

    #[test]
    fn response_parse_round_trip() {
        let cfg = DockResponderConfig {
            location: String::new(),
            service_port: 9090,
            instance_name: "my-bike".to_string(),
        };
        let resp = build_ssdp_response(&cfg, Ipv4Addr::new(10, 0, 0, 5));
        let src: SocketAddr = "10.0.0.5:1900".parse().unwrap();
        let peer = parse_ssdp_response(&resp, src).unwrap();
        assert_eq!(peer.location, "http://10.0.0.5:9090/litebike.json");
        assert_eq!(peer.name, "my-bike");
    }

    #[test]
    fn manifest_json() {
        let json = build_manifest_json("test", 8080, &DockCapabilities {
            has_proxy: true,
            has_knox: false,
            has_socks5: true,
        });
        assert!(json.contains("\"proxy\":true"));
        assert!(json.contains("\"knox\":false"));
        assert!(json.contains("\"socks5\":true"));
        assert!(json.contains("\"port\":8080"));
    }

    #[test]
    fn hash_deterministic() {
        assert_eq!(simple_hash("litebike"), simple_hash("litebike"));
        assert_ne!(simple_hash("a"), simple_hash("b"));
    }
}
