"""Tests for the create_l402_client factory function."""

import pytest

from bolt402 import Budget, L402Client

from bolt402_langchain import create_l402_client


class TestCreateL402Client:
    """Tests for the create_l402_client factory."""

    def test_lnd_backend(self):
        """LND backend creates a working client."""
        client = create_l402_client(
            backend="lnd",
            url="https://localhost:8080",
            macaroon="deadbeef0123456789",
        )
        assert isinstance(client, L402Client)

    def test_cln_backend(self):
        """CLN backend creates a working client."""
        client = create_l402_client(
            backend="cln",
            url="https://localhost:3001",
            rune="test_rune_token",
        )
        assert isinstance(client, L402Client)

    def test_swissknife_backend(self):
        """SwissKnife backend creates a working client."""
        client = create_l402_client(
            backend="swissknife",
            url="https://api.numeraire.tech",
            api_key="sk-test-key",
        )
        assert isinstance(client, L402Client)

    def test_lnd_with_dict_budget(self):
        """Dict budget is converted to Budget instance."""
        client = create_l402_client(
            backend="lnd",
            url="https://localhost:8080",
            macaroon="deadbeef0123456789",
            budget={"per_request_max": 200, "daily_max": 1000},
        )
        assert isinstance(client, L402Client)

    def test_lnd_with_budget_instance(self):
        """Budget instance is passed through directly."""
        budget = Budget(per_request_max=200)
        client = create_l402_client(
            backend="lnd",
            url="https://localhost:8080",
            macaroon="deadbeef0123456789",
            budget=budget,
        )
        assert isinstance(client, L402Client)

    def test_lnd_missing_macaroon_raises(self):
        """Missing macaroon for LND raises ValueError."""
        with pytest.raises(ValueError, match="macaroon"):
            create_l402_client(
                backend="lnd",
                url="https://localhost:8080",
            )

    def test_cln_missing_rune_raises(self):
        """Missing rune for CLN raises ValueError."""
        with pytest.raises(ValueError, match="rune"):
            create_l402_client(
                backend="cln",
                url="https://localhost:3001",
            )

    def test_swissknife_missing_api_key_raises(self):
        """Missing api_key for SwissKnife raises ValueError."""
        with pytest.raises(ValueError, match="api_key"):
            create_l402_client(
                backend="swissknife",
                url="https://api.numeraire.tech",
            )

    def test_unsupported_backend_raises(self):
        """Unsupported backend raises ValueError."""
        with pytest.raises(ValueError, match="Unsupported backend"):
            create_l402_client(
                backend="nwc",
                url="https://localhost:8080",
            )

    def test_invalid_budget_type_raises(self):
        """Non-dict, non-Budget budget raises TypeError."""
        with pytest.raises(TypeError, match="budget must be"):
            create_l402_client(
                backend="lnd",
                url="https://localhost:8080",
                macaroon="deadbeef",
                budget="invalid",  # type: ignore[arg-type]
            )

    def test_custom_max_fee_sats(self):
        """Custom max_fee_sats is accepted."""
        client = create_l402_client(
            backend="lnd",
            url="https://localhost:8080",
            macaroon="deadbeef0123456789",
            max_fee_sats=50,
        )
        assert isinstance(client, L402Client)
