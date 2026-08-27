use crate::common::{GenericMcpMacroAttributes, PromptMessageDsl};
use syn::{parse::Parse, Error};

/// Represents the attributes for the `mcp_prompt` procedural macro.
///
/// This struct parses and validates the attributes provided to the `mcp_prompt` macro.
/// The `name` attribute is required and must not be empty. `messages` is optional: when
/// omitted the prompt is declaration-only and no rendering logic is generated. When
/// provided it must contain at least one message.
#[derive(Debug)]
pub(crate) struct McpPromptMacroAttributes {
    pub name: Option<String>,
    pub description: Option<String>,
    pub meta: Option<String>,
    pub title: Option<String>,
    pub icons: Option<Vec<crate::common::IconDsl>>,
    pub messages: Option<Vec<PromptMessageDsl>>,
}

impl Parse for McpPromptMacroAttributes {
    fn parse(attributes: syn::parse::ParseStream) -> syn::Result<Self> {
        let GenericMcpMacroAttributes {
            name,
            description,
            meta,
            title,
            icons,
            mime_type: _,
            size: _,
            uri: _,
            uri_template: _,
            audience: _,
            messages,
            destructive_hint: _,
            idempotent_hint: _,
            open_world_hint: _,
            read_only_hint: _,
        } = GenericMcpMacroAttributes::parse(attributes)?;

        let instance = Self {
            name,
            description,
            meta,
            title,
            icons,
            messages,
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

        if let Some(messages) = instance.messages.as_ref() {
            if messages.is_empty() {
                return Err(Error::new(
                    attributes.span(),
                    "The 'messages' attribute must contain at least one message when provided.",
                ));
            }
        }

        Ok(instance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_str;

    #[test]
    fn test_valid_attributes() {
        let attrs: McpPromptMacroAttributes = parse_str(
            r#"
            name = "friendly-greeting",
            description = "Generate a warm greeting",
            title = "Friendly Greeting",
            messages = [
                (role = "user", content = "Hello {name}!"),
                (role = "assistant", content = "Hi there!")
            ]
        "#,
        )
        .unwrap();

        assert_eq!(attrs.name.as_deref(), Some("friendly-greeting"));
        assert_eq!(
            attrs.description.as_deref(),
            Some("Generate a warm greeting")
        );
        assert_eq!(attrs.title.as_deref(), Some("Friendly Greeting"));
        let messages = attrs.messages.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "Hello {name}!");
        assert_eq!(messages[1].role, "assistant");
    }

    #[test]
    fn test_missing_name() {
        let err: syn::Error = parse_str::<McpPromptMacroAttributes>(
            r#"
            description = "No name",
            messages = [(role = "user", content = "hi")]
        "#,
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "The 'name' attribute is required and must not be empty."
        );
    }

    #[test]
    fn test_missing_messages_is_optional() {
        let attrs: McpPromptMacroAttributes = parse_str(
            r#"
            name = "greeting"
        "#,
        )
        .unwrap();

        assert_eq!(attrs.name.as_deref(), Some("greeting"));
        assert!(attrs.messages.is_none());
    }

    #[test]
    fn test_empty_messages_array() {
        let err: syn::Error = parse_str::<McpPromptMacroAttributes>(
            r#"
            name = "greeting",
            messages = []
        "#,
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "The 'messages' attribute must contain at least one message when provided."
        );
    }

    #[test]
    fn test_invalid_role() {
        let err: syn::Error = parse_str::<McpPromptMacroAttributes>(
            r#"
            name = "greeting",
            messages = [(role = "system", content = "hi")]
        "#,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("role must be \"user\" or \"assistant\""));
    }
}
