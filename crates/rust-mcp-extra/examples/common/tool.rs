use rust_mcp_sdk::{
    auth::AuthInfo,
    macros::{mcp_tool, JsonSchema},
    schema::{schema_utils::CallToolError, CallToolResult, TextContent},
};

//*******************************//
//  Show Authentication Info  //
//*******************************//
#[mcp_tool(
    name = "show_auth_info",
    description = "Shows current user authentication info in json format"
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema, Default)]
pub struct ShowAuthInfo {}
impl ShowAuthInfo {
    pub fn call_tool(&self, auth_info: Option<AuthInfo>) -> Result<CallToolResult, CallToolError> {
        let auth_info_json = serde_json::to_string_pretty(&auth_info).map_err(|err| {
            CallToolError::from_message(format!("Undable to display auth info as string :{err}"))
        })?;
        Ok(CallToolResult::text_content(vec![TextContent::from(
            auth_info_json,
        )]))
    }
}
