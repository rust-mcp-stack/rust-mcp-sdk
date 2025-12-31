use quote::ToTokens;
use syn::{parse::Parse, punctuated::Punctuated, Error, Expr, ExprLit, Lit, Meta, Token};

use crate::common::{ExecutionSupportDsl, ExprList, GenericMcpMacroAttributes, IconDsl};

/// Represents the attributes for the `mcp_tool` procedural macro.
///
/// This struct parses and validates the attributes provided to the `mcp_tool` macro.
/// The `name` and `description` attributes are required and must not be empty strings.
///
/// # Fields
/// * `name` - A string representing the tool's name (required).
/// * `description` - A string describing the tool (required).
/// * `meta` - An optional JSON string for metadata.
/// * `title` - An optional string for the tool's title.
/// * The following fields are available only with the `2025_03_26` feature and later:
///   * `destructive_hint` - Optional boolean for `ToolAnnotations::destructive_hint`.
///   * `idempotent_hint` - Optional boolean for `ToolAnnotations::idempotent_hint`.
///   * `open_world_hint` - Optional boolean for `ToolAnnotations::open_world_hint`.
///   * `read_only_hint` - Optional boolean for `ToolAnnotations::read_only_hint`.
///
pub(crate) struct McpToolMacroAttributes {
    pub name: Option<String>,
    pub description: Option<String>,
    pub meta: Option<String>, // Store raw JSON string instead of parsed Map
    pub title: Option<String>,
    pub destructive_hint: Option<bool>,
    pub idempotent_hint: Option<bool>,
    pub open_world_hint: Option<bool>,
    pub read_only_hint: Option<bool>,
    pub execution: Option<ExecutionSupportDsl>,
    pub icons: Option<Vec<IconDsl>>,
}

impl Parse for McpToolMacroAttributes {
    /// Parses the macro attributes from a `ParseStream`.
    ///
    /// This implementation extracts `name`, `description`, `meta`, and `title` from the attribute input.
    /// The `name` and `description` must be provided as string literals and be non-empty.
    /// The `meta` attribute must be a valid JSON object provided as a string literal, and `title` must be a string literal.
    ///
    /// # Errors
    /// Returns a `syn::Error` if:
    /// - The `name` attribute is missing or empty.
    /// - The `description` attribute is missing or empty.
    /// - The `meta` attribute is provided but is not a valid JSON object.
    /// - The `title` attribute is provided but is not a string literal.
    fn parse(attributes: syn::parse::ParseStream) -> syn::Result<Self> {
        let GenericMcpMacroAttributes {
            name,
            description,
            meta,
            title,
            icons,
            mime_type: _,
            audience: _,
            uri_template: _,
            uri: _,
            size: _,
            destructive_hint,
            idempotent_hint,
            open_world_hint,
            read_only_hint,
            execution,
        } = GenericMcpMacroAttributes::parse(attributes)?;

        let instance = Self {
            name,
            description,
            meta,
            title,
            destructive_hint,
            idempotent_hint,
            open_world_hint,
            read_only_hint,
            execution,
            icons,
        };

        // Validate presence and non-emptiness
        if instance
            .name
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
        {
            return Err(Error::new(
                attributes.span(),
                "The 'name' attribute is required and must not be empty.",
            ));
        }

        if instance
            .description
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
        {
            return Err(Error::new(
                attributes.span(),
                "The 'description' attribute is required and must not be empty.",
            ));
        }

        Ok(instance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_str;
    #[test]
    fn test_valid_macro_attributes() {
        let input = r#"name = "test_tool", description = "A test tool.", meta = "{\"version\": \"1.0\"}", title = "Test Tool""#;
        let parsed: McpToolMacroAttributes = parse_str(input).unwrap();

        assert_eq!(parsed.name.unwrap(), "test_tool");
        assert_eq!(parsed.description.unwrap(), "A test tool.");
        assert_eq!(parsed.meta.unwrap(), "{\"version\": \"1.0\"}");
        assert_eq!(parsed.title.unwrap(), "Test Tool");
    }

    #[test]
    fn test_missing_name() {
        let input = r#"description = "Only description""#;
        let result: Result<McpToolMacroAttributes, Error> = parse_str(input);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap().to_string(),
            "The 'name' attribute is required and must not be empty."
        );
    }

    #[test]
    fn test_missing_description() {
        let input = r#"name = "OnlyName""#;
        let result: Result<McpToolMacroAttributes, Error> = parse_str(input);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap().to_string(),
            "The 'description' attribute is required and must not be empty."
        );
    }

    #[test]
    fn test_empty_name_field() {
        let input = r#"name = "", description = "something""#;
        let result: Result<McpToolMacroAttributes, Error> = parse_str(input);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap().to_string(),
            "The 'name' attribute is required and must not be empty."
        );
    }

    #[test]
    fn test_empty_description_field() {
        let input = r#"name = "my-tool", description = """#;
        let result: Result<McpToolMacroAttributes, Error> = parse_str(input);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap().to_string(),
            "The 'description' attribute is required and must not be empty."
        );
    }

    #[test]
    fn test_invalid_meta() {
        let input =
            r#"name = "test_tool", description = "A test tool.", meta = "not_a_json_object""#;
        let result: Result<McpToolMacroAttributes, Error> = parse_str(input);
        assert!(result.is_err());
        assert!(result
            .err()
            .unwrap()
            .to_string()
            .contains("Expected a valid JSON object"));
    }

    #[test]
    fn test_non_object_meta() {
        let input = r#"name = "test_tool", description = "A test tool.", meta = "[1, 2, 3]""#;
        let result: Result<McpToolMacroAttributes, Error> = parse_str(input);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap().to_string(), "Expected a JSON object");
    }
}
