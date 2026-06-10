/// Full MCP round-trip integration test.
///
/// Spawns `numcp serve` as a child process, connects via rmcp stdio client,
/// and exercises `tools/list` + `tools/call`.
///
/// Gate: set `NUMCP_INTEGRATION=1` to run.
use std::path::PathBuf;

use rmcp::{ServiceExt, model::CallToolRequestParams, transport::child_process::TokioChildProcess};
use tokio::process::Command;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn numcp_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_numcp"))
}

#[tokio::test]
#[ignore = "integration — set NUMCP_INTEGRATION=1 and run with cargo test --test round_trip"]
async fn full_round_trip_echo_tool() {
    if std::env::var("NUMCP_INTEGRATION").as_deref() != Ok("1") {
        eprintln!("skipping integration test (NUMCP_INTEGRATION != 1)");
        return;
    }

    let config = fixtures_dir().join("echo.toml");
    let bin = numcp_bin();

    let mut cmd = Command::new(&bin);
    cmd.arg("serve")
        .arg("--config")
        .arg(&config)
        .current_dir(fixtures_dir());
    let transport = TokioChildProcess::new(cmd).expect("failed to spawn numcp");

    let mut client = ().serve(transport).await.expect("client handshake failed");

    // tools/list — echo must be present
    let list = client
        .peer()
        .list_tools(None)
        .await
        .expect("list_tools failed");
    assert!(
        list.tools.iter().any(|t| t.name.as_ref() == "echo"),
        "expected echo tool in list; got {:?}",
        list.tools.iter().map(|t| &t.name).collect::<Vec<_>>()
    );

    // tools/call — echo {msg: "hello"} → output contains "hello"
    let mut args = serde_json::Map::new();
    args.insert("msg".into(), serde_json::json!("hello"));
    let mut req = CallToolRequestParams::default();
    req.name = "echo".into();
    req.arguments = Some(args);

    let result = client
        .peer()
        .call_tool(req)
        .await
        .expect("call_tool failed");
    assert_ne!(result.is_error, Some(true), "tool returned error");
    let text = result.content.iter().find_map(|c| {
        if let rmcp::model::RawContent::Text(t) = &c.raw {
            Some(t.text.clone())
        } else {
            None
        }
    });
    let text = text.expect("expected text content in result");
    assert!(
        text.contains("hello"),
        "expected 'hello' in output; got: {text}"
    );

    client.cancel().await.ok();
}
