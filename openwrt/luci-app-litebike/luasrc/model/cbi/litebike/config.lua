-- LiteBike Configuration Page
-- LuCI CBI model for settings

local m, s, o

m = Map("litebike", translate("LiteBike Proxy Server"),
    translate("LiteBike is an integrated proxy server with protocol detection, " ..
              "gate routing, and Knox bypass capabilities."))

s = m:section(TypedSection, "litebike", translate("General Settings"))
s.anonymous = true
s.addremove = false

-- Enable/Disable
o = s:option(Flag, "enabled", translate("Enable"))
o.rmempty = false
o.default = "1"

-- Bind Address
o = s:option(Value, "bind", translate("Bind Address"))
o.default = "0.0.0.0"
o.datatype = "ipaddr"
o.rmempty = false

-- Port
o = s:option(Value, "port", translate("Port"))
o.default = "8888"
o.datatype = "port"
o.rmempty = false

-- Max Connections
o = s:option(Value, "max_connections", translate("Max Connections"))
o.default = "1000"
o.datatype = "uinteger"
o.rmempty = false

-- Connection Timeout
o = s:option(Value, "timeout", translate("Connection Timeout (seconds)"))
o.default = "300"
o.datatype = "uinteger"
o.rmempty = false

s = m:section(TypedSection, "litebike", translate("Advanced Settings"))
s.anonymous = true

-- Knox Bypass Mode
o = s:option(Flag, "knox", translate("Knox Bypass Mode"))
o.description = translate("Enable Knox bypass for restrictive network environments")
o.default = "0"

-- P2P Subsumption
o = s:option(Flag, "p2p", translate("P2P Subsumption"))
o.description = translate("Enable P2P network subsumption")
o.default = "1"

-- Pattern Matching
o = s:option(Flag, "patterns", translate("Pattern Matching"))
o.description = translate("Enable SIMD-accelerated pattern matching")
o.default = "1"

-- Gate Routing
o = s:option(Flag, "gates", translate("Gate Routing"))
o.description = translate("Enable protocol-aware gate routing")
o.default = "1"

s = m:section(TypedSection, "litebike", translate("Protocol Detection"))
s.anonymous = true

-- HTTP
o = s:option(Flag, "proto_http", translate("HTTP"))
o.default = "1"

-- SOCKS5
o = s:option(Flag, "proto_socks5", translate("SOCKS5"))
o.default = "1"

-- TLS
o = s:option(Flag, "proto_tls", translate("TLS/HTTPS"))
o.default = "1"

-- DoH
o = s:option(Flag, "proto_doh", translate("DNS over HTTPS"))
o.default = "1"

-- Shadowsocks
o = s:option(Flag, "proto_shadowsocks", translate("Shadowsocks"))
o.default = "1"

return m
