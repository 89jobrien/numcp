use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde_json::{Value, json};

use crate::{
    config::{Config, ParamSpec},
    error::{Error, Result},
};

/// A validated, indexed set of tools ready to serve via MCP.
#[derive(Debug, Clone)]
pub struct ToolRegistry {
    tools: IndexMap<String, RegisteredTool>,
}

#[derive(Debug, Clone)]
pub struct RegisteredTool {
    pub name: String,
    pub description: String,
    pub handler: PathBuf,
    pub parameters: IndexMap<String, ParamSpec>,
}

impl ToolRegistry {
    /// Load from a `Config`, resolving handler paths relative to `base_dir`.
    /// Returns an error if any handler file does not exist.
    pub fn load(config: &Config, base_dir: &Path) -> Result<Self> {
        let mut tools = IndexMap::new();
        for tool in &config.tools {
            let handler = base_dir.join(&tool.handler);
            if !handler.exists() {
                return Err(Error::HandlerNotFound {
                    tool: tool.name.clone(),
                    path: handler.display().to_string(),
                });
            }
            tools.insert(
                tool.name.clone(),
                RegisteredTool {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    handler,
                    parameters: tool.parameters.clone(),
                },
            );
        }
        Ok(Self { tools })
    }

    /// Load from a `Config` without validating handler paths exist.
    /// Useful in tests where handler files are not on disk.
    #[cfg(test)]
    pub fn load_unchecked(config: &Config, base_dir: &Path) -> Self {
        let mut tools = IndexMap::new();
        for tool in &config.tools {
            tools.insert(
                tool.name.clone(),
                RegisteredTool {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    handler: base_dir.join(&tool.handler),
                    parameters: tool.parameters.clone(),
                },
            );
        }
        Self { tools }
    }

    /// Return all tools as MCP `tools/list` schema entries.
    pub fn list(&self) -> Vec<Value> {
        self.tools.values().map(|t| tool_to_mcp_schema(t)).collect()
    }

    /// Look up a tool by name.
    pub fn get(&self, name: &str) -> Option<&RegisteredTool> {
        self.tools.get(name)
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

fn tool_to_mcp_schema(tool: &RegisteredTool) -> Value {
    let properties: Value = tool
        .parameters
        .iter()
        .map(|(k, v)| {
            let mut prop = json!({
                "type": v.type_,
                "description": v.description,
            });
            if let Some(enum_vals) = &v.enum_ {
                prop["enum"] = json!(enum_vals);
            }
            (k.clone(), prop)
        })
        .collect::<serde_json::Map<_, _>>()
        .into();

    let required: Vec<&String> = tool
        .parameters
        .iter()
        .filter(|(_, v)| v.required)
        .map(|(k, _)| k)
        .collect();

    json!({
        "name": tool.name,
        "description": tool.description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::path::PathBuf;

    fn two_tool_config() -> Config {
        Config::parse(
            r#"
[[tool]]
name        = "echo"
description = "Echo input"
handler     = "tools/echo.nu"

[tool.parameters.msg]
type        = "string"
description = "Message to echo"
required    = true

[[tool]]
name        = "noop"
description = "Do nothing"
handler     = "tools/noop.nu"
"#,
        )
        .unwrap()
    }

    #[test]
    fn list_returns_all_registered_tools() {
        let config = two_tool_config();
        let reg = ToolRegistry::load_unchecked(&config, &PathBuf::from("/base"));
        let list = reg.list();
        assert_eq!(list.len(), 2);
        let names: Vec<&str> = list.iter().map(|v| v["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"echo"));
        assert!(names.contains(&"noop"));
    }

    #[test]
    fn get_known_tool_returns_some() {
        let config = two_tool_config();
        let reg = ToolRegistry::load_unchecked(&config, &PathBuf::from("/base"));
        let tool = reg.get("echo").unwrap();
        assert_eq!(tool.name, "echo");
    }

    #[test]
    fn get_unknown_tool_returns_none() {
        let config = two_tool_config();
        let reg = ToolRegistry::load_unchecked(&config, &PathBuf::from("/base"));
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn load_resolves_handler_relative_to_base_dir() {
        let config = two_tool_config();
        let base = PathBuf::from("/some/base");
        let reg = ToolRegistry::load_unchecked(&config, &base);
        assert_eq!(
            reg.get("echo").unwrap().handler,
            PathBuf::from("/some/base/tools/echo.nu")
        );
    }

    #[test]
    fn load_fails_when_handler_missing() {
        let config = two_tool_config();
        // Use load (not load_unchecked) with a non-existent base
        let err = ToolRegistry::load(&config, &PathBuf::from("/nonexistent/base")).unwrap_err();
        assert!(matches!(err, Error::HandlerNotFound { .. }));
    }

    #[test]
    fn mcp_schema_includes_required_fields() {
        let config = two_tool_config();
        let reg = ToolRegistry::load_unchecked(&config, &PathBuf::from("/base"));
        let schema = reg
            .list()
            .into_iter()
            .find(|v| v["name"] == "echo")
            .unwrap();
        let required = schema["inputSchema"]["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "msg"));
    }

    #[test]
    fn empty_registry_list_is_empty() {
        let config = Config::parse("").unwrap();
        let reg = ToolRegistry::load_unchecked(&config, &PathBuf::from("/base"));
        assert!(reg.list().is_empty());
    }
}
