#[macro_export]
/// Generates an enum representing a toolbox with mcp tool variants and associated functionality.
///
/// **Note:** The macro assumes that tool types provided are annotated with the mcp_tool() macro.
///
/// This macro creates:
/// - An enum with the specified name containing variants for each mcp tool
/// - A `tools()` function returning a vector of supported tools
/// - A `TryFrom<CallToolRequestParams>` implementation for converting requests to tool instances
///
/// # Arguments
/// * `$enum_name` - The name to give the generated enum
/// * `[$($tool:ident),*]` - A comma-separated list of tool types to include in the enum
///
///
/// # Example
/// ```ignore
/// tool_box!(FileSystemTools, [ReadFileTool, EditFileTool, SearchFilesTool]);
/// // Creates:
/// // pub enum FileSystemTools {
/// //     ReadFileTool(ReadFileTool),
/// //     EditFileTool(EditFileTool),
/// //     SearchFilesTool(SearchFilesTool),
/// // }
/// // pub fn tools() -> Vec<Tool> {
/// //     vec![ReadFileTool::tool(), EditFileTool::tool(), SearchFilesTool::tool()]
/// // }
///
/// // impl TryFrom<CallToolRequestParams> for FileSystemTools {
/// //  //.......
/// // }
macro_rules! tool_box {
    ($enum_name:ident, [$($tool:ident),*]) => {
        #[derive(Debug)]
        pub enum $enum_name {
            $(
                // Just create enum variants for each tool
                $tool($tool),
            )*
        }

        /// Returns the name of the tool as a String
        impl $enum_name {
            pub fn tool_name(&self) -> String {
                match self {
                    $(
                        $enum_name::$tool(_) => $tool::tool_name(),
                    )*
                }
            }

            /// Returns a vector containing instances of all supported tools
            pub fn tools() -> Vec<rust_mcp_schema::Tool> {
                vec![
                    $(
                        $tool::tool(),
                    )*
                ]
            }
        }




        impl TryFrom<rust_mcp_schema::CallToolRequestParams> for $enum_name {
            type Error = rust_mcp_schema::schema_utils::CallToolError;

            /// Attempts to convert a tool request into the appropriate tool variant
            fn try_from(value: rust_mcp_schema::CallToolRequestParams) -> Result<Self, Self::Error> {
                let v = serde_json::to_value(value.arguments.unwrap())
                .map_err(rust_mcp_schema::schema_utils::CallToolError::new)?;
                    match value.name {
                        $(
                            name if name == $tool::tool_name().as_str() => {
                                Ok(Self::$tool(serde_json::from_value(v).map_err(rust_mcp_schema::schema_utils::CallToolError::new)?))
                            }
                        )*
                        _ => {
                               Err(
                                rust_mcp_schema::schema_utils::CallToolError::unknown_tool(value.name.to_string())
                              )
                        }
                    }

            }
        }
    }
}
