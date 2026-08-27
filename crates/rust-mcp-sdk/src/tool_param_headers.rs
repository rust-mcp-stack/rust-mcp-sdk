//! SEP-2243 client support — `x-mcp-header` tool parameter annotations.
//!
//! A server's tool input schema may annotate individual primitive parameters
//! with `"x-mcp-header": "<HeaderName>"`. When calling such a tool over the
//! streamable-HTTP transport, the client MUST mirror each annotated
//! parameter's value into an `Mcp-Param-<HeaderName>` HTTP header on the
//! `tools/call` POST:
//!
//! - values whose string form is safe for an HTTP header (printable ASCII
//!   0x20–0x7E, no leading/trailing whitespace) are sent literally
//!   (`42` → `"42"`, `true` → `"true"`),
//! - any other value (non-ASCII, control characters, surrounding whitespace)
//!   is sent Base64-encoded inside an `=?base64?…?=` wrapper,
//! - parameters that are `null` or absent MUST NOT produce a header,
//! - parameters without an `x-mcp-header` annotation MUST NOT be mirrored.
//!
//! A tool whose annotations are invalid MUST be excluded from the client's
//! tool set (while other, valid tools remain usable). An annotation is
//! invalid when:
//!
//! - the header name is empty,
//! - the annotated property is not a primitive (`string` / `number` /
//!   `integer` / `boolean`),
//! - the same header name is declared twice on one tool (case-insensitive),
//! - the header name contains characters outside the visible-ASCII range
//!   (0x21–0x7E) or a colon.
//!
//! The mirroring itself is performed by a `request_header_provider` installed
//! by [`crate::mcp_runtimes::client_runtime::ClientRuntime`] on the
//! streamable-HTTP transport; this module contains the parsing, validation
//! and encoding logic plus the shared annotation registry.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use http::header::{HeaderMap, HeaderName, HeaderValue};

use crate::schema::Tool;

/// Registry of validated `x-mcp-header` annotations, keyed by tool name.
///
/// Populated when the client lists tools (see
/// [`filter_and_register`]) and read by the transport's
/// `request_header_provider` when a `tools/call` is sent.
pub type ToolHeaderRegistry = Arc<RwLock<HashMap<String, Vec<ToolParamHeader>>>>;

/// Create an empty [`ToolHeaderRegistry`].
pub fn new_tool_header_registry() -> ToolHeaderRegistry {
    Arc::new(RwLock::new(HashMap::new()))
}

/// A single validated `x-mcp-header` annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolParamHeader {
    /// Name of the tool argument the annotation applies to.
    pub param_name: String,
    /// Header suffix declared by the annotation, e.g. `"Region"` for the
    /// `Mcp-Param-Region` header.
    pub header_name: String,
}

impl ToolParamHeader {
    /// Full HTTP header name for this annotation (`Mcp-Param-<header_name>`).
    pub fn http_header_name(&self) -> String {
        format!("Mcp-Param-{}", self.header_name)
    }
}

/// Outcome of validating one tool's annotations.
enum AnnotationParse {
    /// The tool carries no `x-mcp-header` annotations at all.
    NoAnnotations,
    /// All annotations are valid.
    Valid(Vec<ToolParamHeader>),
    /// At least one annotation is invalid — the whole tool MUST be excluded.
    Invalid,
}

/// Check whether a header name is valid per SEP-2243: every character must
/// be visible ASCII (0x21–0x7E) and not a colon.
fn is_valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| matches!(c as u32, 0x21..=0x7E) && c != ':')
}

/// Extract and validate the `x-mcp-header` annotations of a single tool.
fn parse_annotations(tool: &Tool) -> AnnotationParse {
    let Some(extra) = &tool.input_schema.extra else {
        return AnnotationParse::NoAnnotations;
    };
    let Some(properties) = extra.get("properties").and_then(|p| p.as_object()) else {
        return AnnotationParse::NoAnnotations;
    };

    let mut headers: Vec<ToolParamHeader> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for (param_name, prop) in properties {
        let Some(prop_obj) = prop.as_object() else {
            return AnnotationParse::Invalid;
        };
        let Some(annotation) = prop_obj.get("x-mcp-header") else {
            continue;
        };

        // The annotation value must be a non-empty, charset-valid string.
        let Some(header_name) = annotation.as_str() else {
            return AnnotationParse::Invalid;
        };
        if !is_valid_header_name(header_name) {
            return AnnotationParse::Invalid;
        }

        // The annotated property must be a primitive type.
        let primitive = matches!(
            prop_obj.get("type").and_then(|t| t.as_str()),
            Some("string") | Some("number") | Some("integer") | Some("boolean")
        );
        if !primitive {
            return AnnotationParse::Invalid;
        }

        // Header names must be unique per tool (case-insensitive).
        if !seen.insert(header_name.to_ascii_lowercase()) {
            return AnnotationParse::Invalid;
        }

        headers.push(ToolParamHeader {
            param_name: param_name.clone(),
            header_name: header_name.to_string(),
        });
    }

    if headers.is_empty() {
        AnnotationParse::NoAnnotations
    } else {
        AnnotationParse::Valid(headers)
    }
}

/// Validate every tool in a freshly retrieved `tools/list` result: tools
/// with invalid `x-mcp-header` annotations are removed (per SEP-2243 they
/// MUST be excluded), and the annotations of the remaining tools are
/// recorded into `registry` for use by the transport's header provider.
pub fn filter_and_register(tools: &mut Vec<Tool>, registry: &ToolHeaderRegistry) {
    let mut registrations: Vec<(String, Vec<ToolParamHeader>)> = Vec::new();
    tools.retain(|tool| match parse_annotations(tool) {
        AnnotationParse::NoAnnotations => true,
        AnnotationParse::Valid(headers) => {
            registrations.push((tool.name.clone(), headers));
            true
        }
        AnnotationParse::Invalid => {
            tracing::warn!(
                "Excluding tool '{}' from the tool set: invalid x-mcp-header annotation(s)",
                tool.name
            );
            false
        }
    });

    if let Ok(mut lock) = registry.write() {
        for (name, headers) in registrations {
            lock.insert(name, headers);
        }
    }
}

/// Parse the `x-mcp-header` annotations of a `Tool`'s `inputSchema` and
/// return the validated annotations, or `None` when the tool has none or
/// they are invalid (invalid annotations make the whole tool unusable, per
/// SEP-2243).
pub fn annotations_for_tool(tool: &Tool) -> Option<Vec<ToolParamHeader>> {
    match parse_annotations(tool) {
        AnnotationParse::Valid(headers) => Some(headers),
        _ => None,
    }
}

/// `true` when `value` can be sent as a literal HTTP header value: printable
/// ASCII (0x20–0x7E) with no leading or trailing whitespace.
fn is_header_safe(value: &str) -> bool {
    value.chars().all(|c| matches!(c as u32, 0x20..=0x7E)) && value == value.trim()
}

/// Encode an annotated argument value for its `Mcp-Param-*` header.
///
/// Returns `None` when the parameter MUST be omitted (`null` or a
/// non-primitive value, which cannot be represented). Strings that are not
/// header-safe are wrapped as `=?base64?<base64>?=`.
fn encode_header_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::String(s) => {
            if is_header_safe(s) {
                Some(s.clone())
            } else {
                Some(format!("=?base64?{}?=", BASE64.encode(s)))
            }
        }
        // Non-primitive argument values cannot be mirrored; annotated
        // parameters are primitive by validation, so this is defensive.
        _ => None,
    }
}

/// Compute the `Mcp-Param-*` headers for one JSON-RPC request payload.
///
/// Intended as the body of the transport's `request_header_provider`
/// callback: returns `Some(headers)` for `tools/call` payloads that carry
/// mirrored values, `None` otherwise.
fn headers_for_payload(
    payload: &serde_json::Value,
    registry: &ToolHeaderRegistry,
) -> Option<HeaderMap> {
    if payload.get("method").and_then(|m| m.as_str()) != Some("tools/call") {
        return None;
    }
    let params = payload.get("params")?;
    let tool_name = params.get("name").and_then(|n| n.as_str())?;

    let annotations = registry.read().ok()?.get(tool_name)?.clone();
    if annotations.is_empty() {
        return None;
    }

    let empty = serde_json::Map::new();
    let arguments = params
        .get("arguments")
        .and_then(|a| a.as_object())
        .unwrap_or(&empty);

    let mut map = HeaderMap::new();
    for annotation in &annotations {
        let Some(value) = arguments.get(&annotation.param_name) else {
            continue;
        };
        let Some(encoded) = encode_header_value(value) else {
            continue;
        };
        let Ok(name) = HeaderName::try_from(annotation.http_header_name()) else {
            continue;
        };
        let Ok(value) = HeaderValue::from_str(&encoded) else {
            continue;
        };
        map.insert(name, value);
    }

    if map.is_empty() {
        None
    } else {
        Some(map)
    }
}

/// The `request_header_provider` callback installed by the client runtime.
///
/// Parses the raw JSON-RPC payload (single message or batch) and returns the
/// `Mcp-Param-*` headers for any `tools/call` it contains.
pub fn request_header_provider(payload: &str, registry: &ToolHeaderRegistry) -> Option<HeaderMap> {
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    match &value {
        serde_json::Value::Array(batch) => {
            let mut merged: Option<HeaderMap> = None;
            for item in batch {
                if let Some(headers) = headers_for_payload(item, registry) {
                    merged.get_or_insert_with(HeaderMap::new).extend(headers);
                }
            }
            merged.filter(|m| !m.is_empty())
        }
        _ => headers_for_payload(&value, registry),
    }
}

/// Compute the SEP-2243 standard headers for one JSON-RPC request payload:
/// `Mcp-Method` (the request method) plus `Mcp-Name` when the request targets
/// a named object (`tools/call` / `prompts/get` `params.name`, or
/// `resources/read` `params.uri`).
fn standard_headers_for_payload(payload: &serde_json::Value) -> Option<HeaderMap> {
    let method = payload.get("method").and_then(|m| m.as_str())?;
    // Only requests (with an id) carry the standard headers.
    payload.get("id")?;

    let mut map = HeaderMap::new();
    map.insert(
        HeaderName::try_from(rust_mcp_transport::MCP_METHOD_HEADER).ok()?,
        HeaderValue::from_str(method).ok()?,
    );

    let target = match method {
        "tools/call" | "prompts/get" => payload
            .get("params")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str()),
        "resources/read" => payload
            .get("params")
            .and_then(|p| p.get("uri"))
            .and_then(|n| n.as_str()),
        _ => None,
    };
    if let Some(target) = target {
        if let (Ok(name), Ok(value)) = (
            HeaderName::try_from(rust_mcp_transport::MCP_NAME_HEADER),
            HeaderValue::from_str(target),
        ) {
            map.insert(name, value);
        }
    }

    Some(map)
}

/// The standard-headers provider chained into the client runtime's transport:
/// emits `Mcp-Method` / `Mcp-Name` (SEP-2243) for every request payload.
pub fn standard_header_provider(payload: &str) -> Option<HeaderMap> {
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    match &value {
        serde_json::Value::Array(batch) => {
            let mut merged: Option<HeaderMap> = None;
            for item in batch {
                if let Some(headers) = standard_headers_for_payload(item) {
                    merged.get_or_insert_with(HeaderMap::new).extend(headers);
                }
            }
            merged.filter(|m| !m.is_empty())
        }
        _ => standard_headers_for_payload(&value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tool_with_properties(properties: serde_json::Value) -> Tool {
        let mut extra = serde_json::Map::new();
        extra.insert("properties".to_string(), properties);
        Tool {
            name: "test_tool".into(),
            input_schema: crate::schema::ToolInputSchema::new(None, Some(extra)),
            annotations: None,
            description: None,
            icons: vec![],
            meta: None,
            output_schema: None,
            title: None,
        }
    }

    fn parse(tool: &Tool) -> Option<Vec<ToolParamHeader>> {
        match parse_annotations(tool) {
            AnnotationParse::NoAnnotations => Some(vec![]),
            AnnotationParse::Valid(headers) => Some(headers),
            AnnotationParse::Invalid => None,
        }
    }

    #[test]
    fn valid_annotations_are_collected() {
        let tool = tool_with_properties(json!({
            "region": { "type": "string", "x-mcp-header": "Region" },
            "priority": { "type": "integer", "x-mcp-header": "Priority" },
            "verbose": { "type": "boolean", "x-mcp-header": "Verbose" },
            "unannotated": { "type": "string" }
        }));
        let headers = parse(&tool).expect("valid tool");
        assert_eq!(headers.len(), 3);
        assert!(headers
            .iter()
            .any(|h| h.param_name == "region" && h.http_header_name() == "Mcp-Param-Region"));
    }

    #[test]
    fn empty_header_name_is_invalid() {
        let tool = tool_with_properties(json!({ "p": { "type": "string", "x-mcp-header": "" } }));
        assert!(parse(&tool).is_none());
    }

    #[test]
    fn non_primitive_types_are_invalid() {
        for bad in ["object", "array", "null"] {
            let tool = tool_with_properties(json!({
                "p": { "type": bad, "x-mcp-header": "X" }
            }));
            assert!(parse(&tool).is_none(), "type {bad} must be invalid");
        }
    }

    #[test]
    fn duplicate_header_names_are_invalid() {
        for (a, b) in [("Region", "Region"), ("Region", "region")] {
            let tool = tool_with_properties(json!({
                "p1": { "type": "string", "x-mcp-header": a },
                "p2": { "type": "string", "x-mcp-header": b }
            }));
            assert!(parse(&tool).is_none(), "duplicate {a}/{b} must be invalid");
        }
    }

    #[test]
    fn bad_charset_header_names_are_invalid() {
        for bad_name in ["has space", "has:colon", "café", "line\nbreak", "\u{7f}del"] {
            let tool = tool_with_properties(json!({
                "p": { "type": "string", "x-mcp-header": bad_name }
            }));
            assert!(parse(&tool).is_none(), "name {bad_name:?} must be invalid");
        }
    }

    #[test]
    fn non_string_annotation_is_invalid() {
        let tool = tool_with_properties(json!({
            "p": { "type": "string", "x-mcp-header": { "nested": true } }
        }));
        assert!(parse(&tool).is_none());
    }

    #[test]
    fn safe_values_encode_literally() {
        assert_eq!(
            encode_header_value(&json!("us-west1")).as_deref(),
            Some("us-west1")
        );
        assert_eq!(encode_header_value(&json!(42)).as_deref(), Some("42"));
        assert_eq!(encode_header_value(&json!(true)).as_deref(), Some("true"));
        assert_eq!(encode_header_value(&json!(3.5)).as_deref(), Some("3.5"));
        assert_eq!(encode_header_value(&json!("")).as_deref(), Some(""));
    }

    #[test]
    fn unsafe_values_encode_base64() {
        let encoded = encode_header_value(&json!("Hello, 世界")).unwrap();
        assert!(encoded.starts_with("=?base64?") && encoded.ends_with("?="));
        let body = &encoded["=?base64?".len()..encoded.len() - 2];
        assert_eq!(BASE64.decode(body).unwrap(), "Hello, 世界".as_bytes());

        for unsafe_value in [
            " padded ",
            "us west1 ",
            "line1\nline2",
            "line1\rline2",
            "\tindented",
        ] {
            let encoded = encode_header_value(&json!(unsafe_value)).unwrap();
            assert!(
                encoded.starts_with("=?base64?"),
                "{unsafe_value:?} must be base64 wrapped, got {encoded:?}"
            );
        }
    }

    #[test]
    fn null_and_non_primitive_values_are_omitted() {
        assert_eq!(encode_header_value(&json!(null)), None);
        assert_eq!(encode_header_value(&json!({"a": 1})), None);
        assert_eq!(encode_header_value(&json!([1, 2])), None);
    }

    #[test]
    fn filter_removes_invalid_tools_and_registers_valid() {
        let registry = new_tool_header_registry();
        let mut tools = vec![
            tool_with_properties(json!({
                "region": { "type": "string", "x-mcp-header": "Region" }
            })),
            {
                let mut t = tool_with_properties(json!({
                    "p": { "type": "object", "x-mcp-header": "X" }
                }));
                t.name = "invalid_tool".into();
                t
            },
        ];
        tools[0].name = "valid_tool".into();

        filter_and_register(&mut tools, &registry);

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "valid_tool");
        let lock = registry.read().unwrap();
        assert!(lock.contains_key("valid_tool"));
        assert!(!lock.contains_key("invalid_tool"));
    }

    #[test]
    fn provider_computes_headers_for_tool_call() {
        let registry = new_tool_header_registry();
        let mut tools = vec![tool_with_properties(json!({
            "region": { "type": "string", "x-mcp-header": "Region" },
            "verbose": { "type": "boolean", "x-mcp-header": "Verbose" },
            "non_ascii_val": { "type": "string", "x-mcp-header": "NonAscii" }
        }))];
        tools[0].name = "test_tool".into();
        filter_and_register(&mut tools, &registry);

        let payload = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "test_tool",
                "arguments": { "region": "us-west1", "verbose": null, "non_ascii_val": "Hello, 世界" }
            }
        })
        .to_string();

        let headers = request_header_provider(&payload, &registry).expect("headers expected");
        assert_eq!(headers.get("mcp-param-region").unwrap(), "us-west1");
        // null parameter must be omitted
        assert!(headers.get("mcp-param-verbose").is_none());
        // non-ASCII must be base64 wrapped
        let encoded = headers.get("mcp-param-nonascii").unwrap().to_str().unwrap();
        assert!(encoded.starts_with("=?base64?"));

        // Unrelated payloads produce no headers.
        assert!(request_header_provider(
            &json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}).to_string(),
            &registry
        )
        .is_none());
    }
}
