# rust-mcp-macros.

A procedural macro, part of the [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk) ecosystem, to generate `rust_mcp_schema::Tool` instance from a struct.

The `mcp_tool` macro generates an implementation for the annotated struct that includes:

- A `tool_name()` method returning the tool's name as a string.
- A `tool()` method returning a `rust_mcp_schema::Tool` instance with the tool's name,
  description, and input schema derived from the struct's fields.

## Attributes

- `name` - The name of the tool (required, non-empty string).
- `description` - A description of the tool (required, non-empty string).
- `title` - An optional human-readable and easily understood title.
- `meta` - An optional JSON string that provides additional metadata for the tool.
- `destructive_hint` – Optional boolean, indicates whether the tool may make destructive changes to its environment.
- `idempotent_hint` – Optional boolean, indicates whether repeated calls with the same input have the same effect.
- `open_world_hint` – Optional boolean, indicates whether the tool can interact with external or unknown entities.
- `read_only_hint` – Optional boolean, indicates whether the tool makes no modifications to its environment.



## Usage Example

```rust
#[mcp_tool(
   name = "write_file",
   title = "Write File Tool"
   description = "Create a new file or completely overwrite an existing file with new content."
   destructive_hint = false
   idempotent_hint = false
   open_world_hint = false
   read_only_hint = false
   meta = r#"{
       "key" : "value",
       "string_meta" : "meta value",
       "numeric_meta" : 15
   }"#
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
    assert_eq!(tool.title.as_ref().unwrap(), "Write File Tool");
    assert_eq!( tool.description.unwrap(),"Create a new file or completely overwrite an existing file with new content.");

    let meta: &Map<String, Value> = tool.meta.as_ref().unwrap();
    assert_eq!(
        meta.get("key").unwrap(),
        &Value::String("value".to_string())
    );

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


**Note**: The following attributes are available only in version `2025_03_26` and later of the MCP Schema, and their values will be used in the [annotations](https://github.com/rust-mcp-stack/rust-mcp-schema/blob/main/src/generated_schema/2025_03_26/mcp_schema.rs#L5557) attribute of the *[Tool struct](https://github.com/rust-mcp-stack/rust-mcp-schema/blob/main/src/generated_schema/2025_03_26/mcp_schema.rs#L5554-L5566).

- `destructive_hint`
- `idempotent_hint`
- `open_world_hint`
- `read_only_hint`
