"""l402: L402 client SDK for AI agent frameworks.

Pay for APIs with Lightning. Built in Rust, available in Python.

Backends::

    from l402 import LndGrpcBackend, LndRestBackend, ClnGrpcBackend, ClnRestBackend, SwissKnifeBackend

    # LND (gRPC)
    lnd = LndGrpcBackend("https://localhost:10009", "/path/to/tls.cert", "/path/to/admin.macaroon")

    # LND (REST)
    lnd = LndRestBackend("https://localhost:8080", "deadbeef...")

    # CLN (gRPC, mTLS)
    cln = ClnGrpcBackend("https://localhost:9736", "/path/to/ca.pem", "/path/to/client.pem", "/path/to/client-key.pem")

    # CLN (REST, rune)
    cln = ClnRestBackend("https://localhost:3001", "rune_token...")

    # SwissKnife
    sk = SwissKnifeBackend("https://api.numeraire.tech", "sk-...")

L402 Client::

    from l402 import L402Client, Budget

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
    LndGrpcBackend: LND gRPC backend.
    LndRestBackend: LND REST API backend.
    ClnGrpcBackend: CLN gRPC backend (mTLS).
    ClnRestBackend: CLN REST API backend.
    SwissKnifeBackend: SwissKnife REST API backend.
"""

from l402._l402 import (
    Budget,
    ClnGrpcBackend,
    ClnRestBackend,
    L402Client,
    L402Response,
    LndGrpcBackend,
    LndRestBackend,
    NodeInfo,
    PaymentResult,
    Receipt,
    SwissKnifeBackend,
)

__all__ = [
    "Budget",
    "ClnGrpcBackend",
    "ClnRestBackend",
    "L402Client",
    "L402Response",
    "LndGrpcBackend",
    "LndRestBackend",
    "NodeInfo",
    "PaymentResult",
    "Receipt",
    "SwissKnifeBackend",
]

from importlib.metadata import version as _version

__version__ = _version("l402")
