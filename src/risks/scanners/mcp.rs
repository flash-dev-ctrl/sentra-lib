use std::pin::Pin;
use std::sync::Arc;

use super::utils::build_prompt_input;
use crate::SentraResult;
use crate::interfaces::{McpData, McpToolDef, Scanner};
use crate::risks::checkers::{CheckOutput, RiskChecker};
use crate::utils::{hydrate_mcp_tools, sanitize_url_credentials};

const MCP_SCANNER_ID: &str = "mcp-scanner";

pub(crate) struct McpScanner {
    checker: Arc<RiskChecker>,
}

impl McpScanner {
    pub(crate) fn new(checker: Arc<RiskChecker>) -> Self {
        Self { checker }
    }
}

impl Scanner<McpData> for McpScanner {
    fn id(&self) -> &str {
        MCP_SCANNER_ID
    }

    fn scan_asset<'a>(
        &'a self,
        asset: &'a McpData,
    ) -> Pin<Box<dyn std::future::Future<Output = SentraResult<CheckOutput>> + Send + 'a>> {
        Box::pin(async move {
            match build_tools_prompt_input(asset) {
                Some(input) => self.checker.scan(&[input]).await,
                None => self.checker.scan(&[]).await,
            }
        })
    }
}

fn build_tools_prompt_input(asset: &McpData) -> Option<crate::interfaces::CheckInput> {
    let url = asset.url.as_deref()?;
    let hydrated;
    let tools = if asset.tools.is_empty() {
        hydrated = hydrate_mcp_tools(asset.clone());
        &hydrated.tools
    } else {
        &asset.tools
    };
    if tools.is_empty() {
        return None;
    }
    let content = format_tools_prompt(asset, url, tools);
    Some(build_prompt_input(
        &format!("mcp:{}:tools", asset.name),
        &content,
    ))
}

fn format_tools_prompt(asset: &McpData, url: &str, tools: &[McpToolDef]) -> String {
    let mut content = format!(
        "MCP server: {}\nURL: {}\n\nTools:",
        asset.name,
        sanitize_url_credentials(url)
    );
    for tool in tools {
        content.push_str("\n- ");
        content.push_str(&tool.name);
        if let Some(description) = tool.description.as_deref().map(str::trim)
            && !description.is_empty()
        {
            content.push_str(": ");
            content.push_str(description);
        }
    }
    content
}
