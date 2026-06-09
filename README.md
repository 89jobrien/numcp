# numcp

An MCP server that exposes Nushell tool handlers as [Model Context Protocol](https://modelcontextprotocol.io/) tools.

`numcp` is the bridge between Nu-defined tool closures (e.g. `kubectl`, `web_search`,
`query_knowledge_base`) and any MCP client — including LLM agent daemons that need to
call local tools during a reasoning loop.

## Overview

```
LLM agent daemon
  │  MCP stdio transport
  ▼
numcp serve
  │  nu-mcp engine
  ▼
tool handlers (Nu scripts / config-defined tools)
  │
  ▼
kubectl / ragit / SurrealDB / web / ...
```

Tools are defined in a config file (`numcp.toml`) and optionally via `.nu` handler scripts.
`numcp` registers them as MCP tools and executes them on demand.

## Status

**Pre-alpha skeleton.** Not yet functional. Architecture is defined; implementation is in progress.

See [`docs/plans/2026-06-09-baml-agent-daemon.md`](https://github.com/89jobrien/nu_libs/blob/main/docs/plans/2026-06-09-baml-agent-daemon.md)
in the `nu_libs` repo for the full design.

## Installation

```sh
cargo install numcp
```

Or build from source:

```sh
git clone https://github.com/89jobrien/numcp
cargo build --release
```

## Usage

```sh
numcp serve --config numcp.toml
```

## Config

```toml
# numcp.toml

[[tool]]
name        = "web_search"
description = "Search the web for a query"
handler     = "tools/web_search.nu"

[tool.parameters.query]
type        = "string"
description = "Search terms"
required    = true

[[tool]]
name        = "kubectl"
description = "Run a kubectl command"
handler     = "tools/kubectl.nu"

[tool.parameters.args]
type        = "array"
description = "Arguments to pass to kubectl"
required    = true
```

## Tool Handler Format

A handler is a Nu script that receives arguments as `$in` and returns a string result:

```nushell
#!/usr/bin/env nu
# tools/web_search.nu
let q = $in.query
http get $"https://example.com/search?q=($q | url encode)" | get results | to yaml
```

## Design

- **MCP transport**: stdio (default), TCP planned
- **Nu engine**: via [`nu-mcp`](https://crates.io/crates/nu-mcp) (official Nushell crate)
- **Protocol**: [`rmcp`](https://crates.io/crates/rmcp) — official MCP Rust SDK

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
