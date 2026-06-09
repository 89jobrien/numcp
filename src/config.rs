use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Config {
    #[serde(rename = "tool", default)]
    pub tools: Vec<ToolConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ToolConfig {
    pub name: String,
    pub description: String,
    pub handler: PathBuf,
    #[serde(default)]
    pub parameters: IndexMap<String, ParamSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ParamSpec {
    #[serde(rename = "type")]
    pub type_: String,
    pub description: String,
    #[serde(default)]
    pub required: bool,
    #[serde(rename = "enum", default, skip_serializing_if = "Option::is_none")]
    pub enum_: Option<Vec<String>>,
}

impl Config {
    /// Parse a `numcp.toml` from a string.
    pub fn parse(src: &str) -> Result<Self> {
        let config: Config = toml::from_str(src)?;
        config.validate()?;
        Ok(config)
    }

    /// Parse a `numcp.toml` from a file path.
    pub fn from_file(path: &Path) -> Result<Self> {
        let src = std::fs::read_to_string(path)?;
        Self::parse(&src)
    }

    fn validate(&self) -> Result<()> {
        let mut seen = std::collections::HashSet::new();
        for tool in &self.tools {
            if !seen.insert(tool.name.clone()) {
                return Err(Error::DuplicateTool(tool.name.clone()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
[[tool]]
name        = "echo"
description = "Echo input"
handler     = "tools/echo.nu"
"#;

    const FULL: &str = r#"
[[tool]]
name        = "web_search"
description = "Search the web"
handler     = "tools/web_search.nu"

[tool.parameters.query]
type        = "string"
description = "Search terms"
required    = true

[tool.parameters.limit]
type        = "number"
description = "Max results"
required    = false
enum        = ["5", "10", "20"]

[[tool]]
name        = "kubectl"
description = "Run kubectl"
handler     = "tools/kubectl.nu"

[tool.parameters.args]
type        = "array"
description = "kubectl arguments"
required    = true
"#;

    #[test]
    fn parse_minimal_config() {
        let config = Config::parse(MINIMAL).unwrap();
        assert_eq!(config.tools.len(), 1);
        let t = &config.tools[0];
        assert_eq!(t.name, "echo");
        assert_eq!(t.description, "Echo input");
        assert_eq!(t.handler, PathBuf::from("tools/echo.nu"));
        assert!(t.parameters.is_empty());
    }

    #[test]
    fn parse_full_config() {
        let config = Config::parse(FULL).unwrap();
        assert_eq!(config.tools.len(), 2);

        let ws = &config.tools[0];
        assert_eq!(ws.name, "web_search");
        assert_eq!(ws.parameters.len(), 2);

        let query = &ws.parameters["query"];
        assert_eq!(query.type_, "string");
        assert!(query.required);
        assert!(query.enum_.is_none());

        let limit = &ws.parameters["limit"];
        assert!(!limit.required);
        assert_eq!(
            limit.enum_.as_deref(),
            Some(&["5".to_string(), "10".to_string(), "20".to_string()][..])
        );

        let kc = &config.tools[1];
        assert_eq!(kc.name, "kubectl");
        assert_eq!(kc.parameters["args"].type_, "array");
    }

    #[test]
    fn duplicate_tool_name_is_error() {
        let src = r#"
[[tool]]
name = "echo"
description = "first"
handler = "a.nu"

[[tool]]
name = "echo"
description = "second"
handler = "b.nu"
"#;
        let err = Config::parse(src).unwrap_err();
        assert!(matches!(err, crate::error::Error::DuplicateTool(n) if n == "echo"));
    }

    #[test]
    fn empty_config_is_valid() {
        let config = Config::parse("").unwrap();
        assert!(config.tools.is_empty());
    }

    #[test]
    fn invalid_toml_is_error() {
        let err = Config::parse("[[[ not valid toml").unwrap_err();
        assert!(matches!(err, crate::error::Error::Config(_)));
    }
}
