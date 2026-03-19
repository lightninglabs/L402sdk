# Design Doc 012: MCP Server Mode

**Issue:** #26
**Author:** Toshi (maintainer)
**Date:** 2026-03-19
**Status:** Implementing

## Problem

bolt402 currently integrates with AI agents through framework-specific SDKs (Vercel AI SDK, Python/LangChain). Each new agent framework requires a dedicated integration crate. The Model Context Protocol (MCP) provides a universal tool discovery standard that lets any MCP-compatible agent (Claude Code, Cursor, Codex, OpenClaw, etc.) discover and invoke tools without framework-specific code.

We need an MCP server binary that wraps bolt402's L402 capabilities as MCP tools, enabling zero-config integration with any MCP client.

## Proposed Design

### New Crate: `bolt402-mcp`

A standalone binary crate that runs as an MCP server over stdio transport. It wraps the existing `bolt402-core` client engine and exposes three tools.

### Crate Dependency Graph

```
bolt402-proto  (shared protocol types)
     ↑
bolt402-core   (client engine, ports, adapters)
     ↑
bolt402-mcp    (MCP server binary)
     ↑
bolt402-lnd    (LND backend, used when --backend=lnd)
bolt402-mock   (mock backend, used when --backend=mock)
```

### MCP Tools

#### 1. `l402_request`

Make an HTTP request to an L402-gated API. Handles the full 402 challenge → pay invoice → retry flow automatically.

**Parameters:**
```json
{
  "url": "string (required) - The URL to request",
  "method": "string (optional, default: GET) - HTTP method",
  "body": "string (optional) - JSON request body for POST/PUT",
  "max_fee_sats": "number (optional) - Override max routing fee"
}
```

**Returns:** JSON with `status`, `body`, `paid` (bool), and `receipt` (if payment was made).

#### 2. `check_budget`

Query the current budget status and spending statistics.

**Parameters:** None

**Returns:** JSON with `total_spent_sats`, `budget` config, and current period usage.

#### 3. `list_receipts`

Get all payment receipts for audit and cost analysis.

**Parameters:**
```json
{
  "limit": "number (optional) - Max receipts to return",
  "endpoint_filter": "string (optional) - Filter by endpoint substring"
}
```

**Returns:** Array of receipt objects with timestamp, endpoint, amount, fees, payment hash, status, and latency.

### CLI Interface

```bash
# With LND backend
bolt402-mcp --backend lnd \
  --lnd-url https://localhost:10009 \
  --lnd-macaroon /path/to/admin.macaroon \
  --lnd-cert /path/to/tls.cert

# With mock backend (for testing/demos)
bolt402-mcp --backend mock

# With budget limits
bolt402-mcp --backend lnd \
  --lnd-url https://localhost:10009 \
  --lnd-macaroon /path/to/admin.macaroon \
  --budget-per-request 1000 \
  --budget-hourly 10000 \
  --budget-total 100000

# Environment variables also supported
BOLT402_LND_URL=https://localhost:10009 \
BOLT402_LND_MACAROON=/path/to/admin.macaroon \
bolt402-mcp --backend lnd
```

### Architecture

```
main.rs
├── Parse CLI args (clap)
├── Build L402Client from args
│   ├── Select LnBackend (LND or Mock)
│   ├── Configure InMemoryTokenStore
│   └── Configure BudgetTracker
├── Construct Bolt402McpServer (holds Arc<L402Client>)
└── Serve over stdio transport (rmcp)

server.rs
├── Bolt402McpServer struct
│   ├── client: Arc<L402Client>
│   └── budget: Budget (for reporting)
├── #[tool_router] impl
│   ├── #[tool] l402_request(url, method, body, max_fee_sats)
│   ├── #[tool] check_budget()
│   └── #[tool] list_receipts(limit, endpoint_filter)
└── #[tool_handler] impl ServerHandler
```

### Key Decisions

1. **rmcp v1.2 (official Rust MCP SDK):** The official, well-maintained SDK from the MCP project. Provides `#[tool]` macros, stdio transport, and full MCP spec compliance.

2. **Stdio transport only (for now):** The standard MCP pattern. Streamable HTTP can be added later if needed. Stdio is sufficient for Claude Code, Cursor, and other MCP clients.

3. **Binary crate, not library:** This is a standalone executable. Users run it as an MCP server process. The library functionality lives in `bolt402-core`.

4. **Shared `L402Client` via `Arc`:** The MCP server holds an `Arc<L402Client>` so all tool invocations share the same budget tracker, token cache, and receipt log.

5. **clap for CLI:** Standard Rust CLI argument parsing. Environment variable support via `clap`'s `env` attribute.

6. **`schemars` for tool parameter schemas:** Required by rmcp for auto-generating JSON Schema for tool inputs.

### Alternatives Considered

- **Library-only (no binary):** Rejected. MCP servers need to be standalone processes. A binary is essential.
- **Custom MCP implementation:** Rejected. rmcp is the official SDK and handles protocol details correctly.
- **HTTP transport:** Deferred. Stdio is the standard for local MCP servers. Can add HTTP later.
- **SwissKnife backend support:** Deferred to a follow-up. LND and mock are sufficient for the initial release.

## Testing Plan

1. **Unit tests:** Test each tool handler in isolation with a mock backend.
2. **Integration test:** Spawn the MCP server as a child process, send MCP JSON-RPC messages over stdio, verify responses.
3. **CI:** Add `bolt402-mcp` to the workspace build and test pipeline.
4. **Manual test:** Configure in Claude Code's MCP settings, verify tool discovery and invocation.

## Documentation

- README section on MCP server setup
- MCP client configuration examples (Claude Code, Cursor)
- Example: `docs/tutorials/mcp-setup.md`

## Acceptance Criteria

- [ ] `bolt402-mcp` binary builds and runs
- [ ] Three MCP tools: `l402_request`, `check_budget`, `list_receipts`
- [ ] CLI flags for backend configuration
- [ ] Environment variable support
- [ ] Unit tests for all tools
- [ ] Integration test with stdio transport
- [ ] CI passes (fmt, clippy, test, doc)
- [ ] Documentation for setup and usage
