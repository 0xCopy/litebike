# LiteBike Binary Package Makefile
# For GL.iNet BE3600 (IPQ5332, aarch64_cortex-a53)

include $(TOPDIR)/rules.mk

PKG_NAME:=litebike
PKG_VERSION:=0.1.0
PKG_RELEASE:=1

PKG_BUILD_DIR:=$(BUILD_DIR)/$(PKG_NAME)-$(PKG_VERSION)

include $(INCLUDE_DIR)/package.mk

define Package/litebike
    SECTION:=net
    CATEGORY:=Network
    SUBMENU:=Proxy Servers
    TITLE:=LiteBike Integrated Proxy Server
    DEPENDS:=+libpthread +librt +libopenssl +zlib +libstdcpp
    ARCH:=aarch64_cortex-a53_neon-vfpv4
endef

define Package/litebike/description
    LiteBike is an integrated proxy server with:
    - Universal protocol detection (HTTP, SOCKS5, TLS, DoH, etc.)
    - Gate-based routing (Knox, Shadowsocks, Crypto, HTX)
    - SIMD-accelerated pattern matching
    - Knox bypass for restrictive networks
    - Channel management with failover
    - Symmetrical gateway auto-configuration
    
    Optimized for GL.iNet BE3600 (IPQ5332, aarch64_cortex-a53_neon-vfpv4)
    Requires: libev, libevent2-7, libjson-c5, libpcre, libopenssl3, zlib
endef

define Build/Compile
    # Binary is pre-compiled, no build needed
endef

define Package/litebike/install
    $(INSTALL_DIR) $(1)/usr/bin
    $(INSTALL_BIN) $(PKG_BUILD_DIR)/litebike $(1)/usr/bin/
    
    $(INSTALL_DIR) $(1)/etc/config
    $(INSTALL_CONF) ./files/etc/config/litebike $(1)/etc/config/
    
    $(INSTALL_DIR) $(1)/etc/init.d
    $(INSTALL_BIN) ./files/etc/init.d/litebike $(1)/etc/init.d/
    
    $(INSTALL_DIR) $(1)/etc/uci-defaults
    $(INSTALL_BIN) ./files/etc/uci-defaults/litebike $(1)/etc/uci-defaults/
    
    $(INSTALL_DIR) $(1)/www/luci-static/litebike
    $(INSTALL_DATA) ./files/www/luci-static/litebike/style.css $(1)/www/luci-static/litebike/
    
    $(INSTALL_DIR) $(1)/usr/share/rpcd/acl.d
    $(INSTALL_DATA) ./files/usr/share/rpcd/acl.d/litebike.json $(1)/usr/share/rpcd/acl.d/
endef

define Package/litebike/postinst
#!/bin/sh
[ -n "$${IPKG_INSTROOT}" ] || {
    /etc/init.d/litebike enable
    /etc/init.d/litebike start
}
exit 0
endef

$(eval $(call BuildPackage,litebike))
