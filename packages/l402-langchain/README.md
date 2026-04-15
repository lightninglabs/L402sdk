<div align="center">
  <h1>l402-langchain</h1>

  <p>
    <strong>LangChain tools for L402 Lightning payments</strong>
  </p>

  <p>
    <a href="https://pypi.org/project/l402-langchain/"><img alt="PyPI" src="https://img.shields.io/pypi/v/l402-langchain.svg"/></a>
    <a href="https://pypi.org/project/l402-langchain/"><img alt="PyPI downloads" src="https://img.shields.io/pypi/dm/l402-langchain.svg"/></a>
    <a href="https://github.com/lightninglabs/L402sdk/blob/main/LICENSE-MIT"><img alt="MIT or Apache-2.0 Licensed" src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg"/></a>
  </p>

</div>

Enable AI agents to autonomously pay for [L402](https://docs.lightning.engineering/the-lightning-network/l402)-gated APIs using the Lightning Network.

## Install

```bash
pip install l402-langchain
```

## Quick Start

```python
from l402_langchain import create_l402_client, L402FetchTool, L402BudgetTool

# Create a client backed by LND REST
client = create_l402_client(
    backend="lnd",
    url="https://localhost:8080",
    macaroon="hex-encoded-admin-macaroon",
    budget={"per_request_max": 500, "daily_max": 5000},
)

# Create LangChain tools
fetch_tool = L402FetchTool(client=client)
budget_tool = L402BudgetTool(client=client)

# Use directly
result = fetch_tool.invoke("https://api.example.com/data")
print(result)
# [Paid 100 sats (+0 fee) | status 200]
# {"ok": true}

# Check spending
print(budget_tool.invoke(""))
# Total spent: 100 sats across 1 payment(s).
```

## Tools

### `L402FetchTool`

Makes HTTP requests with automatic L402 payment handling.

**Input:** URL string (GET) or JSON `{"url": "...", "body": "..."}` (POST)

```python
# GET
result = fetch_tool.invoke("https://api.example.com/data")

# POST
result = fetch_tool.invoke('{"url": "https://api.example.com/data", "body": "{\"query\": \"BTC\"}"}')
```

### `L402BudgetTool`

Reports spending: total sats, receipt count, per-endpoint breakdown.

```python
result = budget_tool.invoke("")
# Total spent: 250 sats across 2 payment(s).
#
#   #1: https://api.example.com/weather — 50 sats (+0 fee, status 200, 5ms)
#   #2: https://api.example.com/market — 200 sats (+0 fee, status 200, 3ms)
```

## Callbacks

`PaymentCallbackHandler` hooks into LangChain's callback system for payment observability:

```python
from l402_langchain import PaymentCallbackHandler

handler = PaymentCallbackHandler(
    on_payment=lambda event: print(f"Paid {event.amount_sats} sats"),
    max_payment_alert=500,
    on_alert=lambda event: print(f"ALERT: {event.amount_sats} sats!"),
)

# Pass to LangChain agent
executor.invoke({"input": "..."}, config={"callbacks": [handler]})
```

## Lightning Backends

```python
from l402_langchain import create_l402_client

# LND REST
client = create_l402_client(backend="lnd", url="https://localhost:8080", macaroon="...")

# Core Lightning (CLN) REST
client = create_l402_client(backend="cln", url="https://localhost:3010", rune="...")

# SwissKnife
client = create_l402_client(backend="swissknife", url="https://app.numeraire.tech", api_key="sk-...")
```

## With LangChain Agents

```python
from langchain_openai import ChatOpenAI
from langchain.agents import create_tool_calling_agent, AgentExecutor
from langchain_core.prompts import ChatPromptTemplate, MessagesPlaceholder
from l402_langchain import create_l402_client, L402FetchTool, L402BudgetTool

client = create_l402_client(
    backend="lnd",
    url="https://localhost:8080",
    macaroon="deadbeef...",
    budget={"per_request_max": 500},
)

tools = [L402FetchTool(client=client), L402BudgetTool(client=client)]
llm = ChatOpenAI(model="gpt-4o")

prompt = ChatPromptTemplate.from_messages([
    ("system", "You can fetch data from L402-gated APIs. "
               "Use l402_fetch for data, l402_check_budget for spending."),
    ("human", "{input}"),
    MessagesPlaceholder("agent_scratchpad"),
])

agent = create_tool_calling_agent(llm, tools, prompt)
executor = AgentExecutor(agent=agent, tools=tools)
response = executor.invoke({"input": "Get weather data, then report costs."})
```

## License

MIT OR Apache-2.0
