use rust_mcp_macros::JsonSchema;
use rust_mcp_schema::RpcError;
use std::str::FromStr;

#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, JsonSchema)]
/// Represents a text replacement operation.
pub struct EditOperation {
    /// Text to search for - must match exactly.
    #[serde(rename = "oldText")]
    pub old_text: String,
    #[serde(rename = "newText")]
    /// Text to replace the matched text with.
    pub new_text: String,
}

#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug, JsonSchema)]
pub struct EditFileTool {
    /// The path of the file to edit.
    pub path: String,

    /// The list of edit operations to apply.
    pub edits: Vec<EditOperation>,
    /// Preview changes using git-style diff format without applying them.
    #[serde(
        rename = "dryRun",
        default,
        skip_serializing_if = "std::option::Option::is_none"
    )]
    pub dry_run: Option<bool>,
}

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
