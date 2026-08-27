use colored::Colorize;
use rust_mcp_sdk::schema::{CallToolRequestParams, RequestMetaObject};
use rust_mcp_sdk::McpClient;
use rust_mcp_sdk::{error::SdkResult, mcp_client::ClientRuntime};
use serde_json::json;
use std::sync::Arc;

const GREY_COLOR: (u8, u8, u8) = (90, 90, 90);
const HEADER_SIZE: usize = 31;

pub struct InquiryUtils {
    pub client: Arc<ClientRuntime>,
}

impl InquiryUtils {
    fn print_header(&self, title: &str) {
        let pad = ((HEADER_SIZE as f32 / 2.0) + (title.len() as f32 / 2.0)).floor() as usize;
        println!("\n{}", "=".repeat(HEADER_SIZE).custom_color(GREY_COLOR));
        println!("{:>pad$}", title.custom_color(GREY_COLOR));
        println!("{}", "=".repeat(HEADER_SIZE).custom_color(GREY_COLOR));
    }

    fn print_list(&self, list_items: Vec<(String, String)>) {
        list_items.iter().enumerate().for_each(|(index, item)| {
            println!("{}. {}: {}", index + 1, item.0.yellow(), item.1.cyan());
        });
    }

    pub fn print_server_info(&self) {
        self.print_header("Server info");
        println!(
            "{} {}",
            "Use server/discover to query server identity and capabilities".bold(),
            "".cyan()
        );
    }

    pub fn print_server_capabilities(&self) {
        self.print_header("Capabilities");
        println!(
            "{}",
            "Use server/discover to query server capabilities".cyan()
        );
    }

    pub async fn print_tools(&self) -> SdkResult<()> {
        self.print_header("Tools");
        let result = self.client.request_tool_list(None).await?;
        let tool_list = result.tools;
        if tool_list.is_empty() {
            println!("{}", "No tools found!".red());
            return Ok(());
        }
        let items: Vec<(String, String)> = tool_list
            .iter()
            .map(|tool| {
                (
                    tool.name.clone(),
                    tool.description.clone().unwrap_or_default(),
                )
            })
            .collect();
        self.print_list(items);
        Ok(())
    }

    pub async fn print_prompts(&self) -> SdkResult<()> {
        self.print_header("Prompts");
        let result = self.client.request_prompt_list(None).await?;
        let prompt_list = result.prompts;
        if prompt_list.is_empty() {
            println!("{}", "No prompts found!".red());
            return Ok(());
        }
        let items: Vec<(String, String)> = prompt_list
            .iter()
            .map(|prompt| {
                (
                    prompt.name.clone(),
                    prompt.description.clone().unwrap_or_default(),
                )
            })
            .collect();
        self.print_list(items);
        Ok(())
    }

    pub async fn print_resources(&self) -> SdkResult<()> {
        self.print_header("Resources");
        let result = self.client.request_resource_list(None).await?;
        let resource_list = result.resources;
        if resource_list.is_empty() {
            println!("{}", "No resources found!".red());
            return Ok(());
        }
        let items: Vec<(String, String)> = resource_list
            .iter()
            .map(|res| {
                (
                    res.name.clone(),
                    res.description.clone().unwrap_or_default(),
                )
            })
            .collect();
        self.print_list(items);
        Ok(())
    }

    pub async fn print_resource_templates(&self) -> SdkResult<()> {
        self.print_header("Resource Templates");
        let result = self.client.request_resource_template_list(None).await?;
        let templates = result.resource_templates;
        if templates.is_empty() {
            println!("{}", "No resource templates found!".red());
            return Ok(());
        }
        let items: Vec<(String, String)> = templates
            .iter()
            .map(|rt| (rt.name.clone(), rt.description.clone().unwrap_or_default()))
            .collect();
        self.print_list(items);
        Ok(())
    }

    pub async fn call_test_tool(&self, a: i32, b: i32) -> SdkResult<()> {
        println!(
            "{}",
            format!("\nCalling the \"get-sum\" tool with {a} and {b} ...").magenta()
        );

        let params = json!({
            "a": a,
            "b": b
        })
        .as_object()
        .unwrap()
        .clone();

        let result = self
            .client
            .request_tool_call(CallToolRequestParams {
                name: "get-sum".to_string(),
                arguments: Some(params),
                meta: RequestMetaObject::default(),
                input_responses: None,
                request_state: None,
            })
            .await?;

        let result_content = result.content.first().unwrap().as_text_content()?;
        println!("{}", result_content.text.green());

        Ok(())
    }
}
