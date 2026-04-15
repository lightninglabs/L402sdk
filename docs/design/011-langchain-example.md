# Design Doc 011: LangChain Integration Example

**Issue:** #24
**Author:** Dario Anongba Varela
**Date:** 2026-03-18

## Problem

LangChain is the most popular Python AI agent framework (~200M monthly PyPI downloads). L402sdk already has Python bindings (`l402-python` via PyO3), but no example showing how to use them with LangChain agents. A working LangChain integration is the highest-impact way to demonstrate L402sdk adoption in the AI agent ecosystem.

## Proposed Design

### Directory Structure

```
examples/
  langchain/
    README.md                    # Setup instructions, architecture diagram
    langchain_example.py         # Main example: LangChain agent with L402 tool
    requirements.txt             # Python dependencies
```

### Core Component: L402FetchTool

A custom LangChain `BaseTool` that wraps `l402-python`. The tool:

1. Receives a URL from the LangChain agent
2. Uses `L402sdk.L402Client.get()` to make the request
3. Handles the full L402 flow transparently (402 → pay → retry → 200)
4. Returns the response body (or error) back to the agent

```python
from langchain_core.tools import BaseTool
from l402 import create_mock_client, Budget

class L402FetchTool(BaseTool):
    name: str = "l402_fetch"
    description: str = "Fetch data from an L402-gated API. Automatically pays Lightning invoices."

    def _run(self, url: str) -> str:
        response = self.client.get(url)
        return response.text()
```

### Example Flow

1. Create a mock L402 server with test endpoints
2. Build an `L402FetchTool` wrapping the mock client
3. Create a LangChain `ChatOpenAI` agent with the tool
4. Send a prompt asking the agent to fetch paid data
5. The agent calls `l402_fetch`, L402sdk handles payment, agent gets data

### Self-Contained Demo Mode

Since the example should work without real Lightning infrastructure or an OpenAI key, the main path uses:
- `L402sdk.create_mock_client()` for the payment backend
- Direct tool invocation (no LLM call required) to demonstrate the flow
- An optional section showing how to wire it into a full LangChain agent with an LLM

## API Sketch

```python
# Self-contained demo (no API keys needed)
from l402 import create_mock_client, Budget

client, server = create_mock_client(
    {"/api/weather": 50, "/api/market-data": 200},
    budget=Budget(per_request_max=500, daily_max=5000),
)

tool = L402FetchTool(client=client, server_url=server.url)
result = tool.invoke(f"{server.url}/api/weather")

# Full agent mode (requires OPENAI_API_KEY)
from langchain_openai import ChatOpenAI
from langchain.agents import AgentExecutor, create_tool_calling_agent

llm = ChatOpenAI(model="gpt-4o")
agent = create_tool_calling_agent(llm, [tool], prompt)
executor = AgentExecutor(agent=agent, tools=[tool])
response = executor.invoke({"input": "Fetch the weather data and market data"})
```

## Key Decisions

1. **BaseTool over @tool decorator**: Using class-based tool allows the client/server instances to be injected cleanly rather than relying on closures or globals.

2. **Mock-first, real-optional**: The example runs fully self-contained with mocks. A clearly documented section shows how to swap in a real backend.

3. **No new crate/package**: This is an example, not a separate `l402-langchain` package. That can be a future issue if demand warrants it.

4. **requirements.txt over pyproject.toml**: Simpler for an example directory. Lists `langchain`, `langchain-openai`, `L402sdk`.

## Alternatives Considered

- **Full `l402-langchain` adapter package**: Overkill for an initial example. Create a separate issue if there is demand.
- **Notebook (`.ipynb`)**: More interactive but harder to test in CI. Markdown README with code blocks is simpler.
- **CrewAI/AutoGen examples too**: Out of scope per the issue. Separate issues exist for those.

## Testing Plan

1. The example script must run successfully with `python langchain_example.py` using only mock infrastructure (no API keys).
2. Verify `l402-python` builds and the example imports work within CI (optional, could be a follow-up).
3. Manual test: run the example and confirm the output matches expected flow.
