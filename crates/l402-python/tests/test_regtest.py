"""Integration tests for L402sdk Python bindings against the regtest environment.

These tests verify the Python bindings work with real Lightning infrastructure.
They are skipped when the regtest Docker stack is not running.

Required env vars (from tests/regtest/.env.regtest):
    L402_SERVER_URL, LND_REST_HOST, LND_MACAROON_HEX

Optional env vars:
    SWISSKNIFE_API_URL, SWISSKNIFE_API_KEY

Run with:
    pytest tests/test_regtest.py -v
"""

import os
import pathlib

import pytest

# Try to load .env.regtest
_env_candidates = [
    pathlib.Path(__file__).parent / "../../../tests/regtest/.env.regtest",
    pathlib.Path(__file__).parent / "../../tests/regtest/.env.regtest",
]

for _p in _env_candidates:
    _p = _p.resolve()
    if _p.exists():
        for line in _p.read_text().splitlines():
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            eq = line.find("=")
            if eq == -1:
                continue
            key, val = line[:eq], line[eq + 1:]
            os.environ.setdefault(key, val)
        break

L402_SERVER_URL = os.environ.get("L402_SERVER_URL", "http://localhost:8081")

# LND credentials
LND_GRPC_HOST = os.environ.get("LND_GRPC_HOST", "https://localhost:10009")
LND_REST_HOST = os.environ.get("LND_REST_HOST", "https://localhost:8080")
LND_MACAROON_HEX = os.environ.get("LND_MACAROON_HEX", "")
LND_TLS_CERT_BASE64 = os.environ.get("LND_TLS_CERT_BASE64", "")

# CLN credentials
CLN_GRPC_HOST = os.environ.get("CLN_GRPC_HOST", "https://localhost:9736")
CLN_REST_URL = os.environ.get("CLN_REST_URL", "https://localhost:3010")
CLN_RUNE = os.environ.get("CLN_RUNE", "")
CLN_CA_CERT_BASE64 = os.environ.get("CLN_CA_CERT_BASE64", "")
CLN_CLIENT_CERT_BASE64 = os.environ.get("CLN_CLIENT_CERT_BASE64", "")
CLN_CLIENT_KEY_BASE64 = os.environ.get("CLN_CLIENT_KEY_BASE64", "")

# SwissKnife credentials
SWISSKNIFE_API_URL = os.environ.get("SWISSKNIFE_API_URL", "")
SWISSKNIFE_API_KEY = os.environ.get("SWISSKNIFE_API_KEY", "")


def _regtest_available() -> bool:
    """Check if the regtest L402 server is reachable."""
    try:
        import urllib.error
        import urllib.request

        req = urllib.request.Request(f"{L402_SERVER_URL}/health", method="GET")
        resp = urllib.request.urlopen(req, timeout=5)
        return resp.status in (200, 402)
    except urllib.error.HTTPError as e:
        # Aperture returns 402 for all routes — that means it's up.
        return e.code == 402
    except Exception:
        return False


def _has_lnd_credentials() -> bool:
    """Check if LND REST credentials are available."""
    return bool(LND_MACAROON_HEX)


def _has_lnd_grpc_credentials() -> bool:
    """Check if LND gRPC credentials are available."""
    return bool(LND_MACAROON_HEX) and bool(LND_TLS_CERT_BASE64)


def _has_cln_credentials() -> bool:
    """Check if CLN REST credentials are available."""
    return bool(CLN_RUNE)


def _has_cln_grpc_credentials() -> bool:
    """Check if CLN gRPC credentials are available."""
    return bool(CLN_CA_CERT_BASE64) and bool(CLN_CLIENT_CERT_BASE64) and bool(CLN_CLIENT_KEY_BASE64)


def _write_temp_file(name: str, b64_content: str) -> str:
    """Decode base64 content and write to a temp file, return the path."""
    import base64
    import tempfile
    data = base64.b64decode(b64_content)
    path = os.path.join(tempfile.gettempdir(), f"l402-regtest-{name}")
    with open(path, "wb") as f:
        f.write(data)
    return path


def _has_swissknife_credentials() -> bool:
    """Check if SwissKnife credentials are available."""
    return bool(SWISSKNIFE_API_URL) and bool(SWISSKNIFE_API_KEY)


# Skip entire module if regtest is not running
pytestmark = pytest.mark.skipif(
    not _regtest_available(),
    reason="Regtest environment not available (is Docker running?)",
)


from l402 import (
    Budget,
    ClnGrpcBackend,
    ClnRestBackend,
    L402Client,
    LndGrpcBackend,
    LndRestBackend,
    SwissKnifeBackend,
)


# ---------------------------------------------------------------------------
# LND REST backend tests
# ---------------------------------------------------------------------------


@pytest.mark.skipif(not _has_lnd_credentials(), reason="LND credentials not available")
class TestLndRestBackend:
    """Tests for the LND REST backend against regtest."""

    def test_get_info(self):
        """Verify LND node connectivity."""
        backend = LndRestBackend(LND_REST_HOST, LND_MACAROON_HEX)
        info = backend.get_info()
        assert info.pubkey
        assert info.num_active_channels > 0

    def test_get_balance(self):
        """Verify LND balance query."""
        backend = LndRestBackend(LND_REST_HOST, LND_MACAROON_HEX)
        balance = backend.get_balance()
        assert balance > 1000


@pytest.mark.skipif(not _has_lnd_credentials(), reason="LND credentials not available")
class TestLndRestL402Flow:
    """Full L402 protocol flow using LND REST backend."""

    def test_full_flow(self):
        """GET request with automatic L402 payment."""
        client = L402Client.with_lnd_rest(LND_REST_HOST, LND_MACAROON_HEX)

        response = client.get(f"{L402_SERVER_URL}/api/data")
        assert response.status == 200
        assert response.paid is True
        assert response.receipt is not None
        assert response.receipt.amount_sats == 100
        assert response.receipt.response_status == 200
        assert len(response.receipt.payment_hash) > 0
        assert len(response.receipt.preimage) > 0

    def test_response_json(self):
        """Verify response body is parseable JSON."""
        client = L402Client.with_lnd_rest(LND_REST_HOST, LND_MACAROON_HEX)

        response = client.get(f"{L402_SERVER_URL}/api/data")
        data = response.json()
        assert data["ok"] is True

    def test_nonexistent_endpoint(self):
        """Non-existent endpoints return 404 without payment."""
        client = L402Client.with_lnd_rest(LND_REST_HOST, LND_MACAROON_HEX)

        response = client.get(f"{L402_SERVER_URL}/api/nonexistent")
        assert response.status == 404
        assert response.paid is False
        assert response.receipt is None

    def test_token_caching(self):
        """Tokens are cached and reused on subsequent requests."""
        client = L402Client.with_lnd_rest(LND_REST_HOST, LND_MACAROON_HEX)

        # First request: should pay
        r1 = client.get(f"{L402_SERVER_URL}/api/data")
        assert r1.paid is True
        assert r1.cached_token is False

        # Second request: should use cached token (no payment)
        r2 = client.get(f"{L402_SERVER_URL}/api/data")
        assert r2.paid is False
        assert r2.cached_token is True
        assert r2.status == 200

        # Only one receipt
        assert len(client.receipts()) == 1

    def test_budget_enforcement(self):
        """Budget limits prevent overspending."""
        budget = Budget(per_request_max=50)
        client = L402Client.with_lnd_rest(
            LND_REST_HOST,
            LND_MACAROON_HEX,
            budget=budget,
        )

        # 100-sat endpoint should exceed per-request limit of 50
        with pytest.raises(ValueError, match="BudgetExceeded"):
            client.get(f"{L402_SERVER_URL}/api/data")

    def test_budget_allows_cheap_request(self):
        """Requests within budget succeed."""
        budget = Budget(per_request_max=200)
        client = L402Client.with_lnd_rest(
            LND_REST_HOST,
            LND_MACAROON_HEX,
            budget=budget,
        )

        response = client.get(f"{L402_SERVER_URL}/api/data")
        assert response.status == 200
        assert response.paid is True

    def test_total_budget_enforcement(self):
        """Total budget is enforced across multiple requests."""
        budget = Budget(total_max=150)
        client = L402Client.with_lnd_rest(
            LND_REST_HOST,
            LND_MACAROON_HEX,
            budget=budget,
        )

        # First request: 100 sats (within budget)
        r1 = client.get(f"{L402_SERVER_URL}/api/data")
        assert r1.status == 200

        # Second request to different endpoint: would exceed total budget
        with pytest.raises(ValueError, match="BudgetExceeded"):
            client.get(f"{L402_SERVER_URL}/api/premium")

    def test_receipts_accumulate(self):
        """Receipts are recorded across multiple requests."""
        client = L402Client.with_lnd_rest(LND_REST_HOST, LND_MACAROON_HEX)

        client.get(f"{L402_SERVER_URL}/api/cheap")
        client.get(f"{L402_SERVER_URL}/api/data")

        receipts = client.receipts()
        assert len(receipts) == 2
        assert receipts[0].amount_sats == 10
        assert receipts[1].amount_sats == 100

    def test_total_spent(self):
        """Total spent tracker is accurate."""
        client = L402Client.with_lnd_rest(LND_REST_HOST, LND_MACAROON_HEX)

        assert client.total_spent() == 0
        client.get(f"{L402_SERVER_URL}/api/cheap")
        assert client.total_spent() == 10
        client.get(f"{L402_SERVER_URL}/api/data")
        assert client.total_spent() == 110

    def test_receipt_preimage_matches_hash(self):
        """Preimage hashes to the payment hash (proof of payment)."""
        import hashlib

        client = L402Client.with_lnd_rest(LND_REST_HOST, LND_MACAROON_HEX)

        response = client.get(f"{L402_SERVER_URL}/api/cheap")
        receipt = response.receipt

        preimage_bytes = bytes.fromhex(receipt.preimage)
        computed_hash = hashlib.sha256(preimage_bytes).hexdigest()
        assert computed_hash == receipt.payment_hash

    def test_multiple_sequential_payments(self):
        """Multiple endpoints can be paid in sequence."""
        client = L402Client.with_lnd_rest(LND_REST_HOST, LND_MACAROON_HEX)

        endpoints = [
            ("/api/cheap", 10),
            ("/api/data", 100),
            ("/api/premium", 500),
        ]

        for path, expected_sats in endpoints:
            response = client.get(f"{L402_SERVER_URL}{path}")
            assert response.status == 200
            assert response.paid is True
            assert response.receipt.amount_sats == expected_sats

        assert client.total_spent() == 610


# ---------------------------------------------------------------------------
# LND gRPC backend tests
# ---------------------------------------------------------------------------


@pytest.mark.skipif(not _has_lnd_grpc_credentials(), reason="LND gRPC credentials not available")
class TestLndGrpcBackend:
    """Tests for the LND gRPC backend against regtest."""

    def _cert_and_macaroon_paths(self):
        cert_path = _write_temp_file("lnd-tls.cert", LND_TLS_CERT_BASE64)
        mac_bytes = bytes.fromhex(LND_MACAROON_HEX)
        import tempfile
        mac_path = os.path.join(tempfile.gettempdir(), "l402-regtest-admin.macaroon")
        with open(mac_path, "wb") as f:
            f.write(mac_bytes)
        return cert_path, mac_path

    def test_get_info(self):
        """Verify LND gRPC node connectivity."""
        cert_path, mac_path = self._cert_and_macaroon_paths()
        backend = LndGrpcBackend(LND_GRPC_HOST, cert_path, mac_path)
        info = backend.get_info()
        assert info.pubkey
        assert info.num_active_channels > 0

    def test_get_balance(self):
        """Verify LND gRPC balance query."""
        cert_path, mac_path = self._cert_and_macaroon_paths()
        backend = LndGrpcBackend(LND_GRPC_HOST, cert_path, mac_path)
        balance = backend.get_balance()
        assert balance > 1000


@pytest.mark.skipif(not _has_lnd_grpc_credentials(), reason="LND gRPC credentials not available")
class TestLndGrpcL402Flow:
    """Full L402 protocol flow using LND gRPC backend."""

    def _cert_and_macaroon_paths(self):
        cert_path = _write_temp_file("lnd-tls.cert", LND_TLS_CERT_BASE64)
        mac_bytes = bytes.fromhex(LND_MACAROON_HEX)
        import tempfile
        mac_path = os.path.join(tempfile.gettempdir(), "l402-regtest-admin.macaroon")
        with open(mac_path, "wb") as f:
            f.write(mac_bytes)
        return cert_path, mac_path

    def test_full_flow(self):
        """GET request with automatic L402 payment via LND gRPC."""
        cert_path, mac_path = self._cert_and_macaroon_paths()
        client = L402Client.with_lnd_grpc(LND_GRPC_HOST, cert_path, mac_path)

        response = client.get(f"{L402_SERVER_URL}/api/data")
        assert response.status == 200
        assert response.paid is True
        assert response.receipt is not None
        assert response.receipt.amount_sats == 100

    def test_token_caching(self):
        """Tokens are cached and reused."""
        cert_path, mac_path = self._cert_and_macaroon_paths()
        client = L402Client.with_lnd_grpc(LND_GRPC_HOST, cert_path, mac_path)

        r1 = client.get(f"{L402_SERVER_URL}/api/data")
        assert r1.paid is True

        r2 = client.get(f"{L402_SERVER_URL}/api/data")
        assert r2.paid is False
        assert r2.cached_token is True

    def test_budget_enforcement(self):
        """Budget limits prevent overspending."""
        cert_path, mac_path = self._cert_and_macaroon_paths()
        budget = Budget(per_request_max=50)
        client = L402Client.with_lnd_grpc(
            LND_GRPC_HOST, cert_path, mac_path, budget=budget,
        )

        with pytest.raises(ValueError, match="BudgetExceeded"):
            client.get(f"{L402_SERVER_URL}/api/data")


# ---------------------------------------------------------------------------
# CLN REST backend tests
# ---------------------------------------------------------------------------


@pytest.mark.skipif(not _has_cln_credentials(), reason="CLN credentials not available")
class TestClnRestBackend:
    """Tests for the CLN REST backend against regtest."""

    def test_get_info(self):
        """Verify CLN node connectivity."""
        backend = ClnRestBackend(CLN_REST_URL, CLN_RUNE)
        info = backend.get_info()
        assert info.pubkey
        assert info.num_active_channels > 0

    def test_get_balance(self):
        """Verify CLN balance query."""
        backend = ClnRestBackend(CLN_REST_URL, CLN_RUNE)
        balance = backend.get_balance()
        assert balance > 1000


@pytest.mark.skipif(not _has_cln_credentials(), reason="CLN credentials not available")
class TestClnRestL402Flow:
    """Full L402 protocol flow using CLN REST backend."""

    def test_full_flow(self):
        """GET request with automatic L402 payment."""
        client = L402Client.with_cln_rest(CLN_REST_URL, CLN_RUNE)

        response = client.get(f"{L402_SERVER_URL}/api/data")
        assert response.status == 200
        assert response.paid is True
        assert response.receipt is not None
        assert response.receipt.amount_sats == 100
        assert len(response.receipt.payment_hash) > 0
        assert len(response.receipt.preimage) > 0

    def test_token_caching(self):
        """Tokens are cached and reused on subsequent requests."""
        client = L402Client.with_cln_rest(CLN_REST_URL, CLN_RUNE)

        r1 = client.get(f"{L402_SERVER_URL}/api/data")
        assert r1.paid is True
        assert r1.cached_token is False

        r2 = client.get(f"{L402_SERVER_URL}/api/data")
        assert r2.paid is False
        assert r2.cached_token is True
        assert r2.status == 200

    def test_budget_enforcement(self):
        """Budget limits prevent overspending."""
        budget = Budget(per_request_max=50)
        client = L402Client.with_cln_rest(
            CLN_REST_URL,
            CLN_RUNE,
            budget=budget,
        )

        with pytest.raises(ValueError, match="BudgetExceeded"):
            client.get(f"{L402_SERVER_URL}/api/data")

    def test_receipts(self):
        """Receipt tracking works with CLN backend."""
        client = L402Client.with_cln_rest(CLN_REST_URL, CLN_RUNE)

        client.get(f"{L402_SERVER_URL}/api/cheap")
        client.get(f"{L402_SERVER_URL}/api/data")

        receipts = client.receipts()
        assert len(receipts) == 2
        assert client.total_spent() == 110


# ---------------------------------------------------------------------------
# CLN gRPC backend tests
# ---------------------------------------------------------------------------


@pytest.mark.skipif(not _has_cln_grpc_credentials(), reason="CLN gRPC credentials not available")
class TestClnGrpcBackend:
    """Tests for the CLN gRPC backend against regtest."""

    def _cert_paths(self):
        ca = _write_temp_file("cln-ca.pem", CLN_CA_CERT_BASE64)
        cert = _write_temp_file("cln-client.pem", CLN_CLIENT_CERT_BASE64)
        key = _write_temp_file("cln-client-key.pem", CLN_CLIENT_KEY_BASE64)
        return ca, cert, key

    def test_get_info(self):
        """Verify CLN gRPC node connectivity."""
        ca, cert, key = self._cert_paths()
        backend = ClnGrpcBackend(CLN_GRPC_HOST, ca, cert, key)
        info = backend.get_info()
        assert info.pubkey
        assert info.num_active_channels > 0

    def test_get_balance(self):
        """Verify CLN gRPC balance query."""
        ca, cert, key = self._cert_paths()
        backend = ClnGrpcBackend(CLN_GRPC_HOST, ca, cert, key)
        balance = backend.get_balance()
        assert balance > 1000


@pytest.mark.skipif(not _has_cln_grpc_credentials(), reason="CLN gRPC credentials not available")
class TestClnGrpcL402Flow:
    """Full L402 protocol flow using CLN gRPC backend."""

    def _cert_paths(self):
        ca = _write_temp_file("cln-ca.pem", CLN_CA_CERT_BASE64)
        cert = _write_temp_file("cln-client.pem", CLN_CLIENT_CERT_BASE64)
        key = _write_temp_file("cln-client-key.pem", CLN_CLIENT_KEY_BASE64)
        return ca, cert, key

    def test_full_flow(self):
        """GET request with automatic L402 payment via CLN gRPC."""
        ca, cert, key = self._cert_paths()
        client = L402Client.with_cln_grpc(CLN_GRPC_HOST, ca, cert, key)

        response = client.get(f"{L402_SERVER_URL}/api/data")
        assert response.status == 200
        assert response.paid is True
        assert response.receipt is not None
        assert response.receipt.amount_sats == 100

    def test_token_caching(self):
        """Tokens are cached and reused."""
        ca, cert, key = self._cert_paths()
        client = L402Client.with_cln_grpc(CLN_GRPC_HOST, ca, cert, key)

        r1 = client.get(f"{L402_SERVER_URL}/api/data")
        assert r1.paid is True

        r2 = client.get(f"{L402_SERVER_URL}/api/data")
        assert r2.paid is False
        assert r2.cached_token is True

    def test_budget_enforcement(self):
        """Budget limits prevent overspending."""
        ca, cert, key = self._cert_paths()
        budget = Budget(per_request_max=50)
        client = L402Client.with_cln_grpc(
            CLN_GRPC_HOST, ca, cert, key, budget=budget,
        )

        with pytest.raises(ValueError, match="BudgetExceeded"):
            client.get(f"{L402_SERVER_URL}/api/data")


# ---------------------------------------------------------------------------
# SwissKnife backend tests
# ---------------------------------------------------------------------------


@pytest.mark.skipif(
    not _has_swissknife_credentials(),
    reason="SwissKnife credentials not available",
)
class TestSwissKnifeL402Flow:
    """Full L402 protocol flow using SwissKnife backend."""

    def test_get_info(self):
        """Verify SwissKnife connectivity."""
        backend = SwissKnifeBackend(SWISSKNIFE_API_URL, SWISSKNIFE_API_KEY)
        info = backend.get_info()
        assert info.pubkey  # wallet ID

    def test_get_balance(self):
        """Verify SwissKnife balance query."""
        backend = SwissKnifeBackend(SWISSKNIFE_API_URL, SWISSKNIFE_API_KEY)
        balance = backend.get_balance()
        assert isinstance(balance, int)

    def test_full_flow(self):
        """GET request with automatic L402 payment via SwissKnife."""
        client = L402Client.with_swissknife(SWISSKNIFE_API_URL, SWISSKNIFE_API_KEY)

        response = client.get(f"{L402_SERVER_URL}/api/data")
        assert response.status == 200
        assert response.paid is True
        assert response.receipt is not None
        assert response.receipt.amount_sats == 100

    def test_token_caching(self):
        """Tokens are cached and reused."""
        client = L402Client.with_swissknife(SWISSKNIFE_API_URL, SWISSKNIFE_API_KEY)

        r1 = client.get(f"{L402_SERVER_URL}/api/data")
        assert r1.paid is True

        r2 = client.get(f"{L402_SERVER_URL}/api/data")
        assert r2.paid is False
        assert r2.cached_token is True

    def test_budget_enforcement(self):
        """Budget limits are enforced with SwissKnife backend."""
        budget = Budget(per_request_max=50)
        client = L402Client.with_swissknife(
            SWISSKNIFE_API_URL,
            SWISSKNIFE_API_KEY,
            budget=budget,
        )

        with pytest.raises(ValueError, match="BudgetExceeded"):
            client.get(f"{L402_SERVER_URL}/api/data")

    def test_receipts(self):
        """Receipt tracking works with SwissKnife backend."""
        client = L402Client.with_swissknife(SWISSKNIFE_API_URL, SWISSKNIFE_API_KEY)

        client.get(f"{L402_SERVER_URL}/api/cheap")
        client.get(f"{L402_SERVER_URL}/api/data")

        receipts = client.receipts()
        assert len(receipts) == 2
        assert client.total_spent() == 110
