use std::path::Path;
use std::time::Instant;

use serde_json::json;

use crate::{config::Config, error::Result, executor::NuExecutor, registry::ToolRegistry};

/// Result of a warm-up run.
#[derive(Debug)]
pub struct WarmResult {
    pub tools: Vec<String>,
    pub duration_ms: u128,
}

/// Load config, build registry, init executor, optionally ping each handler.
///
/// Prints a JSON status line to stderr on completion.
pub async fn warm(config_path: &Path) -> Result<WarmResult> {
    let start = Instant::now();

    let config = Config::from_file(config_path)?;
    let registry = ToolRegistry::load(&config, config_path.parent().unwrap_or(Path::new(".")))?;
    let executor = NuExecutor::new()?;

    let tool_names: Vec<String> = registry
        .list()
        .iter()
        .filter_map(|v| v["name"].as_str().map(str::to_owned))
        .collect();

    // Ping handlers that have no required params.
    for entry in tool_names.iter().filter_map(|name| registry.get(name)) {
        let has_required = entry.parameters.values().any(|p| p.required);
        if !has_required {
            // Ignore ping errors — warm-up is best-effort.
            let _ = executor.run(&entry.handler, json!({})).await;
        }
    }

    let duration_ms = start.elapsed().as_millis();

    let status = json!({
        "tools": tool_names,
        "nu_engine": "ready",
        "duration_ms": duration_ms,
    });
    eprintln!("{status}");

    Ok(WarmResult {
        tools: tool_names,
        duration_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use std::fs;

    fn make_config(dir: &Path, handler_src: &str) -> std::path::PathBuf {
        let handler = dir.join("echo.nu");
        fs::write(&handler, handler_src).unwrap();
        let cfg_path = dir.join("numcp.toml");
        fs::write(
            &cfg_path,
            format!("[[tool]]\nname = \"echo\"\ndescription = \"echo\"\nhandler = \"echo.nu\"\n"),
        )
        .unwrap();
        cfg_path
    }

    #[tokio::test]
    async fn warm_with_valid_config_succeeds() {
        let dir = std::env::temp_dir().join("numcp_warm_tests");
        fs::create_dir_all(&dir).unwrap();
        let cfg = make_config(&dir, "$in | to nuon");
        let result = warm(&cfg).await.unwrap();
        assert!(result.tools.contains(&"echo".to_string()));
    }

    #[tokio::test]
    async fn warm_with_missing_handler_reports_error_not_panic() {
        let dir = std::env::temp_dir().join("numcp_warm_missing");
        fs::create_dir_all(&dir).unwrap();
        let cfg_path = dir.join("numcp.toml");
        fs::write(
            &cfg_path,
            "[[tool]]\nname = \"ghost\"\ndescription = \"ghost\"\nhandler = \"ghost.nu\"\n",
        )
        .unwrap();
        let err = warm(&cfg_path).await.unwrap_err();
        assert!(matches!(err, Error::HandlerNotFound { .. }));
    }
}
