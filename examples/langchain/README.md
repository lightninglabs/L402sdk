# LangChain + L402sdk Example

Demonstrates how to build LangChain tools that use L402sdk for automatic Lightning payments on L402-gated APIs.

## What It Does

1. Creates a mock L402 server with priced API endpoints
2. Wraps `L402sdk.L402Client` in custom LangChain tools (`L402FetchTool`, `L402CostTool`)
3. Simulates an agent calling the tools — fetching data, paying invoices, tracking costs
4. Demonstrates budget enforcement (over-budget requests are rejected)
5. Shows token caching (repeated requests skip payment)

## Quick Start (No API Keys Needed)

```bash
# From the L402sdk repo root
cd crates/l402-python
python -m venv .venv
source .venv/bin/activate
pip install maturin langchain-core
maturin develop

# Run the example
cd ../../examples/langchain
python langchain_example.py
```

## Expected Output

```
L402sdk + LangChain Example
========================================

Mock server running at http://127.0.0.1:XXXXX
Endpoints: {'/api/weather': 50, '/api/market-data': 200, '/api/premium-report': 1000}
Budget: max 500 sats/request, 5,000 sats/day

Tools registered: [l402_fetch, l402_check_cost]

[1] Agent calls: l402_fetch('/api/weather')
    Result: [Paid 50 sats (+0 fee) | status 200]
    {"ok":true,"price":50}

[2] Agent calls: l402_fetch('/api/market-data')
    Result: [Paid 200 sats (+0 fee) | status 200]
    {"ok":true,"price":200}

[3] Agent calls: l402_check_cost()
    Result: Total spent: 250 sats across 2 payment(s).
    ...

[4] Agent calls: l402_fetch('/api/premium-report') — over budget!
    Result: Payment error: BudgetExceeded: ...

[5] Agent calls: l402_fetch('/api/weather') — should use cached token
    Result: [status 200]
    {"ok":true,"price":50}

[6] Agent calls: l402_check_cost()
    Result: Total spent: 250 sats across 2 payment(s).
    ...
```

## Full Agent Mode (Requires OpenAI)

To run with a real LLM making autonomous tool-calling decisions:

```bash
pip install langchain langchain-openai
export OPENAI_API_KEY=sk-...
python langchain_example.py --agent
```

## Architecture

```
User prompt
    ↓
LangChain Agent (ChatOpenAI / any LLM)
    ↓ tool call
L402FetchTool (custom BaseTool)
    ↓
L402sdk.L402Client.get(url)
    ↓
HTTP 402 → parse challenge → pay invoice → retry → HTTP 200
    ↓
Response text returned to agent
    ↓
LLM generates final answer
```

## Tools Provided

| Tool | Description |
|------|-------------|
| `l402_fetch` | Fetch data from an L402-gated API. Handles payments automatically. |
| `l402_check_cost` | Report total sats spent and payment history. |

## Adapting for Production

Replace the mock client with a real Lightning backend:

```python
# Instead of create_mock_client(), configure a real backend:
from l402 import L402Client, Budget

client = L402Client(
    backend="lnd",
    lnd_url="https://your-lnd-node:8080",
    lnd_macaroon="hex-encoded-macaroon",
    budget=Budget(per_request_max=1000, daily_max=50000),
)

tool = L402FetchTool(client=client)
```

> Note: LND and SwissKnife backends for Python are coming soon. The mock backend is fully functional for development and testing.
