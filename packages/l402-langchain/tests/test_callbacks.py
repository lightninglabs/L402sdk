"""Tests for PaymentCallbackHandler."""

from l402_langchain.callbacks import PaymentCallbackHandler, PaymentEvent


class TestPaymentEvent:
    """Tests for the PaymentEvent data class."""

    def test_total_sats(self):
        """Total sats includes amount plus fee."""
        event = PaymentEvent(
            amount_sats=100,
            fee_sats=5,
            status=200,
            tool_name="l402_fetch",
            tool_input="https://example.com",
        )
        assert event.total_sats == 105

    def test_repr(self):
        """Repr includes key fields."""
        event = PaymentEvent(
            amount_sats=100,
            fee_sats=5,
            status=200,
            tool_name="l402_fetch",
            tool_input="https://example.com",
        )
        repr_str = repr(event)
        assert "amount_sats=100" in repr_str
        assert "fee_sats=5" in repr_str
        assert "l402_fetch" in repr_str


class TestPaymentCallbackHandler:
    """Tests for the PaymentCallbackHandler."""

    def test_detects_payment(self):
        """Handler fires on_payment when tool output contains payment info."""
        events = []
        handler = PaymentCallbackHandler(
            on_payment=lambda e: events.append(e),
        )

        handler.on_tool_end(
            "[Paid 100 sats (+5 fee) | status 200]\n{\"ok\": true}",
            name="l402_fetch",
        )

        assert len(events) == 1
        assert events[0].amount_sats == 100
        assert events[0].fee_sats == 5
        assert events[0].status == 200

    def test_ignores_non_payment(self):
        """Handler does not fire on tool output without payment info."""
        events = []
        handler = PaymentCallbackHandler(
            on_payment=lambda e: events.append(e),
        )

        handler.on_tool_end(
            "[No payment | status 200]\n{\"ok\": true}",
            name="l402_fetch",
        )

        assert len(events) == 0

    def test_tracks_total_spent(self):
        """Handler accumulates total spending."""
        handler = PaymentCallbackHandler()

        handler.on_tool_end(
            "[Paid 100 sats (+5 fee) | status 200]\ndata",
            name="l402_fetch",
        )
        handler.on_tool_end(
            "[Paid 200 sats (+10 fee) | status 200]\ndata",
            name="l402_fetch",
        )

        assert handler.total_spent == 315  # (100+5) + (200+10)
        assert handler.payment_count == 2

    def test_alert_triggered(self):
        """Alert fires when payment exceeds threshold."""
        alerts = []
        handler = PaymentCallbackHandler(
            max_payment_alert=150,
            on_alert=lambda e: alerts.append(e),
        )

        # Below threshold: no alert
        handler.on_tool_end(
            "[Paid 100 sats (+5 fee) | status 200]\ndata",
            name="l402_fetch",
        )
        assert len(alerts) == 0

        # Above threshold: alert
        handler.on_tool_end(
            "[Paid 200 sats (+10 fee) | status 200]\ndata",
            name="l402_fetch",
        )
        assert len(alerts) == 1
        assert alerts[0].amount_sats == 200

    def test_no_alert_when_disabled(self):
        """No alert fires when max_payment_alert is not set."""
        alerts = []
        handler = PaymentCallbackHandler(
            on_alert=lambda e: alerts.append(e),
        )

        handler.on_tool_end(
            "[Paid 99999 sats (+100 fee) | status 200]\ndata",
            name="l402_fetch",
        )

        assert len(alerts) == 0

    def test_initial_state(self):
        """Handler starts with zero spent and zero payments."""
        handler = PaymentCallbackHandler()
        assert handler.total_spent == 0
        assert handler.payment_count == 0

    def test_ignores_budget_errors(self):
        """Handler does not fire on budget exceeded errors."""
        events = []
        handler = PaymentCallbackHandler(
            on_payment=lambda e: events.append(e),
        )

        handler.on_tool_end(
            "Payment error: BudgetExceeded: per-request limit 500",
            name="l402_fetch",
        )

        assert len(events) == 0
