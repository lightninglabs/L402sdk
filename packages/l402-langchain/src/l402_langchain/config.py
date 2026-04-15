"""Configuration helpers for creating L402sdk clients.

Provides ``create_l402_client()`` as a convenient factory function for
LangChain users who want a quick-start setup without learning the full
L402sdk API.
"""

from __future__ import annotations

from typing import Any, Optional, Union

from l402 import Budget, L402Client


def create_l402_client(
    *,
    backend: str,
    url: str,
    macaroon: Optional[str] = None,
    tls_cert_path: Optional[str] = None,
    macaroon_path: Optional[str] = None,
    rune: Optional[str] = None,
    ca_cert_path: Optional[str] = None,
    client_cert_path: Optional[str] = None,
    client_key_path: Optional[str] = None,
    api_key: Optional[str] = None,
    budget: Optional[Union[Budget, dict[str, Any]]] = None,
    max_fee_sats: int = 100,
) -> L402Client:
    """Create a configured L402 client for use with LangChain tools.

    Factory function that simplifies client creation. Supports LND
    (gRPC + REST), CLN (gRPC + REST), and SwissKnife backends.

    Args:
        backend: Lightning backend type. One of ``"lnd"``, ``"lnd-grpc"``,
            ``"cln"``, ``"cln-grpc"``, or ``"swissknife"``.
        url: Backend API URL.
        macaroon: Hex-encoded admin macaroon. Required for ``"lnd"``
            (REST) backend.
        tls_cert_path: Path to LND's ``tls.cert``. Required for
            ``"lnd-grpc"`` backend.
        macaroon_path: Path to admin macaroon file. Required for
            ``"lnd-grpc"`` backend.
        rune: Rune token string. Required for ``"cln"`` (REST) backend.
        ca_cert_path: Path to CA cert. Required for ``"cln-grpc"``.
        client_cert_path: Path to client cert. Required for ``"cln-grpc"``.
        client_key_path: Path to client key. Required for ``"cln-grpc"``.
        api_key: API key. Required for ``"swissknife"`` backend.
        budget: Spending limits. Can be a ``Budget`` instance or a dict
            with keys ``per_request_max``, ``hourly_max``, ``daily_max``,
            ``total_max``.
        max_fee_sats: Maximum routing fee in satoshis per payment.

    Returns:
        A configured ``L402Client`` instance.

    Raises:
        ValueError: If the backend is not supported or required parameters
            are missing.

    Example::

        from l402_langchain import create_l402_client

        client = create_l402_client(
            backend="lnd",
            url="https://localhost:8080",
            macaroon="deadbeef...",
            budget={"per_request_max": 200, "daily_max": 5000},
        )
    """
    resolved_budget = _resolve_budget(budget)

    if backend == "lnd":
        if not macaroon:
            raise ValueError(
                "macaroon is required for LND backend. "
                "Provide a hex-encoded admin macaroon."
            )
        return L402Client.with_lnd_rest(
            url,
            macaroon,
            budget=resolved_budget,
            max_fee_sats=max_fee_sats,
        )

    if backend == "lnd-grpc":
        if not tls_cert_path or not macaroon_path:
            raise ValueError(
                "tls_cert_path and macaroon_path are required for LND gRPC backend."
            )
        return L402Client.with_lnd_grpc(
            url,
            tls_cert_path,
            macaroon_path,
            budget=resolved_budget,
            max_fee_sats=max_fee_sats,
        )

    if backend == "cln":
        if not rune:
            raise ValueError(
                "rune is required for CLN backend. "
                "Provide a CLN rune token string."
            )
        return L402Client.with_cln_rest(
            url,
            rune,
            budget=resolved_budget,
            max_fee_sats=max_fee_sats,
        )

    if backend == "cln-grpc":
        if not ca_cert_path or not client_cert_path or not client_key_path:
            raise ValueError(
                "ca_cert_path, client_cert_path, and client_key_path are required "
                "for CLN gRPC backend."
            )
        return L402Client.with_cln_grpc(
            url,
            ca_cert_path,
            client_cert_path,
            client_key_path,
            budget=resolved_budget,
            max_fee_sats=max_fee_sats,
        )

    if backend == "swissknife":
        if not api_key:
            raise ValueError(
                "api_key is required for SwissKnife backend. "
                "Provide a SwissKnife API key."
            )
        return L402Client.with_swissknife(
            url,
            api_key,
            budget=resolved_budget,
            max_fee_sats=max_fee_sats,
        )

    raise ValueError(
        f"Unsupported backend: {backend!r}. "
        f"Supported: 'lnd', 'lnd-grpc', 'cln', 'cln-grpc', 'swissknife'."
    )


def _resolve_budget(
    budget: Optional[Union[Budget, dict[str, Any]]],
) -> Optional[Budget]:
    """Convert a budget dict to a Budget instance, or pass through."""
    if budget is None:
        return None
    if isinstance(budget, Budget):
        return budget
    if isinstance(budget, dict):
        return Budget(
            per_request_max=budget.get("per_request_max"),
            hourly_max=budget.get("hourly_max"),
            daily_max=budget.get("daily_max"),
            total_max=budget.get("total_max"),
        )
    raise TypeError(
        f"budget must be a Budget instance or dict, got {type(budget).__name__}"
    )
