"""LangChain callback handler for L402 payment events.

Provides ``PaymentCallbackHandler`` that hooks into LangChain's callback
system to observe and react to L402 payments made by tools. Useful for
logging, alerting, cost monitoring, and integration with external systems.
"""

from __future__ import annotations

import re
from typing import Any, Callable, Optional

from langchain_core.callbacks import BaseCallbackHandler

# Pattern to extract payment info from tool output
_PAYMENT_PATTERN = re.compile(
    r"\[Paid (\d+) sats \(\+(\d+) fee\) \| status (\d+)\]"
)


class PaymentEvent:
    """Represents an L402 payment event extracted from tool output.

    Attributes:
        amount_sats: Amount paid in satoshis (excluding fees).
        fee_sats: Routing fee in satoshis.
        status: HTTP response status code after payment.
        tool_name: Name of the LangChain tool that made the payment.
        tool_input: Input that was passed to the tool.
    """

    __slots__ = ("amount_sats", "fee_sats", "status", "tool_name", "tool_input")

    def __init__(
        self,
        amount_sats: int,
        fee_sats: int,
        status: int,
        tool_name: str,
        tool_input: str,
    ) -> None:
        self.amount_sats = amount_sats
        self.fee_sats = fee_sats
        self.status = status
        self.tool_name = tool_name
        self.tool_input = tool_input

    @property
    def total_sats(self) -> int:
        """Total cost including routing fees."""
        return self.amount_sats + self.fee_sats

    def __repr__(self) -> str:
        return (
            f"PaymentEvent(amount_sats={self.amount_sats}, "
            f"fee_sats={self.fee_sats}, status={self.status}, "
            f"tool={self.tool_name!r})"
        )


class PaymentCallbackHandler(BaseCallbackHandler):
    """LangChain callback handler that fires on L402 payment events.

    Hooks into ``on_tool_end`` to parse payment metadata from tool output.
    When a payment is detected, calls the configured callback functions.

    Example::

        from l402_langchain import PaymentCallbackHandler

        payments = []
        handler = PaymentCallbackHandler(
            on_payment=lambda event: payments.append(event),
            max_payment_alert=500,
            on_alert=lambda event: print(f"ALERT: {event.amount_sats} sats!"),
        )

        # Pass to LangChain agent
        executor.invoke({"input": "..."}, config={"callbacks": [handler]})
    """

    def __init__(
        self,
        on_payment: Optional[Callable[[PaymentEvent], None]] = None,
        on_alert: Optional[Callable[[PaymentEvent], None]] = None,
        max_payment_alert: Optional[int] = None,
    ) -> None:
        """Initialize the callback handler.

        Args:
            on_payment: Called for every detected payment event.
            on_alert: Called when a payment exceeds ``max_payment_alert``.
            max_payment_alert: Satoshi threshold for triggering alerts.
                If ``None``, alerts are disabled.
        """
        super().__init__()
        self._on_payment = on_payment
        self._on_alert = on_alert
        self._max_payment_alert = max_payment_alert
        self._total_spent: int = 0
        self._payment_count: int = 0

    @property
    def total_spent(self) -> int:
        """Total satoshis observed across all payments."""
        return self._total_spent

    @property
    def payment_count(self) -> int:
        """Number of payments observed."""
        return self._payment_count

    def on_tool_end(
        self,
        output: Any,
        *,
        run_id: Any = None,
        parent_run_id: Any = None,
        tags: Optional[list[str]] = None,
        **kwargs: Any,
    ) -> None:
        """Process tool output and extract payment events.

        Called by LangChain after a tool finishes execution. Parses the
        output string for payment metadata and fires callbacks.
        """
        output_str = str(output)
        match = _PAYMENT_PATTERN.search(output_str)
        if not match:
            return

        amount_sats = int(match.group(1))
        fee_sats = int(match.group(2))
        status = int(match.group(3))

        # Extract tool metadata from kwargs if available
        tool_name = kwargs.get("name", "unknown")
        tool_input = kwargs.get("tool_input", "")
        if isinstance(tool_input, dict):
            tool_input = str(tool_input)

        event = PaymentEvent(
            amount_sats=amount_sats,
            fee_sats=fee_sats,
            status=status,
            tool_name=tool_name,
            tool_input=str(tool_input),
        )

        self._total_spent += event.total_sats
        self._payment_count += 1

        if self._on_payment is not None:
            self._on_payment(event)

        if (
            self._on_alert is not None
            and self._max_payment_alert is not None
            and amount_sats > self._max_payment_alert
        ):
            self._on_alert(event)
