"""Shared test fixtures for l402-langchain tests."""

import pytest

from l402 import Budget, L402Client


@pytest.fixture()
def client_setup():
    """Create a client with dummy LND credentials for constructor tests.

    This client cannot make real requests but is valid for testing tool
    construction, metadata, and budget reporting.
    """
    client = L402Client.with_lnd_rest(
        "https://localhost:8080",
        "deadbeef0123456789",
    )
    return {"client": client}


@pytest.fixture()
def budget_client_setup():
    """Create a client with budget limits and dummy LND credentials.

    Budget: 500 sats per request, 2000 sats daily.
    """
    budget = Budget(per_request_max=500, daily_max=2000)
    client = L402Client.with_lnd_rest(
        "https://localhost:8080",
        "deadbeef0123456789",
        budget=budget,
    )
    return {"client": client}
