use std::path::Path;

use nu_engine::eval_block;
use nu_parser::parse;
use nu_protocol::{
    PipelineData, Span, Value as NuValue,
    debugger::WithoutDebug,
    engine::{EngineState, Stack, StateWorkingSet},
};
use serde_json::Value;

use crate::error::{Error, Result};

/// Wraps the Nu engine and runs handler scripts.
pub struct NuExecutor {
    engine_state: EngineState,
}

impl NuExecutor {
    /// Create a new executor with a fresh Nu engine state (includes all builtins).
    pub fn new() -> Result<Self> {
        let engine_state = nu_cmd_lang::create_default_context();
        let engine_state = nu_command::add_shell_command_context(engine_state);
        Ok(NuExecutor { engine_state })
    }

    /// Run a Nu handler script with the given JSON args as `$in`.
    ///
    /// Returns the script's output serialised to a NUON string.
    pub async fn run(&self, handler: &Path, args: Value) -> Result<String> {
        if !handler.exists() {
            return Err(Error::HandlerNotFound {
                tool: handler
                    .file_name()
                    .unwrap_or(handler.as_os_str())
                    .to_string_lossy()
                    .into_owned(),
                path: handler.display().to_string(),
            });
        }

        let src = std::fs::read_to_string(handler)?;
        let engine_state = self.engine_state.clone();

        tokio::task::spawn_blocking(move || run_nu_script(engine_state, &src, args))
            .await
            .map_err(|e| Error::Execution(format!("executor task panicked: {e}")))?
    }
}

/// Convert a `serde_json::Value` into a `nu_protocol::Value`.
fn json_to_nu(v: &Value) -> NuValue {
    let span = Span::unknown();
    match v {
        Value::Null => NuValue::nothing(span),
        Value::Bool(b) => NuValue::bool(*b, span),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                NuValue::int(i, span)
            } else {
                NuValue::float(n.as_f64().unwrap_or(0.0), span)
            }
        }
        Value::String(s) => NuValue::string(s.clone(), span),
        Value::Array(arr) => {
            let vals: Vec<NuValue> = arr.iter().map(json_to_nu).collect();
            NuValue::list(vals, span)
        }
        Value::Object(obj) => {
            let mut record = nu_protocol::Record::new();
            for (k, v) in obj {
                record.push(k.clone(), json_to_nu(v));
            }
            NuValue::record(record, span)
        }
    }
}

/// Parse and evaluate a Nu script string, passing `args` as `$in`.
///
/// Returns the output serialised as a NUON string.
fn run_nu_script(mut engine_state: EngineState, src: &str, args: Value) -> Result<String> {
    // Parse
    let (block, delta) = {
        let mut working_set = StateWorkingSet::new(&engine_state);
        let block = parse(&mut working_set, None, src.as_bytes(), false);

        if let Some(err) = working_set.parse_errors.first() {
            return Err(Error::Execution(format!("parse error: {err}")));
        }
        if let Some(err) = working_set.compile_errors.first() {
            return Err(Error::Execution(format!("compile error: {err:?}")));
        }

        (block, working_set.render())
    };

    engine_state
        .merge_delta(delta)
        .map_err(|e| Error::Execution(format!("merge error: {e}")))?;

    // Evaluate with args as $in
    let mut stack = Stack::new().capture_all();
    let input = PipelineData::value(json_to_nu(&args), None);

    let output = eval_block::<WithoutDebug>(&engine_state, &mut stack, &block, input)
        .map_err(|e| Error::Execution(format!("runtime error: {e}")))?;

    // Serialise output to NUON
    let span = block.span.unwrap_or(Span::unknown());
    let value = output
        .body
        .into_value(span)
        .map_err(|e| Error::Execution(format!("output error: {e}")))?;

    let nuon = nuon::to_nuon(
        &engine_state,
        &value,
        nuon::ToNuonConfig::default()
            .style(nuon::ToStyle::Raw)
            .span(Some(span)),
    )
    .map_err(|e| Error::Execution(format!("serialise error: {e}")))?;

    Ok(nuon)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("numcp_executor_tests");
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_fixture(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
        let p = dir.join(name);
        fs::write(&p, content).unwrap();
        p
    }

    #[tokio::test]
    async fn run_echo_handler_round_trips_args() {
        let dir = fixture_dir();
        let handler = write_fixture(&dir, "echo.nu", "$in | to json");
        let executor = NuExecutor::new().unwrap();
        let args = serde_json::json!({"msg": "hello"});
        let result = executor.run(&handler, args).await.unwrap();
        assert!(
            result.contains("hello"),
            "expected 'hello' in output, got: {result}"
        );
    }

    #[tokio::test]
    async fn run_missing_handler_returns_error() {
        let dir = fixture_dir();
        let executor = NuExecutor::new().unwrap();
        let err = executor
            .run(&dir.join("ghost.nu"), serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::HandlerNotFound { .. }));
    }

    #[tokio::test]
    async fn run_error_handler_returns_err_not_panic() {
        let dir = fixture_dir();
        let handler = write_fixture(&dir, "err.nu", "error make {msg: \"boom\"}");
        let executor = NuExecutor::new().unwrap();
        let result = executor.run(&handler, serde_json::json!({})).await;
        assert!(result.is_err(), "expected Err from error make, got Ok");
    }
}
