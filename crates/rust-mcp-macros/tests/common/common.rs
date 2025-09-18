use std::str::FromStr;

use rust_mcp_macros::JsonSchema;
use rust_mcp_schema::RpcError;

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
