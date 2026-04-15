"""L402sdk + LangChain Integration Example.

Demonstrates how to create a LangChain tool that uses L402sdk to access
L402-gated APIs with automatic Lightning payments.

This example runs fully self-contained using mock infrastructure.
No API keys, no Lightning node, no LLM required.

Run:
    python langchain_example.py

For the full LangChain agent version (requires OPENAI_API_KEY), see the
``run_agent_example()`` function at the bottom.
"""

from __future__ import annotations

from typing import ClassVar

from langchain_core.tools import BaseTool
from pydantic import ConfigDict

from l402 import Budget, L402Client, L402Response, MockL402Server, create_mock_client


# ---------------------------------------------------------------------------
# L402FetchTool — LangChain tool wrapping L402sdk
# ---------------------------------------------------------------------------


class L402FetchTool(BaseTool):
    """LangChain tool that fetches data from L402-gated APIs.

    Wraps a ``L402sdk.L402Client`` to handle the full L402 payment flow
    transparently: HTTP 402 → parse challenge → pay Lightning invoice →
    retry with token → return data.

    The tool is designed to be given to a LangChain agent so the LLM can
    decide when and what to fetch, while L402sdk handles payment logic.

    Example::

        client, server = create_mock_client({"/api/data": 100})
        tool = L402FetchTool(client=client)
        result = tool.invoke(f"{server.url}/api/data")
    """

    name: ClassVar[str] = "l402_fetch"
    description: ClassVar[str] = (
        "Fetch data from an API that requires Lightning payment (L402). "
        "Input should be a full URL. The tool automatically handles payment "
        "if the API responds with HTTP 402. Returns the response body as text."
    )

    model_config = ConfigDict(arbitrary_types_allowed=True)

    client: L402Client

    def _run(self, url: str) -> str:
        """Execute the L402-aware HTTP GET request.

        Args:
            url: Full URL to fetch (e.g. ``http://127.0.0.1:PORT/api/data``).

        Returns:
            Response body as text, prefixed with payment info if a payment
            was made.
        """
        try:
            response: L402Response = self.client.get(url)
        except ValueError as exc:
            return f"Payment error: {exc}"
        except RuntimeError as exc:
            return f"Request error: {exc}"

        if response.paid and response.receipt is not None:
            receipt = response.receipt
            header = (
                f"[Paid {receipt.amount_sats} sats "
                f"(+{receipt.fee_sats} fee) | "
                f"status {response.status}]\n"
            )
            return header + response.text()

        return f"[status {response.status}]\n{response.text()}"


# ---------------------------------------------------------------------------
# L402CostTool — Query spending so far
# ---------------------------------------------------------------------------


class L402CostTool(BaseTool):
    """LangChain tool that reports the total amount spent via L402sdk.

    Useful for agents that need to be cost-aware or report spending to users.
    """

    name: ClassVar[str] = "l402_check_cost"
    description: ClassVar[str] = (
        "Check total Lightning sats spent so far across all L402 API calls. "
        "Takes no input. Returns total sats spent and receipt count."
    )

    model_config = ConfigDict(arbitrary_types_allowed=True)

    client: L402Client

    def _run(self, _input: str = "") -> str:
        """Return spending summary."""
        total = self.client.total_spent()
        receipts = self.client.receipts()
        if not receipts:
            return "No payments made yet. Total spent: 0 sats."

        lines = [f"Total spent: {total} sats across {len(receipts)} payment(s).\n"]
        for i, r in enumerate(receipts, 1):
            lines.append(
                f"  #{i}: {r.endpoint} — {r.amount_sats} sats "
                f"(status {r.response_status}, {r.latency_ms}ms)"
            )
        return "\n".join(lines)


# ---------------------------------------------------------------------------
# Demo: self-contained (no API keys needed)
# ---------------------------------------------------------------------------


def run_mock_demo() -> None:
    """Run a self-contained demo using mock infrastructure.

    Demonstrates:
    - Creating a mock L402 server with priced endpoints
    - Wrapping L402sdk in LangChain tools
    - Invoking tools the same way a LangChain agent would
    - Budget enforcement
    - Cost tracking via receipts
    """
    print("L402sdk + LangChain Example")
    print("=" * 40)
    print()

    # --- Setup: mock server + client with budget ---
    endpoints = {
        "/api/weather": 50,
        "/api/market-data": 200,
        "/api/premium-report": 1000,
    }
    budget = Budget(per_request_max=500, daily_max=5000)
    client, server = create_mock_client(endpoints, budget=budget)

    print(f"Mock server running at {server.url}")
    print(f"Endpoints: {endpoints}")
    print(f"Budget: max 500 sats/request, 5,000 sats/day")
    print()

    # --- Create LangChain tools ---
    fetch_tool = L402FetchTool(client=client)
    cost_tool = L402CostTool(client=client)

    print(f"Tools registered: [{fetch_tool.name}, {cost_tool.name}]")
    print()

    # --- Simulate agent calling tools ---

    # 1. Fetch weather data (50 sats — within budget)
    print("[1] Agent calls: l402_fetch('/api/weather')")
    result = fetch_tool.invoke(f"{server.url}/api/weather")
    print(f"    Result: {result}")
    print()

    # 2. Fetch market data (200 sats — within budget)
    print("[2] Agent calls: l402_fetch('/api/market-data')")
    result = fetch_tool.invoke(f"{server.url}/api/market-data")
    print(f"    Result: {result}")
    print()

    # 3. Check spending so far
    print("[3] Agent calls: l402_check_cost()")
    result = cost_tool.invoke("")
    print(f"    Result: {result}")
    print()

    # 4. Try premium report (1000 sats — exceeds per-request budget)
    print("[4] Agent calls: l402_fetch('/api/premium-report') — over budget!")
    result = fetch_tool.invoke(f"{server.url}/api/premium-report")
    print(f"    Result: {result}")
    print()

    # 5. Fetch weather again (cached token — no payment)
    print("[5] Agent calls: l402_fetch('/api/weather') — should use cached token")
    result = fetch_tool.invoke(f"{server.url}/api/weather")
    print(f"    Result: {result}")
    print()

    # 6. Final cost check
    print("[6] Agent calls: l402_check_cost()")
    result = cost_tool.invoke("")
    print(f"    Result: {result}")
    print()

    print("=" * 40)
    print("Demo complete.")
    print()
    print(
        "To use with a real LangChain agent + LLM, see run_agent_example() "
        "in this file and install langchain-openai."
    )


# ---------------------------------------------------------------------------
# Full agent example (requires OPENAI_API_KEY + langchain-openai)
# ---------------------------------------------------------------------------


def run_agent_example() -> None:
    """Run a full LangChain agent with an LLM.

    Requires:
        pip install langchain-openai
        export OPENAI_API_KEY=sk-...

    The agent will autonomously decide which tools to call based on the
    prompt, using L402sdk to pay for L402-gated APIs.
    """
    try:
        from langchain_openai import ChatOpenAI
    except ImportError:
        print("Install langchain-openai to run this example:")
        print("  pip install langchain-openai")
        return

    from langchain_core.prompts import ChatPromptTemplate, MessagesPlaceholder

    # Setup (same mock infrastructure)
    endpoints = {
        "/api/weather": 50,
        "/api/market-data": 200,
    }
    budget = Budget(per_request_max=500, daily_max=5000)
    client, server = create_mock_client(endpoints, budget=budget)

    # Create tools
    fetch_tool = L402FetchTool(client=client)
    cost_tool = L402CostTool(client=client)
    tools = [fetch_tool, cost_tool]

    # Create agent
    llm = ChatOpenAI(model="gpt-4o", temperature=0)
    llm_with_tools = llm.bind_tools(tools)

    prompt = ChatPromptTemplate.from_messages(
        [
            (
                "system",
                "You are a data analyst assistant. You can fetch data from "
                "L402-gated APIs using the l402_fetch tool. Each API call may "
                "cost Lightning sats. Use l402_check_cost to monitor spending. "
                f"Available endpoints at {server.url}: "
                "/api/weather (50 sats), /api/market-data (200 sats). "
                "Always report costs after fetching data.",
            ),
            ("human", "{input}"),
            MessagesPlaceholder("agent_scratchpad"),
        ]
    )

    from langchain.agents import AgentExecutor, create_tool_calling_agent

    agent = create_tool_calling_agent(llm, tools, prompt)
    executor = AgentExecutor(agent=agent, tools=tools, verbose=True)

    # Run
    response = executor.invoke(
        {"input": "Fetch both weather and market data, then tell me the total cost."}
    )
    print("\nAgent response:", response["output"])


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    import sys

    if "--agent" in sys.argv:
        run_agent_example()
    else:
        run_mock_demo()
