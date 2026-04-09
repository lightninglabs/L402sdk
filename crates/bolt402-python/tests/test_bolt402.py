"""Tests for bolt402 Python bindings.

Run with: pytest tests/test_bolt402.py -v
Requires: pip install bolt402 (or maturin develop)
"""

import bolt402
from bolt402 import (
    Budget,
    ClnRestBackend,
    L402Client,
    LndRestBackend,
    NodeInfo,
    PaymentResult,
    Receipt,
    SwissKnifeBackend,
)


class TestBudget:
    """Tests for the Budget class."""

    def test_unlimited(self):
        budget = Budget.unlimited()
        assert repr(budget) == "Budget(per_request_max=None, hourly_max=None, daily_max=None, total_max=None)"

    def test_with_limits(self):
        budget = Budget(
            per_request_max=100,
            hourly_max=1000,
            daily_max=5000,
            total_max=50000,
        )
        assert "per_request_max=100" in repr(budget)
        assert "total_max=50000" in repr(budget)

    def test_partial_limits(self):
        budget = Budget(per_request_max=100)
        assert "per_request_max=100" in repr(budget)
        assert "hourly_max=None" in repr(budget)

    def test_default_is_unlimited(self):
        budget = Budget()
        assert "per_request_max=None" in repr(budget)
        assert "total_max=None" in repr(budget)


class TestLndRestBackend:
    """Tests for the LndRestBackend class."""

    def test_constructor(self):
        backend = LndRestBackend("https://localhost:8080", "deadbeef")
        assert repr(backend) == "LndRestBackend(...)"

    def test_constructor_strips_trailing_slash(self):
        # Should not raise
        backend = LndRestBackend("https://localhost:8080/", "deadbeef")
        assert backend is not None


class TestClnRestBackend:
    """Tests for the ClnRestBackend class."""

    def test_constructor_rune(self):
        backend = ClnRestBackend("https://localhost:3001", "test_rune")
        assert repr(backend) == "ClnRestBackend(...)"


class TestSwissKnifeBackend:
    """Tests for the SwissKnifeBackend class."""

    def test_constructor(self):
        backend = SwissKnifeBackend("https://api.numeraire.tech", "sk-test-key")
        assert repr(backend) == "SwissKnifeBackend(...)"


class TestL402ClientConstructors:
    """Tests for L402Client static constructor methods."""

    def test_with_lnd_rest(self):
        client = L402Client.with_lnd_rest("https://localhost:8080", "deadbeef")
        assert repr(client) == "L402Client(...)"

    def test_with_lnd_rest_custom_budget(self):
        budget = Budget(per_request_max=100, total_max=10000)
        client = L402Client.with_lnd_rest(
            "https://localhost:8080",
            "deadbeef",
            budget=budget,
            max_fee_sats=50,
        )
        assert client is not None

    def test_with_cln_rest(self):
        client = L402Client.with_cln_rest("https://localhost:3001", "test_rune")
        assert repr(client) == "L402Client(...)"

    def test_with_swissknife(self):
        client = L402Client.with_swissknife("https://api.numeraire.tech", "sk-test")
        assert repr(client) == "L402Client(...)"

    def test_with_swissknife_custom_budget(self):
        budget = Budget(daily_max=5000)
        client = L402Client.with_swissknife(
            "https://api.numeraire.tech",
            "sk-test",
            budget=budget,
            max_fee_sats=200,
        )
        assert client is not None

    def test_default_max_fee_sats(self):
        """Constructor defaults max_fee_sats to 100."""
        # These should not raise (defaults applied internally)
        L402Client.with_lnd_rest("https://localhost:8080", "deadbeef")
        L402Client.with_cln_rest("https://localhost:3001", "rune")
        L402Client.with_swissknife("https://api.numeraire.tech", "key")


class TestExports:
    """Test that all expected classes are exported."""

    def test_all_classes_importable(self):
        assert Budget is not None
        assert ClnRestBackend is not None
        assert L402Client is not None
        assert LndRestBackend is not None
        assert NodeInfo is not None
        assert PaymentResult is not None
        assert Receipt is not None
        assert SwissKnifeBackend is not None

    def test_version(self):
        assert bolt402.__version__ == "0.1.0"

    def test_all_exports(self):
        expected = {
            "Budget",
            "ClnRestBackend",
            "L402Client",
            "L402Response",
            "LndRestBackend",
            "NodeInfo",
            "PaymentResult",
            "Receipt",
            "SwissKnifeBackend",
        }
        assert set(bolt402.__all__) == expected
