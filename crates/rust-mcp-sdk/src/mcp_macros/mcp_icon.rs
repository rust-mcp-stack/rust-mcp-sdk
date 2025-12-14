#[macro_export]
/// Macro to conveniently create an `Icon` instance using a shorthand syntax".
///
/// # Syntax
/// ```text
/// mcp_icon!(
///     src = "path_or_url",
///     mime_type = "optional_mime_type",
///     sizes = ["WxH", "WxH", ...],
///     theme = "dark" | "light",
/// )
/// ```
///
/// # Rules
/// - `src` is **mandatory**.
/// - `mime_type`, `sizes`, and `theme` are **optional**.
/// - If `theme` is missing or invalid, it defaults to `IconTheme::Light`.
/// - `sizes` uses a Rust array of string literals (DSL style), which are converted to `Vec<String>`.
///
/// # Example
/// ```rust
/// let my_icon: rust_mcp_sdk::schema::Icon = rust_mcp_sdk::mcp_icon!(
///     src = "/icons/dark.png",
///     mime_type = "image/png",
///     sizes = ["128x128", "256x256"],
///     theme = "dark"
/// );
/// ```
macro_rules! mcp_icon {
    (
        src = $src:expr
        $(, mime_type = $mime_type:expr )?
        $(, sizes = [$($size:expr),* $(,)?] )?
        $(, theme = $theme:expr )?
        $(,)?
    ) => {
        $crate::schema::Icon {
            src: $src.into(),
            mime_type: None $(.or(Some($mime_type.into())))?,
            sizes: vec![$($($size.into()),*)?],
            theme: None $(.or(Some($theme.into())))?,
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::schema::*;

    // Helper function to convert IconTheme to &str for easy comparisons
    fn theme_str(theme: Option<IconTheme>) -> &'static str {
        match theme {
            Some(IconTheme::Dark) => "dark",
            Some(IconTheme::Light) => "light",
            None => "none",
        }
    }

    #[test]
    fn test_minimal_icon() {
        // Only mandatory src
        let icon = mcp_icon!(src = "/icons/simple.png");
        assert_eq!(icon.src, "/icons/simple.png");
        assert!(icon.mime_type.is_none());
        assert!(icon.sizes.is_empty());
        assert!(icon.theme.is_none());
    }

    #[test]
    fn test_icon_with_mime_type() {
        let icon = mcp_icon!(src = "/icons/simple.png", mime_type = "image/png");
        assert_eq!(icon.src, "/icons/simple.png");
        assert_eq!(icon.mime_type.as_deref(), Some("image/png"));
        assert!(icon.sizes.is_empty());
        assert!(icon.theme.is_none());
    }

    #[test]
    fn test_icon_with_sizes() {
        let icon = mcp_icon!(src = "/icons/simple.png", sizes = ["32x32", "64x64"]);
        assert_eq!(icon.src, "/icons/simple.png");
        assert!(icon.mime_type.is_none());
        assert_eq!(icon.sizes, vec!["32x32", "64x64"]);
        assert!(icon.theme.is_none());
    }

    #[test]
    fn test_icon_with_theme_light() {
        let icon = mcp_icon!(src = "/icons/simple.png", theme = "light");
        assert_eq!(icon.src, "/icons/simple.png");
        assert!(icon.mime_type.is_none());
        assert!(icon.sizes.is_empty());
        assert_eq!(theme_str(icon.theme), "light");
    }

    #[test]
    fn test_icon_with_theme_dark() {
        let icon = mcp_icon!(src = "/icons/simple.png", theme = "dark");
        assert_eq!(theme_str(icon.theme), "dark");
    }

    #[test]
    fn test_icon_with_invalid_theme_defaults_to_light() {
        let icon = mcp_icon!(src = "/icons/simple.png", theme = "foo");
        // Invalid theme should default to Light
        assert_eq!(theme_str(icon.theme), "light");
    }

    #[test]
    fn test_icon_full() {
        let icon = mcp_icon!(
            src = "/icons/full.png",
            mime_type = "image/png",
            sizes = ["16x16", "32x32", "64x64"],
            theme = "dark"
        );

        assert_eq!(icon.src, "/icons/full.png");
        assert_eq!(icon.mime_type.as_deref(), Some("image/png"));
        assert_eq!(icon.sizes, vec!["16x16", "32x32", "64x64"]);
        assert_eq!(theme_str(icon.theme), "dark");
    }

    #[test]
    fn test_icon_sizes_empty_when_missing() {
        let icon = mcp_icon!(src = "/icons/empty.png");
        assert!(icon.sizes.is_empty());
    }

    #[test]
    fn test_icon_optional_fields_missing() {
        let icon = mcp_icon!(src = "/icons/missing.png");
        assert!(icon.mime_type.is_none());
        assert!(icon.sizes.is_empty());
        assert!(icon.theme.is_none());
    }

    #[test]
    fn test_icon_trailing_comma() {
        let icon = mcp_icon!(
            src = "/icons/comma.png",
            mime_type = "image/jpeg",
            sizes = ["48x48"],
            theme = "light",
        );
        assert_eq!(icon.src, "/icons/comma.png");
        assert_eq!(icon.mime_type.as_deref(), Some("image/jpeg"));
        assert_eq!(icon.sizes, vec!["48x48"]);
        assert_eq!(theme_str(icon.theme), "light");
    }
}
