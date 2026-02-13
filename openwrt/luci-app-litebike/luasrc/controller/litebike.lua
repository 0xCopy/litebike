-- LiteBike LuCI Controller
-- Handles web UI routing and API endpoints

module("luci.controller.litebike", package.seeall)

function index()
    entry({"admin", "services", "litebike"}, 
        alias("admin", "services", "litebike", "config"),
        _("LiteBike Proxy"), 60).dependent = true
    
    entry({"admin", "services", "litebike", "config"},
        cbi("litebike/config"),
        _("Settings"), 10).leaf = true
    
    entry({"admin", "services", "litebike", "status"},
        call("action_status"),
        _("Status"), 20).leaf = true
    
    entry({"admin", "services", "litebike", "logs"},
        call("action_logs"),
        _("Logs"), 30).leaf = true
    
    entry({"admin", "services", "litebike", "api", "status"},
        call("api_status")).dependent = true
    
    entry({"admin", "services", "litebike", "api", "start"},
        call("api_start")).dependent = true
    
    entry({"admin", "services", "litebike", "api", "stop"},
        call("api_stop")).dependent = true
    
    entry({"admin", "services", "litebike", "api", "restart"},
        call("api_restart")).dependent = true
end

function action_status()
    luci.template.render("litebike/status")
end

function action_logs()
    luci.template.render("litebike/logs")
end

function api_status()
    local sys = require("luci.sys")
    local json = require("luci.jsonc")
    
    local running = sys.call("pidof litebike > /dev/null") == 0
    local status = {
        running = running,
        pid = running and sys.exec("pidof litebike") or nil,
        uptime = running and sys.exec("litebike stats uptime 2>/dev/null") or "N/A",
        connections = running and sys.exec("litebike stats connections 2>/dev/null") or "0",
        memory = running and sys.exec("litebike stats memory 2>/dev/null") or "N/A",
        version = sys.exec("litebike --version 2>/dev/null | head -1") or "unknown"
    }
    
    luci.http.prepare_content("application/json")
    luci.http.write(json.stringify(status))
end

function api_start()
    local sys = require("luci.sys")
    local uci = require("luci.model.uci").cursor()
    
    local port = uci:get("litebike", "config", "port") or "8888"
    local bind = uci:get("litebike", "config", "bind") or "0.0.0.0"
    local knox = uci:get("litebike", "config", "knox") or "0"
    
    local cmd = string.format("litebike integrated %s:%s", bind, port)
    if knox == "1" then
        cmd = cmd .. " --knox"
    end
    
    sys.call(cmd .. " &")
    
    luci.http.prepare_content("application/json")
    luci.http.write('{"status":"started"}')
end

function api_stop()
    local sys = require("luci.sys")
    sys.call("killall litebike 2>/dev/null")
    
    luci.http.prepare_content("application/json")
    luci.http.write('{"status":"stopped"}')
end

function api_restart()
    api_stop()
    luci.sys.call("sleep 1")
    api_start()
end
