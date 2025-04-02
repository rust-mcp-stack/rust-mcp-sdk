# rust-mcp-macros.

A procedural macro, part of the [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk) ecosystem, to generate `rust_mcp_schema::Tool` instance from a struct.

The `mcp_tool` macro generates an implementation for the annotated struct that includes:

- A `tool_name()` method returning the tool's name as a string.
- A `tool()` method returning a `rust_mcp_schema::Tool` instance with the tool's name,
  description, and input schema derived from the struct's fields.

## Attributes

- `name` - The name of the tool (required, non-empty string).
- `description` - A description of the tool (required, non-empty string).

## Usage Example

```rust
#[mcp_tool(
   name = "write_file",
   description = "Create a new file or completely overwrite an existing file with new content."
)]
#[derive(rust_mcp_macros::JsonSchema)]
pub struct WriteFileTool {
    /// The target file's path for writing content.
    pub path: String,
    /// The string content to be written to the file
    pub content: String,
}

fn main() {

    assert_eq!(WriteFileTool::tool_name(), "write_file");

    let tool: rust_mcp_schema::Tool = WriteFileTool::tool();
    assert_eq!(tool.name, "write_file");
    assert_eq!( tool.description.unwrap(),"Create a new file or completely overwrite an existing file with new content.");

    let schema_properties = tool.input_schema.properties.unwrap();
    assert_eq!(schema_properties.len(), 2);
    assert!(schema_properties.contains_key("path"));
    assert!(schema_properties.contains_key("content"));

    // get the `content` prop from schema
    let content_prop = schema_properties.get("content").unwrap();

    // assert the type
    assert_eq!(content_prop.get("type").unwrap(), "string");
    // assert the description
    assert_eq!(
        content_prop.get("description").unwrap(),
        "The string content to be written to the file"
    );
}

```

---

<img align="top" src="assets/rust-mcp-stack-icon.png" width="24" style="border-radius:0.2rem;"> Check out [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk) , a high-performance, asynchronous toolkit for building MCP servers and clients. Focus on your app's logic while [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk) takes care of the rest!

---
