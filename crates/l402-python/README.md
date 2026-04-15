<div align="center">
  <h1>L402sdk</h1>

  <p>
    <strong>L402 client SDK for Python — pay for APIs with Lightning</strong>
  </p>

  <p>
    <a href="https://pypi.org/project/l402/"><img alt="PyPI" src="https://img.shields.io/pypi/v/L402sdk.svg"/></a>
    <a href="https://pypi.org/project/l402/"><img alt="PyPI downloads" src="https://img.shields.io/pypi/dm/L402sdk.svg"/></a>
    <a href="https://github.com/lightninglabs/L402sdk/blob/main/LICENSE-MIT"><img alt="MIT or Apache-2.0 Licensed" src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg"/></a>
  </p>

</div>

Built in Rust via [PyO3](https://pyo3.rs). Supports LND, CLN, and SwissKnife Lightning backends.

## Install

```bash
pip install l402
```

## Quick Start

```python
from l402 import L402Client, Budget

# Create a client backed by LND REST
client = L402Client.with_lnd_rest(
    "https://localhost:8080",
    "hex-encoded-admin-macaroon",
    budget=Budget(per_request_max=1000, daily_max=50_000),
)

# L402 negotiation happens automatically
response = client.get("https://api.example.com/paid-resource")
print(response.status)  # 200
print(response.paid)    # True

# Payment receipt
receipt = response.receipt
print(receipt.amount_sats)    # 100
print(receipt.payment_hash)   # hex string

# Budget tracking
print(client.total_spent())   # 100
print(client.receipts())      # [Receipt(...)]
```

## Lightning Backends

```python
from l402 import LndRestBackend, ClnRestBackend, SwissKnifeBackend, L402Client

# LND REST
client = L402Client.with_lnd_rest("https://localhost:8080", "macaroon_hex")

# Core Lightning (CLN) REST
client = L402Client.with_cln_rest("https://localhost:3010", "rune_token")

# SwissKnife
client = L402Client.with_swissknife("https://app.numeraire.tech", "sk-...")
```

## Budget Control

```python
from l402 import Budget

budget = Budget(
    per_request_max=100,   # Max sats per request
    hourly_max=1000,       # Max sats per hour
    daily_max=5000,        # Max sats per day
    total_max=50000,       # Max sats total
)

# Or no limits
budget = Budget.unlimited()
```

## API Reference

### `L402Response`

```python
response.status        # HTTP status code (int)
response.body          # Response body (str)
response.paid          # Whether a payment was made (bool)
response.cached_token  # Whether a cached token was used (bool)
response.receipt       # Payment receipt or None
response.json()        # Response body parsed as JSON
```

### `Receipt`

```python
receipt.timestamp        # Unix timestamp (seconds)
receipt.endpoint         # URL accessed
receipt.amount_sats      # Amount paid (sats)
receipt.fee_sats         # Routing fee (sats)
receipt.payment_hash     # Payment hash (hex)
receipt.preimage         # Preimage (hex)
receipt.response_status  # HTTP status after payment
receipt.total_cost_sats  # amount + fee
```

## License

MIT OR Apache-2.0
