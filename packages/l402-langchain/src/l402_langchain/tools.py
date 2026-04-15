"""LangChain tools for L402-gated API access via L402sdk.

Provides ``L402FetchTool`` for making HTTP requests that automatically
handle L402 payment challenges, and ``L402BudgetTool`` for monitoring
spending across all L402 interactions.
"""

from __future__ import annotations

import json
from typing import Any, ClassVar, Optional

from langchain_core.tools import BaseTool
from pydantic import ConfigDict, Field

from l402 import L402Client, L402Response


class L402FetchTool(BaseTool):
    """LangChain tool for fetching data from L402-gated APIs.

    Wraps a ``L402sdk.L402Client`` to handle the full L402 payment flow
    transparently: HTTP 402 -> parse challenge -> pay Lightning invoice ->
    retry with token -> return data.

    Budget limits configured on the client are enforced automatically.
    Token caching is handled by L402sdk so repeated requests to the same
    endpoint reuse cached tokens without additional payments.

    Example::

        from l402_langchain import create_l402_client, L402FetchTool

        client = create_l402_client(
            backend="lnd",
            url="https://localhost:8080",
            macaroon="deadbeef...",
        )
        tool = L402FetchTool(client=client)
        result = tool.invoke("https://api.example.com/data")
    """

    name: ClassVar[str] = "l402_fetch"
    description: ClassVar[str] = (
        "Fetch data from an API that requires Lightning payment (L402). "
        "Input should be a full URL, or a JSON object with 'url' and "
        "optional 'body' for POST requests. The tool automatically handles "
        "payment if the API responds with HTTP 402. Returns the response "
        "body with payment metadata."
    )

    model_config = ConfigDict(arbitrary_types_allowed=True)

    client: L402Client = Field(exclude=True)

    def _run(self, input_str: str) -> str:
        """Execute an L402-aware HTTP request.

        Args:
            input_str: Either a plain URL for GET requests, or a JSON
                string ``{"url": "...", "body": "..."}`` for POST requests.

        Returns:
            Response body prefixed with payment metadata. On error,
            returns a descriptive error string (not an exception) so the
            LLM agent can reason about the failure.
        """
        url, body = _parse_input(input_str)

        try:
            if body is not None:
                response: L402Response = self.client.post(url, body)
            else:
                response = self.client.get(url)
        except ValueError as exc:
            return f"Payment error: {exc}"
        except RuntimeError as exc:
            return f"Request error: {exc}"

        return _format_response(response)


class L402BudgetTool(BaseTool):
    """LangChain tool for monitoring L402 spending.

    Reports total satoshis spent, number of payments, and a per-endpoint
    breakdown. Useful for cost-aware agents that need to track or report
    their API spending.

    Example::

        from l402_langchain import create_l402_client, L402BudgetTool

        client = create_l402_client(
            backend="lnd",
            url="https://localhost:8080",
            macaroon="deadbeef...",
        )
        tool = L402BudgetTool(client=client)
        result = tool.invoke("")  # "No payments made yet. ..."
    """

    name: ClassVar[str] = "l402_check_budget"
    description: ClassVar[str] = (
        "Check total Lightning sats spent so far across all L402 API "
        "calls. Takes no input. Returns total sats spent, receipt count, "
        "and a breakdown per endpoint."
    )

    model_config = ConfigDict(arbitrary_types_allowed=True)

    client: L402Client = Field(exclude=True)

    def _run(self, _input: str = "") -> str:
        """Return a spending summary.

        Returns:
            Human-readable spending report with total sats, receipt
            count, and per-endpoint breakdown.
        """
        total = self.client.total_spent()
        receipts = self.client.receipts()

        if not receipts:
            return "No payments made yet. Total spent: 0 sats."

        lines = [
            f"Total spent: {total} sats across {len(receipts)} payment(s).",
            "",
        ]
        for i, receipt in enumerate(receipts, 1):
            lines.append(
                f"  #{i}: {receipt.endpoint} — "
                f"{receipt.amount_sats} sats "
                f"(+{receipt.fee_sats} fee, "
                f"status {receipt.response_status}, "
                f"{receipt.latency_ms}ms)"
            )

        return "\n".join(lines)


def _parse_input(input_str: str) -> tuple[str, Optional[str]]:
    """Parse tool input into (url, optional_body).

    Accepts either a plain URL string or a JSON object with 'url' and
    optional 'body' keys.
    """
    stripped = input_str.strip()

    # Try JSON parse for POST requests
    if stripped.startswith("{"):
        try:
            data = json.loads(stripped)
            url = data.get("url", "")
            body = data.get("body")
            if isinstance(body, dict):
                body = json.dumps(body)
            return url, body
        except (json.JSONDecodeError, AttributeError):
            pass

    # Plain URL for GET requests
    return stripped, None


def _format_response(response: L402Response) -> str:
    """Format an L402Response into a human-readable string with metadata."""
    if response.paid and response.receipt is not None:
        receipt = response.receipt
        header = (
            f"[Paid {receipt.amount_sats} sats "
            f"(+{receipt.fee_sats} fee) | "
            f"status {response.status}]\n"
        )
        return header + response.text()

    return f"[No payment | status {response.status}]\n{response.text()}"
