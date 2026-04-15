"""l402-langchain: LangChain integration for L402 Lightning payments.

Provides LangChain tools that enable AI agents to autonomously pay for
L402-gated APIs using Lightning Network payments via L402sdk.

Quick start::

    from l402_langchain import create_l402_client, L402FetchTool

    client = create_l402_client(
        backend="lnd",
        url="https://localhost:8080",
        macaroon="deadbeef...",
    )
    fetch = L402FetchTool(client=client)
    result = fetch.invoke("https://api.example.com/data")

Classes:
    L402FetchTool: LangChain tool for L402-aware HTTP requests.
    L402BudgetTool: LangChain tool for spending monitoring.
    PaymentCallbackHandler: LangChain callback for payment events.

Functions:
    create_l402_client: Factory for creating configured L402 clients.
"""

from l402_langchain.callbacks import PaymentCallbackHandler
from l402_langchain.config import create_l402_client
from l402_langchain.tools import L402BudgetTool, L402FetchTool

__all__ = [
    "L402FetchTool",
    "L402BudgetTool",
    "PaymentCallbackHandler",
    "create_l402_client",
]

__version__ = "0.1.0"
