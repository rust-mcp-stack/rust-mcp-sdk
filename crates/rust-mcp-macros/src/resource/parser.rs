use crate::common::{GenericMcpMacroAttributes, IconDsl};
use syn::{parse::Parse, Error};

pub(crate) const VALID_ROLES: [&str; 2] = ["assistant", "user"];

#[derive(Debug)]
pub(crate) struct McpResourceMacroAttributes {
    pub name: Option<String>,
    pub description: Option<String>,
    pub meta: Option<String>,
    pub title: Option<String>,
    pub icons: Option<Vec<IconDsl>>,
    pub mime_type: Option<String>,
    pub size: Option<i64>,
    pub uri: Option<String>,
    pub audience: Option<Vec<String>>,
}

impl Parse for McpResourceMacroAttributes {
    fn parse(attributes: syn::parse::ParseStream) -> syn::Result<Self> {
        let GenericMcpMacroAttributes {
            name,
            description,
            meta,
            title,
            icons,
            mime_type,
            size,
            uri,
            audience,
            uri_template: _,
        } = GenericMcpMacroAttributes::parse(attributes)?;

        let instance = Self {
            name,
            description,
            meta,
            title,
            icons,
            mime_type,
            size,
            uri,
            audience,
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
            .uri
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
        {
            return Err(Error::new(
                attributes.span(),
                "The 'uri' attribute is required and must not be empty.",
            ));
        }

        if instance
            .audience
            .as_ref()
            .map(|s| s.len())
            .unwrap_or_default()
            > VALID_ROLES.len()
        {
            return Err(Error::new(
                attributes.span(),
                format!("valid audience values are : {}. Is there any duplication in the audience values?", VALID_ROLES.join(" , ")),
            ));
        }

        Ok(instance)
    }
}

#[derive(Debug)]
pub(crate) struct McpResourceTemplateMacroAttributes {
    pub name: Option<String>,
    pub description: Option<String>,
    pub meta: Option<String>,
    pub title: Option<String>,
    pub icons: Option<Vec<IconDsl>>,
    pub mime_type: Option<String>,
    pub size: Option<i64>,
    pub uri_template: Option<String>,
    pub audience: Option<Vec<String>>,
}

impl Parse for McpResourceTemplateMacroAttributes {
    fn parse(attributes: syn::parse::ParseStream) -> syn::Result<Self> {
        let GenericMcpMacroAttributes {
            name,
            description,
            meta,
            title,
            icons,
            mime_type,
            size,
            uri: _,
            audience,
            uri_template,
        } = GenericMcpMacroAttributes::parse(attributes)?;

        let instance = Self {
            name,
            description,
            meta,
            title,
            icons,
            mime_type,
            size,
            uri_template,
            audience,
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
            .uri_template
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
        {
            return Err(Error::new(
                attributes.span(),
                "The 'uri_template' attribute is required and must not be empty.",
            ));
        }

        if instance
            .audience
            .as_ref()
            .map(|s| s.len())
            .unwrap_or_default()
            > VALID_ROLES.len()
        {
            return Err(Error::new(
                attributes.span(),
                format!("valid audience values are : {}. Is there any duplication in the audience values?", VALID_ROLES.join(" , ")),
            ));
        }

        Ok(instance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_str;

    fn parse_attributes(input: &str) -> syn::Result<McpResourceMacroAttributes> {
        parse_str(input)
    }

    #[test]
    fn test_minimal_required_attributes() {
        let attrs = parse_attributes(
            r#"
            name = "test-resource",
            description = "A test resource",
            uri="ks://crmofaroundc"
        "#,
        )
        .unwrap();

        assert_eq!(attrs.name, Some("test-resource".to_string()));
        assert_eq!(attrs.description, Some("A test resource".to_string()));
        assert_eq!(attrs.title, None);
        assert_eq!(attrs.meta, None);
        assert!(attrs.icons.is_none());
        assert_eq!(attrs.mime_type, None);
        assert_eq!(attrs.size, None);
        assert_eq!(attrs.uri.clone(), Some("ks://crmofaroundc".into()));
        assert_eq!(attrs.audience, None);
    }

    #[test]
    fn test_all_attributes_with_simple_values() {
        let attrs = parse_attributes(
            r#"
            name = "my-file",
            description = "Important document",
            title = "My Document",
            meta = "{\"key\": \"value\", \"num\": 42}",
            mime_type = "application/pdf",
            size = 1024,
            uri = "https://example.com/file.pdf",
            audience = ["user", "assistant"],
            icons = [(src = "icon.png", mime_type = "image/png", sizes = ["48x48"])]
        "#,
        )
        .unwrap();

        assert_eq!(attrs.name.as_deref(), Some("my-file"));
        assert_eq!(attrs.description.as_deref(), Some("Important document"));
        assert_eq!(attrs.title.as_deref(), Some("My Document"));
        assert_eq!(
            attrs.meta.as_deref(),
            Some("{\"key\": \"value\", \"num\": 42}")
        );
        assert_eq!(attrs.mime_type.as_deref(), Some("application/pdf"));
        assert_eq!(attrs.size, Some(1024));
        assert_eq!(attrs.uri.as_deref(), Some("https://example.com/file.pdf"));
        assert_eq!(
            attrs.audience,
            Some(vec!["user".to_string(), "assistant".to_string()])
        );

        let icons = attrs.icons.unwrap();
        assert_eq!(icons.len(), 1);
        assert_eq!(icons[0].src.value(), "icon.png");

        assert_eq!(icons[0].sizes.as_ref().unwrap(), &vec!["48x48".to_string()]);
        assert_eq!(icons[0].mime_type, Some("image/png".to_string()));
    }

    #[test]
    fn test_concat_in_string_fields() {
        let attrs = parse_attributes(
            r#"
            name = concat!("prefix-", "resource"),
            description = concat!("This is ", "a multi-part ", "description"),
            title = concat!("Title: ", "Document"),
            uri="ks://crmofaroundc"

        "#,
        )
        .unwrap();

        assert_eq!(attrs.name, Some("prefix-resource".to_string()));
        assert_eq!(
            attrs.description,
            Some("This is a multi-part description".to_string())
        );
        assert_eq!(attrs.title, Some("Title: Document".to_string()));
    }

    #[test]
    fn test_multiple_icons() {
        let attrs = parse_attributes(
            r#"
            name = "app",
            uri="ks://crmofaroundc",
            description = "App with icons",
    icons = [(src = "icon-192.png", sizes = ["192x192"]),
             (src = "icon-512.png",  mime_type = "image/png", sizes = ["512x512"]),
            ]
        "#,
        )
        .unwrap();

        let icons = attrs.icons.unwrap();
        assert_eq!(icons.len(), 2);
        assert_eq!(icons[0].src.value(), "icon-192.png");
        assert_eq!(icons[1].src.value(), "icon-512.png");
        assert_eq!(icons[1].mime_type, Some("image/png".to_string()));
    }

    #[test]
    fn test_missing_name() {
        let err = parse_attributes(
            r#"
            description = "Has description but no name"
        "#,
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "The 'name' attribute is required and must not be empty."
        );
    }

    #[test]
    fn test_missing_uri() {
        let err = parse_attributes(
            r#"
            name = "has-name",
        "#,
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "The 'uri' attribute is required and must not be empty."
        );
    }

    #[test]
    fn test_invalid_audience() {
        let err = parse_attributes(
            r#"
            name = "has-name",
            uri="something",
            audience = ["user", "secretary"],
        "#,
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "valid audience values are : assistant , user"
        );
    }

    #[test]
    fn test_duplicated_audience() {
        let err = parse_attributes(
            r#"
            name = "has-name",
            uri="something",
            audience = ["user", "assistant", "user"],
        "#,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("Is there any duplication in the audience values?"),);
    }

    #[test]
    fn test_empty_name() {
        let err = parse_attributes(
            r#"
            name = "",
            description = "valid"
        "#,
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "The 'name' attribute is required and must not be empty."
        );
    }

    #[test]
    fn test_invalid_meta_not_json_object() {
        let err = parse_attributes(
            r#"
            name = "test",
            description = "test",
            meta = "[1, 2, 3]"
        "#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("Expected a JSON object"));
    }

    #[test]
    fn test_invalid_meta_not_string() {
        let err = parse_attributes(
            r#"
            name = "test",
            description = "test",
            meta = { invalid }
        "#,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("Expected a JSON object as a string literal"));
    }

    #[test]
    fn test_invalid_audience_not_array() {
        let err = parse_attributes(
            r#"
            name = "test",
            description = "test",
            audience = "not-an-array"
        "#,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("Expected an array of string literals"));
    }

    #[test]
    fn test_audience_with_non_string() {
        let err = parse_attributes(
            r#"
            name = "test",
            description = "test",
            audience = ["user", 123]
        "#,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("Expected a string literal in array"));
    }

    #[test]
    fn test_icons_not_array() {
        let err = parse_attributes(
            r#"
            name = "test",
            description = "test",
            icons = (src = "icon.png")
        "#,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("Expected an array for the 'icons' attribute"));
    }

    #[test]
    fn test_size_not_integer() {
        let err = parse_attributes(
            r#"
            name = "test",
            description = "test",
            size = "not-a-number"
        "#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("Expected a integer literal"));
    }

    #[test]
    fn test_unknown_attribute_is_ignored() {
        // The parser currently ignores unknown name-value pairs silently
        let attrs = parse_attributes(
            r#"
            name = "test",
            description = "test",
            unknown = "should be ignored",
            uri="ks://crmofaroundc"
        "#,
        )
        .unwrap();

        assert_eq!(attrs.name.as_deref(), Some("test"));
        // No panic or error on unknown field
    }

    #[test]
    fn test_invalid_concat_usage() {
        let err = parse_attributes(
            r#"
            name = concat!(123),
            description = "valid"
        "#,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("Only string literals are allowed inside concat!()"));
    }

    #[test]
    fn test_unsupported_expr_in_string_field() {
        let err = parse_attributes(
            r#"
            name = env!("CARGO_PKG_NAME"),
            description = "valid"
        "#,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("Expected a string literal or concat!(...)"));
    }
}
