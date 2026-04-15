# Design: LangChain Python Integration Package

**Issue:** #55
**Author:** Dario Anongba Varela
**Date:** 2026-03-22
**Status:** Implementing

## Problem

LangChain is the largest AI agent framework with 200M+ monthly PyPI downloads. L402sdk has Python bindings (PyO3) and an existing LangChain example (`examples/langchain/`), but no installable, tested, CI-validated Python package. AI developers need a drop-in integration they can `pip install` and immediately use with LangChain agents.

The existing example (`examples/langchain/langchain_example.py`) proves the concept works. Issue #55 asks us to promote this into a proper package with:
- Installable Python package structure
- LangChain `BaseTool` implementations (fetch + budget checking)
- Payment event callbacks for observability
- Configuration helpers for backend/budget setup
- Tests passing in CI
- Documentation and tutorial

## Proposed Design

### Package: `l402-langchain`

A pure-Python package (no Rust build required at install time) that depends on the `L402sdk` PyO3 bindings and `langchain-core`.

### Directory Structure

```
packages/l402-langchain/
├── pyproject.toml
├── README.md
├── src/
│   └── l402_langchain/
│       ├── __init__.py         # Public API re-exports
│       ├── tools.py            # L402FetchTool, L402BudgetTool
│       ├── callbacks.py        # PaymentCallbackHandler (LangChain callback)
│       └── config.py           # Client factory: create_l402_client()
└── tests/
    ├── conftest.py             # Shared fixtures (mock client/server)
    ├── test_tools.py           # Tool unit tests
    ├── test_callbacks.py       # Callback tests
    └── test_config.py          # Config/factory tests
```

### Core Components

#### 1. `L402FetchTool` (LangChain BaseTool)

Wraps `L402sdk.L402Client.get()` and `.post()` for autonomous API access:

```python
from l402_langchain import L402FetchTool
from l402 import create_mock_client

client, server = create_mock_client({"/api/data": 100})
tool = L402FetchTool(client=client)
result = tool.invoke(f"{server.url}/api/data")
```

The tool:
- Accepts a URL (GET) or URL + JSON body (POST) 
- Handles the full L402 flow transparently
- Returns structured output with payment metadata
- Catches budget errors and returns them as tool output (not exceptions)

#### 2. `L402BudgetTool` (spending monitor)

Reports total spending, receipt count, and per-endpoint breakdown:

```python
from l402_langchain import L402BudgetTool

budget_tool = L402BudgetTool(client=client)
result = budget_tool.invoke("")  # "Total spent: 100 sats across 1 payment(s)"
```

#### 3. `PaymentCallbackHandler` (LangChain BaseCallbackHandler)

A LangChain callback handler that fires on L402 payment events. Useful for logging, alerting, or integrating with external systems:

```python
from l402_langchain import PaymentCallbackHandler

handler = PaymentCallbackHandler(
    on_payment=lambda receipt: print(f"Paid {receipt.amount_sats} sats"),
    max_payment_alert=500,  # Alert if any single payment exceeds this
)
```

Since LangChain callbacks fire on tool start/end/error, the handler hooks into `on_tool_end` to extract payment info from tool output.

#### 4. `create_l402_client()` factory

Configuration helper that creates a `L402sdk.L402Client` from keyword arguments:

```python
from l402_langchain import create_l402_client

# Mock mode (for testing)
client, server = create_l402_client(
    backend="mock",
    endpoints={"/api/data": 100},
    budget={"per_request_max": 500, "daily_max": 5000},
)

# Future: real backend mode
# client = create_l402_client(backend="lnd", lnd_url="...", budget={...})
```

### Integration with LangChain Agents

Works with any LangChain agent type:

```python
from langchain_openai import ChatOpenAI
from langchain.agents import create_tool_calling_agent, AgentExecutor
from l402_langchain import L402FetchTool, L402BudgetTool, create_l402_client

client, server = create_l402_client(backend="mock", endpoints={"/api/data": 100})
tools = [L402FetchTool(client=client), L402BudgetTool(client=client)]
agent = create_tool_calling_agent(ChatOpenAI(), tools, prompt)
executor = AgentExecutor(agent=agent, tools=tools)
```

### Key Decisions

1. **Pure Python package**: No Rust compilation at install. Depends on pre-built `L402sdk` wheel. This keeps installation simple (`pip install l402-langchain`).

2. **`langchain-core` dependency, not `langchain`**: Only depend on the minimal `langchain-core` package (which provides `BaseTool`, `BaseCallbackHandler`). Users bring their own LLM integrations.

3. **Evolved from example**: The tools are refined versions of `examples/langchain/langchain_example.py`, promoted to a proper package with full test coverage.

4. **Tool output includes payment metadata**: Rather than hiding payments, the tool output includes `[Paid X sats]` prefix so the LLM agent is aware of costs. This enables cost-aware agent behavior.

5. **Error handling via tool output**: Budget exceeded and payment failures return descriptive strings rather than raising exceptions. This follows LangChain best practice where tools should return error descriptions so the agent can reason about them.

6. **Callback handler for observability**: Instead of baking logging into the tools, we use LangChain's callback system. This is composable and follows LangChain patterns.

### Alternatives Considered

- **Embedding in `l402-python` crate**: Would couple LangChain dependency to the core bindings. Separate package is cleaner.
- **Using `@tool` decorator**: Class-based `BaseTool` is better for dependency injection (client instance).
- **Async tools**: LangChain supports async but `L402sdk` Python bindings use `block_on` internally. Sync tools are simpler and correct.

### Testing Plan

1. **Unit tests**: Test each tool with mock client/server. Verify payment flow, caching, budget enforcement, error handling.
2. **Callback tests**: Verify `PaymentCallbackHandler` fires correctly.
3. **Config tests**: Verify `create_l402_client()` factory.
4. **CI integration**: Add a `langchain` job to the GitHub Actions workflow that:
   - Builds `L402sdk` Python bindings via `maturin develop`
   - Installs `l402-langchain` in editable mode
   - Runs `pytest` on the package tests
5. **No LLM in CI**: All tests use mock infrastructure and direct tool invocation. No OpenAI API key required.
