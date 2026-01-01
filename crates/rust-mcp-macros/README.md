# rust-mcp-macros

`rust-mcp-macros` provides procedural macros for the [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk) ecosystem. These macros simplify the generation of `tools` and `elicitation` schemas compatible with the latest MCP protocol specifications.


The available macros are:

[mcp_tool](#mcp_tool-macro): Generates a [rust_mcp_schema::Tool](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/struct.Tool.html) instance from a struct.
[mcp_elicit](#mcp_elicit): Generates elicitation logic for gathering user input based on a struct's schema, supporting [Form](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/struct.ElicitRequestFormParams.html) and [URL](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/struct.ElicitRequestUrlParams.html) modes.
[derive(JsonSchema)]: Derives a JSON Schema representation for structs and enums, used by the other macros for schema generation.

These macros rely on [rust_mcp_schema](https://crates.io/crates/rust-mcp-schema) and serde_json for schema handling.


## ➡️ mcp_tool Macro
A procedural macro to generate a [rust_mcp_schema::Tool](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/struct.Tool.html) instance from a struct. The struct must derive **JsonSchema**.


### Generated methods:

- `tool_name()`: Returns the tool's name.
- `tool()`: Returns a [rust_mcp_schema::Tool](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/struct.Tool.html) with name, description, input schema, and optional metadata/annotations.
- `request_params()`: Returns a [CallToolRequestParams](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/struct.CallToolRequestParams.html) pre-initialized with the tool's name, ready for building a tool call via the builder pattern.


### Attributes

- `name`: Required, non-empty string for the tool's name.
- `description`: Required, a full and detailed description of the tool’s functionality.
- `title`: Optional human readable title for the tools.
- `description` - A description of the tool (required, non-empty string).
- `meta` - An optional JSON string that provides additional metadata for the tool.
- `execution`: Optional, controls task support. Accepted values are "required", "optional", and "forbidden".
- `icons`: Optional array of icons with src (required), mime_type, sizes (array of strings), theme ("light" or "dark").
- `destructive_hint` – Optional boolean, indicates whether the tool may make destructive changes to its environment.
- `idempotent_hint` – Optional boolean, indicates whether repeated calls with the same input have the same effect.
- `open_world_hint` – Optional boolean, indicates whether the tool can interact with external or unknown entities.
- `read_only_hint` – Optional boolean, indicates whether the tool makes no modifications to its environment.


### Usage Example

```rust
use rust_mcp_macros::{mcp_tool, JsonSchema};
use rust_mcp_schema::Tool;
#[mcp_tool(
    name = "write_file",
    title = "Write File Tool",
    description = "Create or overwrite a file with content.",
    destructive_hint = false,
    idempotent_hint = false,
    open_world_hint = false,
    read_only_hint = false,
    execution(task_support = "optional"),
    icons = [
        (src = "https:/mywebsite.com/write.png", mime_type = "image/png", sizes = ["128x128"], theme = "light"),
        (src = "https:/mywebsite.com/write_dark.svg", mime_type = "image/svg+xml", sizes = ["64x64","128x128"], theme = "dark")
    ],
    meta = r#"{"key": "value"}"#
)]
#[derive(JsonSchema)]
pub struct WriteFileTool {
    /// The target file's path.
    pub path: String,
    /// The string content to be written to the file
    pub content: String,
}

WriteFileTool::request_params().with_arguments(
    json!({"path":"./test.txt","content":"hello tool"})
        .as_object()
        .unwrap()
        .clone(),
)

// send a call_tool requeest:
let result = client.request_tool_call( WriteFileTool::request_params().with_arguments(
    json!({"path":"./test.txt","content":"hello tool"}).as_object().unwrap().clone(),
))?;

// Handle ListToolsRequest, return list of available tools as ListToolsResult
async fn handle_list_tools_request(
    &self,
    request: Option<PaginatedRequestParams>,
    runtime: Arc<dyn McpServer>,
) -> std::result::Result<ListToolsResult, RpcError> {
    Ok(ListToolsResult {
        meta: None,
        next_cursor: None,
        tools: vec![WriteFileTool::tool()],
    })
}

```


## ➡️ mcp_elicit Macro

The `mcp_elicit` macro generates implementations for eliciting user input based on the struct's schema. The struct must derive **JsonSchema**. It supports two modes: **form** (default) for schema-based forms and **url** for redirecting the user to an external URL to collect input.


### Generated methods:

- `message()`: Returns the elicitation message.
- `elicit_request_params(elicitation_id)`: Returns [ElicitRequestParams](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/struct.ElicitRequestUrlParams.html) (FormParams or UrlParams based on mode).
- `from_elicit_result_content(content)`: Parses user input back into the struct.


### Attributes

- `message` : Optional string (or concat!(...)), defaults to empty.
- `mode`: Optional, elicitation mode ("form"|"URL), defaults to form.
- `url` = "https://example.com/form": Required if mode = url.

### Supported Field Types

- `String`: Maps to [ElicitResultContentPrimitive::String](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/enum.ElicitResultContentPrimitive.html).
- `bool`: Maps to [ElicitResultContentPrimitive::Boolean](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/enum.ElicitResultContentPrimitive.html).
- `i32`: Maps to [ElicitResultContentPrimitive::Integer](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/enum.ElicitResultContentPrimitive.html) (with bounds checking).
- `i64`: Maps to [ElicitResultContentPrimitive::Integer](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/enum.ElicitResultContentPrimitive.html).
- `Vec<String>`: Maps to [ElicitResultContent::StringArray](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/enum.ElicitResultContent.html).
- `Option<T>`: Supported for any of the above types, mapping to `None` if the field is missing.


### Usage Example (Form Mode)

```rust
    #[mcp_elicit(message = "Please enter your info", mode = form)]
    #[derive(JsonSchema)]
    pub struct UserInfo {
        #[json_schema(title = "Name", min_length = 5, max_length = 100)]
        pub name: String,
        #[json_schema(title = "Email", format = "email")]
        pub email: Option<String>,
        #[json_schema(title = "Age", minimum = 15, maximum = 125)]
        pub age: i32,
        #[json_schema(title = "Tags")]
        pub tags: Vec<String>,
    }

    // Sends a request to the client asking the user to provide input
    let result: ElicitResult = server.request_elicitation(UserInfo::elicit_request_params()).await?;

    // Convert result.content into a UserInfo instance
    let user_info = UserInfo::from_elicit_result_content(result.content)?; 
    
    println!("name: {}", user_info.name);
    println!("age: {}", user_info.age);
    println!("email: {}",user.email.clone().unwrap_or("not provider".into()));
    println!("tags: {}", user_info.tags.join(","));    

```


### Usage Example (URL Mode)

```rust
#[mcp_elicit(message = "Complete the form", mode = url, url = "https://example.com/form")]
    #[derive(JsonSchema)]
    pub struct UserInfo {
        #[json_schema(title = "Name", min_length = 5, max_length = 100)]
        pub name: String,
        #[json_schema(title = "Email", format = "email")]
        pub email: Option<String>,
        #[json_schema(title = "Age", minimum = 15, maximum = 125)]
        pub age: i32,
        #[json_schema(title = "Tags")]
        pub tags: Vec<String>,
    }

    let elicit_url = UserInfo::elicit_url_params("elicit_10".into());
    
    // Sends a request to the client asking the user to provide input
    let result: ElicitResult = server.request_elicitation(UserInfo::elicit_request_params()).await?;

    // Convert result.content into a UserInfo instance
    let user_info = UserInfo::from_elicit_result_content(result.content)?; 
    
    println!("name: {}", user_info.name);
    println!("age: {}", user_info.age);
    println!("email: {}", user_info.email.unwrap_or_default();
    println!("tags: {}", user_info.tags.join(","));     
```


## ➡️ mcp_resource Macro

A procedural macro attribute that generates utility methods for declaring static resources. It produces a fully populated [rust_mcp_schema::Resource](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/struct.Resource.html) instance from compile-time metadata.
Useful for declaring files, images, documents, or any other static asset that your MCP server wants to expose (e.g., in ListResources responses).

Generated methods

- `resource_name()` → &'static str: Returns the resource name.
- `resource_uri()` → &'static str: Returns the resource URI.
- `resource()` → [rust_mcp_schema::Resource](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/struct.Resource.html) Constructs and returns the complete Resource struct.

### Attributes

- `name` : Unique identifier for the resource. Must be non-empty.
- `description` : Human-readable description of what the resource is.
- `title` : Display title
- `meta` : Arbitrary metadata. Must be a valid JSON object string.
- `mime_type` : MIME type of the resource (e.g., "image/png").
- `size` : Size in bytes.
- `uri` : Publicly accessible URI for the resource.
- `audience` : Intended audiences roles (e.g., ["user", "assistant"]). Resource will use them in the resource annotation and [rust_mcp_schema::Role](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/enum.Role.html).
- `icons` :  icons (same format as mcp_tool icons ^^).


### Usage Example


```rs
use rust_mcp_macros::mcp_resource;

#[mcp_resource(
    name = "company-logo",
    description = "Official high-resolution company logo",
    title = "Company Logo",
    mime_type = "image/png",
    size = 1024,
    uri = "https://example.com/assets/logo.png",
    audience = ["user", "assistant"],
    meta = r#"{"license": "proprietary", "author": "Design Team"}"#,
    icons = [
        ( src = "logo-192.png", sizes = ["192x192"], mime_type = "image/png" ),
        ( src = "logo-512.png", sizes = ["512x512"], mime_type = "image/png" )
    ]
)]
pub struct CompanyLogo{};

// In your server handler:
async fn handle_list_resources(
    &self,
    _request: Option<PaginatedRequestParams>,
) -> Result<ListResourcesResult, RpcError> {
    Ok(ListResourcesResult {
        meta: None,
        next_cursor: None,
        resources: vec![CompanyLogo::resource()],
    })
}

// Usage elsewhere:
assert_eq!(CompanyLogo::resource_name(), "company-logo");
assert_eq!(CompanyLogo::resource_uri(), "https://example.com/assets/logo.png");

let res = CompanyLogo::resource();
assert_eq!(res.mime_type.unwrap(), "image/png");
assert_eq!(res.size.unwrap(), 1024);
assert!(res.icons.len() == 2);
```


## ➡️ mcp_resource_template Macro

A procedural macro attribute that generates utility methods for declaring template description for parameterized resources available on the server. It produces a fully populated [rust_mcp_schema::ResourceTemplate](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/struct.ResourceTemplate.html) instance from compile-time metadata.
Resource templates allow servers to expose parameterized resources

Generated methods

- `resource_template_name()` → &'static str: Returns the resource template name.
- `resource_template_uri()` → &'static str: Returns the resource template URI .
- `resource_template()` → [rust_mcp_schema::ResourceTemplate](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/struct.ResourceTemplate.html) Constructs and returns the complete ResourceTemplate struct.

### Attributes

- `name` : Unique identifier for the resource. Must be non-empty.
- `description` : Human-readable description of what the resource is.
- `title` : Display title
- `meta` : Arbitrary metadata. Must be a valid JSON object string.
- `mime_type` : MIME type of the resource (e.g., "image/png").
- `uri_template` : Publicly accessible URI for the resource.
- `audience` : Intended audiences roles (e.g., ["user", "assistant"]). Resource will use them in the resource annotation and [rust_mcp_schema::Role](https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/enum.Role.html).
- `icons` :  icons (same format as mcp_tool icons ^^).

---

<img align="top" src="assets/rust-mcp-stack-icon.png" width="24" style="border-radius:0.2rem;"> Check out [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk), a high-performance, asynchronous toolkit for building MCP servers and clients. Focus on your app's logic while [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk) takes care of the rest!

---
