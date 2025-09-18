# rust-mcp-macros.


## mcp_tool Macro

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




**Note**: The following attributes are available only in version `2025_03_26` and later of the MCP Schema, and their values will be used in the [annotations](https://github.com/rust-mcp-stack/rust-mcp-schema/blob/main/src/generated_schema/2025_03_26/mcp_schema.rs#L5557) attribute of the *[Tool struct](https://github.com/rust-mcp-stack/rust-mcp-schema/blob/main/src/generated_schema/2025_03_26/mcp_schema.rs#L5554-L5566).

- `destructive_hint`
- `idempotent_hint`
- `open_world_hint`
- `read_only_hint`





## mcp_elicit Macro

The `mcp_elicit` macro generates implementations for the annotated struct to facilitate data elicitation. It enables struct to generate `ElicitRequestedSchema` and also parsing a map of field names to `ElicitResultContentValue` values back into the struct, supporting both required and optional fields. The generated implementation includes:

- A `message()` method returning the elicitation message as a string.
- A `requested_schema()` method returning an `ElicitRequestedSchema` based on the struct’s JSON schema.
- A `from_content_map()` method to convert a map of `ElicitResultContentValue` values into a struct instance.

### Attributes

- `message` - An optional string (or `concat!(...)` expression) to prompt the user or system for input. Defaults to an empty string if not provided.

### Supported Field Types

- `String`: Maps to `ElicitResultContentValue::String`.
- `bool`: Maps to `ElicitResultContentValue::Boolean`.
- `i32`: Maps to `ElicitResultContentValue::Integer` (with bounds checking).
- `i64`: Maps to `ElicitResultContentValue::Integer`.
- `enum` Only simple enums are supported. The enum must implement the FromStr trait.
- `Option<T>`: Supported for any of the above types, mapping to `None` if the field is missing.


### Usage Example

```rust
use rust_mcp_sdk::macros::{mcp_elicit, JsonSchema};
use rust_mcp_sdk::schema::RpcError;
use std::str::FromStr;

// Simple enum with FromStr trait implemented
#[derive(JsonSchema, Debug)]
pub enum Colors {
    #[json_schema(title = "Green Color")]
    Green,
    #[json_schema(title = "Red Color")]
    Red,
}
impl FromStr for Colors {
    type Err = RpcError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "green" => Ok(Colors::Green),
            "red" => Ok(Colors::Red),
            _ => Err(RpcError::parse_error().with_message("Invalid color".to_string())),
        }
    }
}

// A struct that could be used to send elicit request and get the input from the user
#[mcp_elicit(message = "Please enter your info")]
#[derive(JsonSchema)]
pub struct UserInfo {
    #[json_schema(
        title = "Name",
        description = "The user's full name",
        min_length = 5,
        max_length = 100
    )]
    pub name: String,

    /// Email address of the user
    #[json_schema(title = "Email", format = "email")]
    pub email: Option<String>,

    /// The user's age in years
    #[json_schema(title = "Age", minimum = 15, maximum = 125)]
    pub age: i32,

    /// Is user a student?
    #[json_schema(title = "Is student?", default = true)]
    pub is_student: Option<bool>,

    /// User's favorite color
    pub favorate_color: Colors,
}

    // ....
    // .......
    // ...........

    // send a Elicit Request , ask for UserInfo data and convert the result back to a valid UserInfo instance

    let result: ElicitResult = server
        .elicit_input(UserInfo::message(), UserInfo::requested_schema())
        .await?;

    // Create a UserInfo instance using data provided by the user on the client side
    let user_info = UserInfo::from_content_map(result.content)?;


```

---

<img align="top" src="assets/rust-mcp-stack-icon.png" width="24" style="border-radius:0.2rem;"> Check out [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk), a high-performance, asynchronous toolkit for building MCP servers and clients. Focus on your app's logic while [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk) takes care of the rest!

---
