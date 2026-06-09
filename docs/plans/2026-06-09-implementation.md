# numcp Implementation Plan

**Date**: 2026-06-09
**Design ref**: [nu_libs/docs/plans/2026-06-09-baml-agent-daemon.md](https://github.com/89jobrien/nu_libs/blob/main/docs/plans/2026-06-09-baml-agent-daemon.md)
**Status**: planning

## Goal

`numcp` is an MCP server that exposes Nushell tool handlers as MCP tools. An LLM agent
daemon (e.g. `nu-ai-daemon`) connects via stdio MCP transport, calls `tools/list` to
discover available tools, and calls `tools/call` to execute them. Tool execution runs
inside a Nu engine (via `nu-mcp`).

## Architecture

```
nu-ai-daemon (MCP client)
  │  stdio transport (rmcp)
  ▼
numcp serve
  ├── config loader       — reads numcp.toml, discovers tool definitions
  ├── tool registry       — maps tool names → schemas + handler paths
  ├── MCP server (rmcp)   — implements initialize / tools/list / tools/call
  └── Nu executor (nu-mcp) — runs handler.nu scripts with $in = tool args
```

## Module Layout

```
src/
  main.rs          — CLI entrypoint (clap: serve subcommand)
  config.rs        — Config structs, TOML parsing, discovery path resolution
  registry.rs      — ToolRegistry: load from config, validate, index by name
  server.rs        — MCP server impl (rmcp): initialize, tools/list, tools/call
  executor.rs      — Nu engine wrapper: run handler.nu with args, capture output
  error.rs         — thiserror error types
  warm.rs          — warm-up logic: load config, prime Nu engine, verify tools

docs/plans/
  2026-06-09-implementation.md   — this file

tests/
  conformance_mcp_server.rs      — MCP protocol conformance suite (reusable)
  integration/
    round_trip.rs                — daemon → numcp → Nu handler → result

fuzz/
  fuzz_targets/
    fuzz_tool_toml.rs            — arbitrary bytes into tool.toml parser
    fuzz_mcp_tools_call.rs       — arbitrary JSON into tools/call handler
```

## Implementation Tasks

Tasks are ordered by dependency. Complete each before starting the next.

---

### Phase 1: Config and Registry

**P1-1 — `config.rs`: define config structs and parse `numcp.toml`**

```toml
# numcp.toml shape
[[tool]]
name        = "web_search"
description = "Search the web"
handler     = "tools/web_search.nu"

[tool.parameters.query]
type        = "string"
description = "Search terms"
required    = true
```

Structs:

- `Config { tools: Vec<ToolConfig> }`
- `ToolConfig { name, description, handler: PathBuf, parameters: IndexMap<String, ParamSpec> }`
- `ParamSpec { type_: String, description: String, required: bool, enum_: Option<Vec<String>> }`

Tests (unit):

- `parse_minimal_config` — single tool, no parameters
- `parse_full_config` — multiple tools, all parameter fields
- `missing_handler_path_is_error` — handler file not found → error at load time
- `duplicate_tool_name_is_error` — two `[[tool]]` blocks with same name → error

Property tests:

- Round-trip: serialise → deserialise → same struct

---

**P1-2 — `registry.rs`: ToolRegistry**

Loads from `Config`, validates handler paths exist, builds an index by tool name.
Produces the MCP `tools/list` response shape.

```rust
pub struct ToolRegistry {
    tools: IndexMap<String, RegisteredTool>,
}

impl ToolRegistry {
    pub fn load(config: &Config, base_dir: &Path) -> Result<Self>
    pub fn list(&self) -> Vec<McpToolSchema>
    pub fn get(&self, name: &str) -> Option<&RegisteredTool>
}
```

Tests (unit):

- `list_returns_all_registered_tools`
- `get_unknown_tool_returns_none`
- `load_resolves_handler_relative_to_config_dir`

---

### Phase 2: Nu Executor

**P2-1 — `executor.rs`: run a Nu handler script**

Wraps `nu-mcp` engine. Takes a `handler: &Path` and `args: serde_json::Value` (a JSON
object), sets `$in = args` in the Nu scope, runs the script, captures stdout as a string.

```rust
pub struct NuExecutor { /* nu-mcp engine handle */ }

impl NuExecutor {
    pub fn new() -> Result<Self>
    pub async fn run(&self, handler: &Path, args: Value) -> Result<String>
}
```

Tests (unit, using a real `.nu` script fixture):

- `run_echo_handler` — handler returns `$in | to json`, verify round-trip
- `run_error_handler` — handler calls `error make`, verify error is captured not panicked
- `run_missing_handler` — non-existent path → error before execution

Property tests:

- `run_with_arbitrary_json_object_does_not_panic` — proptest over random `serde_json::Value::Object`

Fuzz target (`fuzz_mcp_tools_call.rs`):

- Arbitrary bytes parsed as UTF-8 → fed as args to a known-safe echo handler
- Assert: no panic; result is either Ok or a well-formed Err

---

### Phase 3: MCP Server

**P3-1 — `server.rs`: implement MCP protocol over rmcp**

Implements the three required MCP methods:

| Method       | Behaviour                                                   |
| ------------ | ----------------------------------------------------------- |
| `initialize` | Return server info and capabilities                         |
| `tools/list` | Return `ToolRegistry::list()`                               |
| `tools/call` | Look up tool → `NuExecutor::run()` → return result or error |

`tools/call` error mapping:

- Tool not found → JSON-RPC error `-32601` (method not found)
- Nu execution error → JSON-RPC error `-32000` (server error) with message

Tests (conformance, `tests/conformance_mcp_server.rs`):

```rust
fn assert_mcp_server_contract<S: McpServer>(server: S) {
    // initialize returns server name and version
    // tools/list after warm-up returns non-empty list
    // tools/call with unknown tool returns error code -32601
    // tools/call with valid tool + valid args returns non-null result
    // tools/call with invalid args returns error, does not panic
}

#[tokio::test]
async fn numcp_server_satisfies_mcp_contract() {
    let server = build_test_server().await;
    assert_mcp_server_contract(server).await;
}
```

Fuzz target (`fuzz_tool_toml.rs`):

- Arbitrary bytes into `config::parse_tool_config`
- Assert: no panic; if Ok, `name` is non-empty and `parameters` has no duplicate keys

---

### Phase 4: Warm-up

**P4-1 — `warm.rs`: warm-up routine**

Called at startup (non-blocking) and via future `--warm` CLI flag. Steps:

1. Load and validate config
2. Load `ToolRegistry`
3. Initialise `NuExecutor` (primes Nu engine)
4. Run each registered handler with an empty `{}` args ping (optional, skipped if handler
   declares `required` params)
5. Print status to stderr

Output (stderr JSON):

```json
{ "tools": ["web_search", "kubectl"], "nu_engine": "ready", "duration_ms": 180 }
```

Tests (unit):

- `warm_with_valid_config_succeeds`
- `warm_with_missing_handler_reports_error_not_panic`

---

### Phase 5: Integration

**P5-1 — `tests/integration/round_trip.rs`**

Spawns `numcp serve` as a subprocess, connects via rmcp stdio client, exercises the full
MCP protocol round-trip with a fixture tool (`tools/echo.nu`).

Gate: only runs when `NUMCP_INTEGRATION=1` is set.

```rust
#[tokio::test]
#[ignore = "integration — set NUMCP_INTEGRATION=1"]
async fn full_round_trip_echo_tool() {
    // spawn numcp serve --config tests/fixtures/echo.toml
    // connect rmcp stdio client
    // call tools/list — verify echo tool present
    // call tools/call {name: "echo", arguments: {msg: "hello"}} — verify "hello" returned
    // shutdown
}
```

---

### Phase 6: Fuzz CI

**P6-1 — add fuzz targets to CI (nightly, short budget)**

```yaml
# .github/workflows/fuzz.yml
- run: cargo +nightly fuzz run fuzz_tool_toml -- -max_total_time=30
- run: cargo +nightly fuzz run fuzz_mcp_tools_call -- -max_total_time=30
```

Corpus files committed to `fuzz/corpus/` from first run.

---

## Acceptance Criteria

The implementation is complete when:

- [ ] `cargo +stable test` passes with zero failures
- [ ] `cargo +stable clippy -- -D warnings` is clean
- [ ] Conformance suite passes against the `numcp` MCP server
- [ ] Integration round-trip test passes (`NUMCP_INTEGRATION=1`)
- [ ] Both fuzz targets run 30s without crash on CI
- [ ] `numcp serve --config example/numcp.toml` starts and responds to `initialize` via
      `rmcp` stdio client

## Out of Scope (v1)

- TCP transport (stdio only)
- Hot-reload of tool definitions
- Authentication or access control
- Windows support
- Tool output streaming (return on completion only)
