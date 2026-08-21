//! Scope handling helpers for MCP OAuth client.
//!
//! Implements the scope-selection strategy from
//! [SEP-835](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization#scope-selection-strategy)
//! and the scope-union behavior on re-authorization required by
//! [SEP-2350](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization#scope-challenge-handling).

use std::collections::BTreeSet;

/// Apply SEP-835 scope-selection strategy: pick the first non-empty value
/// from the prioritized list:
///
/// 1. `scope` parameter from the `WWW-Authenticate` challenge
/// 2. `scopes_supported` from RFC 9728 Protected Resource Metadata
/// 3. Client-configured fallback scope (e.g. set via `McpAuthConfig::scope`)
///
/// Returns `None` when no scope is available, signaling that the
/// `scope` parameter should be omitted entirely from the OAuth request.
///
/// # Examples
///
/// ```
/// use rust_mcp_sdk::auth::select_scope;
///
/// // WWW-Authenticate wins
/// let s = select_scope(Some("mcp:read"), Some(&["mcp:basic".into()]), Some("mcp"));
/// assert_eq!(s.as_deref(), Some("mcp:read"));
///
/// // No WWW-Authenticate: PRM scopes_supported joined by space
/// let s = select_scope(None, Some(&["a".into(), "b".into()]), Some("fallback"));
/// assert_eq!(s.as_deref(), Some("a b"));
///
/// // Neither available: configured fallback
/// let s = select_scope(None, None, Some("fallback"));
/// assert_eq!(s.as_deref(), Some("fallback"));
///
/// // Nothing at all: omit scope
/// let s = select_scope(None, None, None);
/// assert!(s.is_none());
/// ```
pub fn select_scope(
    www_authenticate_scope: Option<&str>,
    prm_scopes_supported: Option<&[String]>,
    configured_scope: Option<&str>,
) -> Option<String> {
    if let Some(s) = www_authenticate_scope {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    if let Some(scopes) = prm_scopes_supported {
        let joined = scopes
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if !joined.is_empty() {
            return Some(joined);
        }
    }
    configured_scope.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

/// Compute the set-wise union of two space-separated scope strings.
///
/// Used by SEP-2350 (scope-challenge handling): when a request is
/// rejected with a 403 `insufficient_scope` challenge, a well-behaved
/// client preserves the previously-granted scopes alongside the newly
/// challenged scopes so other operations don't lose authorization.
///
/// Returns `None` when both inputs are empty.
///
/// # Examples
///
/// ```
/// use rust_mcp_sdk::auth::union_scopes;
///
/// assert_eq!(
///     union_scopes(Some("mcp:basic"), Some("mcp:write")).as_deref(),
///     Some("mcp:basic mcp:write")
/// );
/// assert_eq!(
///     union_scopes(Some("a b"), Some("b c")).as_deref(),
///     Some("a b c")
/// );
/// assert_eq!(union_scopes(None, None), None);
/// ```
pub fn union_scopes(prior: Option<&str>, challenged: Option<&str>) -> Option<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for s in prior.into_iter().flat_map(|s| s.split_whitespace()) {
        if !s.is_empty() {
            set.insert(s.to_string());
        }
    }
    for s in challenged.into_iter().flat_map(|s| s.split_whitespace()) {
        if !s.is_empty() {
            set.insert(s.to_string());
        }
    }
    if set.is_empty() {
        None
    } else {
        Some(set.into_iter().collect::<Vec<_>>().join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_scope_prefers_www_auth() {
        let s = select_scope(Some("mcp:a"), Some(&["x".into()]), Some("y"));
        assert_eq!(s.as_deref(), Some("mcp:a"));
    }

    #[test]
    fn select_scope_falls_back_to_prm_scopes_supported() {
        let s = select_scope(None, Some(&["a".into(), "b".into()]), Some("y"));
        assert_eq!(s.as_deref(), Some("a b"));
    }

    #[test]
    fn select_scope_falls_back_to_config() {
        let s = select_scope(None, None, Some("y"));
        assert_eq!(s.as_deref(), Some("y"));
    }

    #[test]
    fn select_scope_returns_none_when_nothing_available() {
        let s = select_scope(None, None, None);
        assert!(s.is_none());
    }

    #[test]
    fn select_scope_empty_strings_treated_as_absent() {
        let s = select_scope(Some(""), Some(&[]), Some(""));
        assert!(s.is_none());
    }

    #[test]
    fn union_scopes_basic() {
        assert_eq!(union_scopes(Some("a"), Some("b")).as_deref(), Some("a b"));
    }

    #[test]
    fn union_scopes_dedupes() {
        assert_eq!(
            union_scopes(Some("a b"), Some("b c")).as_deref(),
            Some("a b c")
        );
    }

    #[test]
    fn union_scopes_empty_inputs() {
        assert!(union_scopes(None, None).is_none());
        assert!(union_scopes(Some(""), Some("")).is_none());
        assert_eq!(union_scopes(Some("x"), None).as_deref(), Some("x"));
        assert_eq!(union_scopes(None, Some("y")).as_deref(), Some("y"));
    }
}
