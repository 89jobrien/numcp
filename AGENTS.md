# numcp — Agent Operating Guide

An MCP server that exposes Nushell tool handlers as Model Context Protocol tools.
Parses TOML config, dynamically loads `.nu` handler scripts, and streams NUON
output to LLM clients via stdio.

## Build, Lint, and Test Commands

### Quick Reference

```bash
# Build all components
cargo build

# Run all tests
cargo test

# Lint code
cargo clippy -- -D warnings

# Format code
cargo fmt

# Integration test (requires release build)
cargo build --release
NUMCP_INTEGRATION=1 cargo test --test round_trip -- --include-ignored
```

### Running Individual Tests

#### Unit Tests

```bash
# Run all tests
cargo test

# Run tests matching a pattern
cargo test parse_minimal_config

# Run a specific test file
cargo test config::tests

# Run with nextest (faster output)
cargo nextest run
```

#### Integration Tests

```bash
# Full integration test (requires NUMCP_INTEGRATION=1)
NUMCP_INTEGRATION=1 cargo test --test round_trip

# With ignores included
NUMCP_INTEGRATION=1 cargo test --test round_trip -- --include-ignored
```

#### Fuzz Tests

```bash
# Fuzz TOML parsing
cargo +nightly fuzz run fuzz_tool_toml

# Fuzz MCP tool calls
cargo +nightly fuzz run fuzz_mcp_tools_call
```

## Code Style Guidelines

### Rust Version & Toolchain

- **Rust Edition**: 2024
- **Components**: rustfmt, clippy
- **Key dependency**: rmcp (MCP Rust SDK v1)

### Formatting (rustfmt)

- Consistency is paramount in tool handler scripts
- Run `cargo fmt` before every commit

### Linting (clippy)

- **Strict linting**: `cargo clippy -- -D warnings`
- Upstream Nu crate warnings are allowed; focus on numcp code only
- No unsafe blocks outside executor's NUON serialization

### Naming Conventions

- **Structs/Enums**: PascalCase (`Config`, `ToolRegistry`, `NuExecutor`)
- **Functions/Methods**: snake_case (`load`, `call_tool`)
- **Module files**: snake_case.rs (`config.rs`, `executor.rs`)

### Error Handling

- **Primary pattern**: `anyhow::Result<T>`
- **Avoid**: `unwrap()` and `expect()` in production code
- **Tool errors**: Return as MCP error content (not protocol errors) for
  graceful client handling

## Project Structure

### Layout

```
numcp/
├── src/
│   ├── main.rs           # CLI entry point, stdio transport wiring
│   ├── lib.rs            # Public library interface
│   ├── config.rs         # Config parsing, validation
│   ├── registry.rs       # Tool registry, MCP schema generation
│   ├── executor.rs       # Nu engine wrapper, handler execution
│   ├── server.rs         # rmcp ServerHandler implementation
│   ├── warm.rs           # Startup validation helper
│   └── error.rs          # Error types
├── tests/integration/
│   └── round_trip.rs     # End-to-end test
├── fuzz/fuzz_targets/
│   ├── fuzz_tool_toml.rs
│   └── fuzz_mcp_tools_call.rs
├── tests/fixtures/
│   ├── echo.toml         # Test config
│   └── echo.nu           # Test handler
├── Cargo.toml
└── Cargo.lock
```

### Module Responsibilities

| Module     | Responsibility                                            |
| ---------- | --------------------------------------------------------- |
| `config`   | Parse `numcp.toml`, validate tool definitions             |
| `registry` | Maintain ordered tool index, generate MCP schemas         |
| `executor` | Initialize Nu engine, evaluate handlers, serialize output |
| `server`   | Implement rmcp ServerHandler, route tool calls            |
| `warm`     | Startup validation, test-run handlers with no params      |
| `error`    | Custom error types for configuration and execution        |

## Testing Patterns

### Unit Tests

- Inline in each module (`#[cfg(test)] mod tests`)
- Config validation in `config::tests`
- Registry ordering in `registry::tests`
- Executor NUON serialization in `executor::tests`

### Integration Tests

- Defined in `tests/integration/round_trip.rs`
- Spawns real `numcp` binary via `rmcp::transport::child_process`
- Gated on `NUMCP_INTEGRATION=1` environment variable
- Tests full MCP protocol round trip

### Fixtures

- Test configs in `tests/fixtures/*.toml`
- Test handlers in `tests/fixtures/*.nu`
- Example: `echo.toml` + `echo.nu` for basic round-trip validation

### Test Isolation

- Executor tests write `.nu` fixtures to `std::env::temp_dir()`
- Tests run in parallel; each creates unique temp files
- No shared state across test modules

## Conventions

### Handler Files

A handler is a `.nu` script that:

- Receives arguments as `$in` (a Nu record)
- Returns any Nu value (list, string, record, etc.)
- Output is serialized to NUON and returned as MCP text content

Example:

```nushell
# tools/web_search.nu
let q = $in.query
http get $"https://example.com/search?q=($q | url encode)" | get results
```

### Config Format

TOML config defines tools with metadata:

```toml
[[tool]]
name = "web_search"
path = "tools/web_search.nu"
description = "Search the web"

[[tool.param]]
name = "query"
type = "string"
description = "Search query"
required = true
```

### MCP Integration

- Tools are exposed via MCP `list_tools` / `call_tool` protocol
- Each tool maps to one `.nu` handler
- Arguments are passed as `$in` (serialized from JSON)
- Output is serialized back to NUON for the client

## Workflows

### Adding a New Tool

1. Create handler script: `tools/my_tool.nu`
2. Add entry to `numcp.toml`:

   ```toml
   [[tool]]
   name = "my_tool"
   path = "tools/my_tool.nu"
   description = "..."

   [[tool.param]]
   name = "param1"
   type = "string"
   required = true
   ```

3. Test with: `NUMCP_INTEGRATION=1 cargo test --test round_trip`
4. Run clippy: `cargo clippy -- -D warnings`
5. Commit

### Testing a Handler

```bash
# Quick validation: warm() runs all handlers with no params
cargo run -- serve --config numcp.toml

# Full integration test
cargo build --release
NUMCP_INTEGRATION=1 cargo test --test round_trip -- --include-ignored
```

### Debugging

```bash
# Inspect config parsing
cargo test config::tests -- --nocapture

# Trace executor behavior
RUST_LOG=debug cargo run -- serve --config numcp.toml 2>&1 | grep executor

# Test handler directly (requires manual Nu setup)
nu -c 'source tools/my_tool.nu; $in'
```

## Key Dependencies

| Crate      | Version | Purpose                                |
| ---------- | ------- | -------------------------------------- |
| `rmcp`     | 1.x     | MCP Rust SDK, stdio transport          |
| `nu-*`     | 0.113.1 | Nu engine (parser, protocol, builtins) |
| `clap`     | 4.x     | CLI argument parsing                   |
| `tokio`    | 1.x     | Async runtime (handler execution)      |
| `indexmap` | 2.x     | Ordered tool registry                  |
| `serde`    | 1.x     | Config serialization                   |

## Development Tips

- **Start small**: Test one handler before expanding to multiple tools
- **Use fixtures**: Define test handlers in `tests/fixtures/` for consistency
- **Validate early**: `warm()` catches handler errors at startup
- **Check schemas**: `registry.rs` generates MCP input schemas from config
- **Parallel tests**: Unit tests run in parallel; avoid shared temp paths
