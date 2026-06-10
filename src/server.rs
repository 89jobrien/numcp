use std::sync::Arc;

use rmcp::{
    ServerHandler,
    model::{
        CallToolRequestParams, CallToolResult, Content, ErrorCode, ErrorData, Implementation,
        ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
    },
    service::RequestContext,
};

use crate::{executor::NuExecutor, registry::ToolRegistry};

/// MCP server that exposes Nu handlers as tools.
pub struct NumcpServer {
    registry: Arc<ToolRegistry>,
    executor: Arc<NuExecutor>,
}

impl NumcpServer {
    pub fn new(registry: ToolRegistry, executor: NuExecutor) -> Self {
        Self {
            registry: Arc::new(registry),
            executor: Arc::new(executor),
        }
    }
}

impl NumcpServer {
    fn build_tools_list(&self) -> Vec<Tool> {
        self.registry
            .list()
            .into_iter()
            .map(|v| serde_json::from_value(v).expect("registry produces valid Tool JSON"))
            .collect()
    }

    async fn invoke_tool(
        &self,
        request: CallToolRequestParams,
    ) -> Result<CallToolResult, ErrorData> {
        let tool_entry = self.registry.get(&request.name).ok_or_else(|| {
            ErrorData::new(
                ErrorCode::METHOD_NOT_FOUND,
                format!("tool not found: `{}`", request.name),
                None,
            )
        })?;

        let args = request
            .arguments
            .map(serde_json::Value::Object)
            .unwrap_or(serde_json::Value::Null);

        match self.executor.run(&tool_entry.handler, args).await {
            Ok(nuon) => Ok(CallToolResult::success(vec![Content::text(nuon)])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }
}

impl ServerHandler for NumcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<rmcp::RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: self.build_tools_list(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<rmcp::RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        self.invoke_tool(request).await
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use serde_json::json;

    use crate::{
        config::{Config, ToolConfig},
        executor::NuExecutor,
        registry::ToolRegistry,
    };

    use super::*;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join("numcp_server_tests");
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_handler(dir: &PathBuf, name: &str, src: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, src).unwrap();
        p
    }

    fn make_server(dir: &PathBuf) -> NumcpServer {
        let handler_path = write_handler(dir, "echo.nu", "$in | to nuon");
        let cfg = Config {
            tools: vec![ToolConfig {
                name: "echo".into(),
                description: "echoes input".into(),
                handler: handler_path,
                parameters: Default::default(),
            }],
        };
        let registry = ToolRegistry::load_unchecked(&cfg, &dir);
        let executor = NuExecutor::new().unwrap();
        NumcpServer::new(registry, executor)
    }

    #[test]
    fn get_info_reports_tools_capability() {
        let dir = tmp_dir();
        let server = make_server(&dir);
        let info = server.get_info();
        assert!(info.capabilities.tools.is_some());
        assert_eq!(info.server_info.name, "numcp");
    }

    #[test]
    fn list_tools_returns_registered_tools() {
        let dir = tmp_dir();
        let server = make_server(&dir);
        let tools = server.build_tools_list();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "echo");
    }

    #[tokio::test]
    async fn call_tool_unknown_returns_method_not_found() {
        let dir = tmp_dir();
        let server = make_server(&dir);
        let mut req = CallToolRequestParams::default();
        req.name = "no_such_tool".into();
        let err = server.invoke_tool(req).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn call_tool_echo_round_trips() {
        let dir = tmp_dir();
        let server = make_server(&dir);
        let mut args = serde_json::Map::new();
        args.insert("msg".into(), json!("hello"));
        let mut req = CallToolRequestParams::default();
        req.name = "echo".into();
        req.arguments = Some(args);
        let result = server.invoke_tool(req).await.unwrap();
        assert_eq!(result.is_error, Some(false));
        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("expected text content"),
        };
        assert!(text.contains("hello"), "got: {text}");
    }
}
