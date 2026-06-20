use super::middleware::DnsRebindProtector;

/// Shared DNS rebinding protection configuration for all HTTP framework
/// integrations.
///
/// Include this in every framework's server options struct. At startup,
/// call [`resolve_dns_middleware`] to optionally install the protection
/// middleware.
///
/// # Per-framework adoption
///
/// ```ignore
/// // 1. Add to options struct
/// pub dns_rebinding: DnsRebindingOptions,
///
/// // 2. Default it
/// dns_rebinding: DnsRebindingOptions::default(),
///
/// // 3. Install middleware in your server's new()
/// if let Some(dns) = resolve_dns_middleware(
///     &mut opts.dns_rebinding, &opts.host, opts.port,
/// ) {
///     middlewares.push(Arc::new(dns));
/// }
/// ```
///
/// # Default behavior
///
/// When `dns_rebinding_protection` is `true` and `allowed_hosts` is `None`,
/// the host is auto-derived from `host:port` unless the bind address is a
/// wildcard (`0.0.0.0`, `::`, or empty). Auto-derived hosts are logged at
/// `debug` level.
pub struct DnsRebindingOptions {
    /// Enable DNS rebinding protection. Default is `true`.
    pub dns_rebinding_protection: bool,
    /// List of allowed host header values for DNS rebinding protection.
    /// If not specified and `dns_rebinding_protection` is true, auto-derives
    /// from `host:port` unless the host is a wildcard.
    pub allowed_hosts: Option<Vec<String>>,
    /// List of allowed origin header values for DNS rebinding protection.
    /// If not specified, origin validation is disabled.
    pub allowed_origins: Option<Vec<String>>,
}

impl Default for DnsRebindingOptions {
    fn default() -> Self {
        Self {
            dns_rebinding_protection: true,
            allowed_hosts: None,
            allowed_origins: None,
        }
    }
}

fn is_wildcard(host: &str) -> bool {
    host.is_empty() || host == "0.0.0.0" || host == "::"
}

/// Resolves DNS rebinding middleware from the given options.
///
/// Returns `Some(DnsRebindProtector)` if protection should be active, or
/// `None` if protection is disabled or cannot be configured (wildcard host
/// with no explicit lists). When `allowed_hosts` is `None` and protection
/// is enabled, it auto-derives from `host:port` unless the host is a
/// wildcard.
pub fn resolve_dns_middleware(
    opts: &mut DnsRebindingOptions,
    host: &str,
    port: u16,
) -> Option<DnsRebindProtector> {
    if !opts.dns_rebinding_protection {
        return None;
    }
    let allowed_hosts = if let Some(hosts) = opts.allowed_hosts.take() {
        Some(hosts)
    } else if is_wildcard(host) {
        tracing::warn!(
            "DNS-rebinding protection is enabled but host is a wildcard ({host}) \
             and neither `allowed_hosts` nor `allowed_origins` is configured. \
             Set `allowed_hosts` explicitly, or set `dns_rebinding_protection = false`."
        );
        None
    } else {
        let host_port = format!("{host}:{port}");
        tracing::debug!(
            "DNS-rebinding protection: auto-derived allowed_hosts=[\"{host_port}\"] \
             from host config"
        );
        Some(vec![host_port])
    };
    let allowed_origins = opts.allowed_origins.take();
    if allowed_hosts.is_some() || allowed_origins.is_some() {
        Some(DnsRebindProtector::new(allowed_hosts, allowed_origins))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explicit_allowed_hosts() {
        let mut opts = DnsRebindingOptions {
            allowed_hosts: Some(vec!["example.com".into()]),
            ..Default::default()
        };
        let middleware = resolve_dns_middleware(&mut opts, "127.0.0.1", 8080).unwrap();
        assert_eq!(middleware.allowed_hosts, Some(vec!["example.com".into()]));
    }

    #[test]
    fn test_auto_derive_from_domain() {
        let mut opts = DnsRebindingOptions::default();
        let middleware = resolve_dns_middleware(&mut opts, "api.example.com", 443).unwrap();
        assert_eq!(
            middleware.allowed_hosts,
            Some(vec!["api.example.com:443".into()])
        );
    }

    #[test]
    fn test_auto_derive_from_localhost_default() {
        let mut opts = DnsRebindingOptions::default();
        let middleware = resolve_dns_middleware(&mut opts, "127.0.0.1", 8080).unwrap();
        assert_eq!(
            middleware.allowed_hosts,
            Some(vec!["127.0.0.1:8080".into()])
        );
    }

    #[test]
    fn test_wildcard_v4_returns_none() {
        let mut opts = DnsRebindingOptions::default();
        assert!(resolve_dns_middleware(&mut opts, "0.0.0.0", 8080).is_none());
    }

    #[test]
    fn test_wildcard_v6_returns_none() {
        let mut opts = DnsRebindingOptions::default();
        assert!(resolve_dns_middleware(&mut opts, "::", 8080).is_none());
    }

    #[test]
    fn test_empty_host_returns_none() {
        let mut opts = DnsRebindingOptions::default();
        assert!(resolve_dns_middleware(&mut opts, "", 8080).is_none());
    }

    #[test]
    fn test_ipv6_loopback_auto_derives() {
        let mut opts = DnsRebindingOptions::default();
        let middleware = resolve_dns_middleware(&mut opts, "::1", 3000).unwrap();
        assert_eq!(middleware.allowed_hosts, Some(vec!["::1:3000".into()]));
    }

    #[test]
    fn test_auto_derive_respects_custom_port() {
        let mut opts = DnsRebindingOptions::default();
        let middleware = resolve_dns_middleware(&mut opts, "127.0.0.1", 9090).unwrap();
        assert_eq!(
            middleware.allowed_hosts,
            Some(vec!["127.0.0.1:9090".into()])
        );
    }

    #[test]
    fn test_disabled_returns_none() {
        let mut opts = DnsRebindingOptions {
            dns_rebinding_protection: false,
            ..Default::default()
        };
        assert!(resolve_dns_middleware(&mut opts, "127.0.0.1", 8080).is_none());
    }

    #[test]
    fn test_disabled_ignores_explicit_lists() {
        let mut opts = DnsRebindingOptions {
            allowed_hosts: Some(vec!["example.com".into()]),
            dns_rebinding_protection: false,
            ..Default::default()
        };
        assert!(resolve_dns_middleware(&mut opts, "127.0.0.1", 8080).is_none());
    }
}
