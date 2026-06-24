//! Ergonomic constructors for [`ContentBlock`] and related schema types.
//!
//! The generated MCP schema exposes `TextContent::new(...)`,
//! `ImageContent::new(...)` etc, but each requires several `None` parameters
//! and a final `.into()` to produce a `ContentBlock`. These helpers wrap the
//! common shapes so application code can write
//!
//! ```
//! use rust_mcp_sdk::content::Content;
//!
//! let _block = Content::text("hello, world");
//! let _block = Content::image_png_base64("…base64…");
//! ```
//!
//! instead of multi-line constructions.

use crate::schema::{
    AudioContent, BlobResourceContents, ContentBlock, EmbeddedResource, EmbeddedResourceResource,
    ImageContent, TextContent, TextResourceContents,
};

/// Static factory namespace for building [`ContentBlock`] values.
///
/// This is a zero-sized type used only as a namespace; you never instantiate it.
pub struct Content;

impl Content {
    /// A plain text [`ContentBlock`].
    pub fn text(text: impl Into<String>) -> ContentBlock {
        TextContent::new(text.into(), None, None).into()
    }

    /// An image [`ContentBlock`] from base64-encoded data and a MIME type
    /// (e.g. `"image/png"`, `"image/jpeg"`).
    pub fn image(data: impl Into<String>, mime_type: impl Into<String>) -> ContentBlock {
        ImageContent::new(data.into(), mime_type.into(), None, None).into()
    }

    /// Shortcut for a base64-encoded PNG image.
    pub fn image_png_base64(data: impl Into<String>) -> ContentBlock {
        Self::image(data, "image/png")
    }

    /// Shortcut for a base64-encoded JPEG image.
    pub fn image_jpeg_base64(data: impl Into<String>) -> ContentBlock {
        Self::image(data, "image/jpeg")
    }

    /// An audio [`ContentBlock`] from base64-encoded data and a MIME type
    /// (e.g. `"audio/wav"`, `"audio/mpeg"`).
    pub fn audio(data: impl Into<String>, mime_type: impl Into<String>) -> ContentBlock {
        AudioContent::new(data.into(), mime_type.into(), None, None).into()
    }

    /// An embedded text resource [`ContentBlock`].
    ///
    /// Use this for resources whose content is best represented as text
    /// (JSON, source code, plain documents, …).
    pub fn embedded_text_resource(
        uri: impl Into<String>,
        mime_type: impl Into<String>,
        text: impl Into<String>,
    ) -> ContentBlock {
        let trc = TextResourceContents::new(text.into(), uri.into())
            .with_mime_type(mime_type.into());
        EmbeddedResource::new(
            EmbeddedResourceResource::TextResourceContents(trc),
            None,
            None,
        )
        .into()
    }

    /// An embedded binary (blob) resource [`ContentBlock`].
    ///
    /// `data` must be base64-encoded. Use this for binary resources such as
    /// images, audio, or arbitrary file blobs that the server is bundling
    /// inline.
    pub fn embedded_blob_resource(
        uri: impl Into<String>,
        mime_type: impl Into<String>,
        base64_data: impl Into<String>,
    ) -> ContentBlock {
        let brc = BlobResourceContents::new(base64_data.into(), uri.into())
            .with_mime_type(mime_type.into());
        EmbeddedResource::new(
            EmbeddedResourceResource::BlobResourceContents(brc),
            None,
            None,
        )
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_block() {
        let block = Content::text("hello");
        match block {
            ContentBlock::TextContent(c) => assert_eq!(c.text, "hello"),
            _ => panic!("expected TextContent"),
        }
    }

    #[test]
    fn image_block_with_mime() {
        let block = Content::image("DATA", "image/png");
        match block {
            ContentBlock::ImageContent(c) => {
                assert_eq!(c.data, "DATA");
                assert_eq!(c.mime_type, "image/png");
            }
            _ => panic!("expected ImageContent"),
        }
    }

    #[test]
    fn audio_block() {
        let block = Content::audio("AUDIO", "audio/wav");
        match block {
            ContentBlock::AudioContent(c) => {
                assert_eq!(c.data, "AUDIO");
                assert_eq!(c.mime_type, "audio/wav");
            }
            _ => panic!("expected AudioContent"),
        }
    }

    #[test]
    fn embedded_text_resource_block() {
        let block = Content::embedded_text_resource(
            "test://x",
            "application/json",
            r#"{"a":1}"#,
        );
        match block {
            ContentBlock::EmbeddedResource(c) => match c.resource {
                EmbeddedResourceResource::TextResourceContents(trc) => {
                    assert_eq!(trc.uri, "test://x");
                    assert_eq!(trc.mime_type.as_deref(), Some("application/json"));
                    assert_eq!(trc.text, r#"{"a":1}"#);
                }
                _ => panic!("expected text resource contents"),
            },
            _ => panic!("expected EmbeddedResource"),
        }
    }

    #[test]
    fn embedded_blob_resource_block() {
        let block =
            Content::embedded_blob_resource("test://y", "image/png", "BASE64DATA");
        match block {
            ContentBlock::EmbeddedResource(c) => match c.resource {
                EmbeddedResourceResource::BlobResourceContents(brc) => {
                    assert_eq!(brc.uri, "test://y");
                    assert_eq!(brc.mime_type.as_deref(), Some("image/png"));
                    assert_eq!(brc.blob, "BASE64DATA");
                }
                _ => panic!("expected blob resource contents"),
            },
            _ => panic!("expected EmbeddedResource"),
        }
    }
}
