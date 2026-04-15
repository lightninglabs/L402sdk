"""Tests for L402FetchTool and L402BudgetTool.

Constructor and metadata tests work without a real Lightning backend.
Tests that require actual L402 responses are marked as regtest tests
and skipped when the regtest environment is not available.
"""

import json

import pytest

from l402 import L402Client
from l402_langchain import L402BudgetTool, L402FetchTool


class TestL402FetchToolMetadata:
    """Tests for L402FetchTool metadata and construction."""

    def test_tool_metadata(self, client_setup):
        """Tool has correct name and description for LangChain."""
        tool = L402FetchTool(client=client_setup["client"])

        assert tool.name == "l402_fetch"
        assert "L402" in tool.description
        assert "Lightning" in tool.description

    def test_constructor_accepts_client(self, client_setup):
        """Tool can be constructed with an L402Client."""
        tool = L402FetchTool(client=client_setup["client"])
        assert tool is not None


class TestL402BudgetToolMetadata:
    """Tests for L402BudgetTool metadata and construction."""

    def test_tool_metadata(self, client_setup):
        """Tool has correct name and description."""
        tool = L402BudgetTool(client=client_setup["client"])

        assert tool.name == "l402_check_budget"
        assert "spent" in tool.description.lower()

    def test_no_payments(self, client_setup):
        """Reports zero spending when no payments made."""
        tool = L402BudgetTool(client=client_setup["client"])
        result = tool.invoke("")

        assert "No payments made yet" in result
        assert "0 sats" in result
