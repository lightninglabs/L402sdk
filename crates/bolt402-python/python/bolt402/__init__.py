"""bolt402: L402 client SDK for AI agent frameworks.

Pay for APIs with Lightning. Built in Rust, available in Python.

Backends::

    from bolt402 import LndRestBackend, ClnRestBackend, SwissKnifeBackend

    # LND
    lnd = LndRestBackend("https://localhost:8080", "deadbeef...")
    info = lnd.get_info()

    # CLN (rune)
    cln = ClnRestBackend("https://localhost:3001", "rune_token...")

    # SwissKnife
    sk = SwissKnifeBackend("https://api.numeraire.tech", "sk-...")

L402 Client::

    from bolt402 import L402Client, Budget

    client = L402Client.with_lnd_rest(
        "https://localhost:8080",
        "deadbeef...",
        budget=Budget(total_max=10000),
    )
    response = client.get("https://api.example.com/data")
    print(response.status, response.paid)

Classes:
    L402Client: Main client for L402-aware HTTP requests.
    Budget: Budget configuration for spending limits.
    Receipt: Payment receipt for audit and cost analysis.
    L402Response: Response from an L402-aware request.
    PaymentResult: Result of a Lightning payment.
    NodeInfo: Information about a Lightning node.
    LndRestBackend: LND REST API backend.
    ClnRestBackend: CLN REST API backend.
    SwissKnifeBackend: SwissKnife REST API backend.
"""

from bolt402._bolt402 import (
    Budget,
    ClnRestBackend,
    L402Client,
    L402Response,
    LndRestBackend,
    NodeInfo,
    PaymentResult,
    Receipt,
    SwissKnifeBackend,
)

__all__ = [
    "Budget",
    "ClnRestBackend",
    "L402Client",
    "L402Response",
    "LndRestBackend",
    "NodeInfo",
    "PaymentResult",
    "Receipt",
    "SwissKnifeBackend",
]

__version__ = "0.1.0"
