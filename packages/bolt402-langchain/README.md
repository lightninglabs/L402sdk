# bolt402-langchain

LangChain integration for [bolt402](https://github.com/lightninglabs/bolt402) — enabling AI agents to autonomously pay for L402-gated APIs using Lightning Network payments.

## Overview

`bolt402-langchain` provides LangChain tools that wrap the `bolt402` L402 client SDK. When an AI agent encounters an API that requires Lightning payment (HTTP 402), these tools handle the entire payment flow transparently:

1. Agent calls `l402_fetch` with a URL
2. bolt402 detects the 402 response and L402 challenge
3. Lightning invoice is paid automatically
4. Agent receives the API response with payment metadata

## Installation

```bash
pip install bolt402-langchain
```

## Quick Start

```python
from bolt402 import create_mock_client
from bolt402_langchain import L402FetchTool, L402BudgetTool

# Create a mock L402 server for testing
client, server = create_mock_client({
    "/api/weather": 50,      # 50 sats per request
    "/api/market-data": 200, # 200 sats per request
})

# Create LangChain tools
fetch_tool = L402FetchTool(client=client)
budget_tool = L402BudgetTool(client=client)

# Use directly (or give to a LangChain agent)
result = fetch_tool.invoke(f"{server.url}/api/weather")
print(result)
# [Paid 50 sats (+0 fee) | status 200]
# {"ok": true, "price": 50}

# Check spending
print(budget_tool.invoke(""))
# Total spent: 50 sats across 1 payment(s).
```

## Tools

### `L402FetchTool`

Makes HTTP requests to L402-gated APIs with automatic payment handling.

**Input:** URL string (GET) or JSON `{"url": "...", "body": "..."}` (POST)

**Output:** Response body prefixed with payment metadata

```python
# GET request
result = fetch_tool.invoke("https://api.example.com/data")

# POST request
result = fetch_tool.invoke('{"url": "https://api.example.com/data", "body": "{\"query\": \"BTC\"}"}')
```

### `L402BudgetTool`

Reports spending: total sats, receipt count, per-endpoint breakdown.

```python
result = budget_tool.invoke("")
# Total spent: 250 sats across 2 payment(s).
#
#   #1: http://127.0.0.1:PORT/api/weather — 50 sats (+0 fee, status 200, 5ms)
#   #2: http://127.0.0.1:PORT/api/market — 200 sats (+0 fee, status 200, 3ms)
```

## Callbacks

`PaymentCallbackHandler` hooks into LangChain's callback system for payment observability:

```python
from bolt402_langchain import PaymentCallbackHandler

payments = []
handler = PaymentCallbackHandler(
    on_payment=lambda event: payments.append(event),
    max_payment_alert=500,
    on_alert=lambda event: print(f"ALERT: {event.amount_sats} sats!"),
)

# Pass to LangChain agent
executor.invoke({"input": "..."}, config={"callbacks": [handler]})

# After execution
print(handler.total_spent)    # Total sats observed
print(handler.payment_count)  # Number of payments
```

## Configuration

The `create_l402_client` factory simplifies setup:

```python
from bolt402_langchain import create_l402_client

# Mock backend (for testing)
client, server = create_l402_client(
    backend="mock",
    endpoints={"/api/data": 100},
    budget={"per_request_max": 500, "daily_max": 5000},
)
```

Budget can be a dict or a `bolt402.Budget` instance:

```python
from bolt402 import Budget

budget = Budget(per_request_max=500, hourly_max=2000, daily_max=10000)
client, server = create_l402_client(
    backend="mock",
    endpoints={"/api/data": 100},
    budget=budget,
)
```

## With LangChain Agents

Works with any LangChain agent type (ReAct, OpenAI Functions, tool-calling):

```python
from langchain_openai import ChatOpenAI
from langchain.agents import create_tool_calling_agent, AgentExecutor
from langchain_core.prompts import ChatPromptTemplate, MessagesPlaceholder
from bolt402_langchain import L402FetchTool, L402BudgetTool, create_l402_client

client, server = create_l402_client(
    backend="mock",
    endpoints={"/api/weather": 50, "/api/market": 200},
    budget={"per_request_max": 500},
)

tools = [L402FetchTool(client=client), L402BudgetTool(client=client)]
llm = ChatOpenAI(model="gpt-4o")

prompt = ChatPromptTemplate.from_messages([
    ("system", f"You can fetch data from L402 APIs at {server.url}. "
               "Use l402_fetch for data, l402_check_budget for spending."),
    ("human", "{input}"),
    MessagesPlaceholder("agent_scratchpad"),
])

agent = create_tool_calling_agent(llm, tools, prompt)
executor = AgentExecutor(agent=agent, tools=tools)
response = executor.invoke({"input": "Get weather and market data, report costs."})
```

## License

MIT OR Apache-2.0
