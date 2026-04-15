# Design Doc 010: Python Bindings via PyO3

## Problem

Python is the dominant language for AI agent frameworks. LangChain alone has ~200M monthly PyPI downloads. Developers building L402-enabled agents in Python currently have no native library — they must implement the full protocol flow manually or give up and use API keys.

L402sdk's Rust core was designed from day one for cross-language FFI. Python bindings are the first and highest-impact language target.

## Proposed Design

### Crate: `crates/l402-python`

A PyO3 module (`L402sdk`) that exposes the Rust core to Python. Uses maturin for building native wheels.

### Python API

```python
from l402 import L402Client, Budget, InMemoryTokenStore

# Create a budget
budget = Budget(
    per_request_max=100,
    hourly_max=1000,
    daily_max=5000,
    total_max=50000,
)

# Create a client with the mock backend (for testing)
client = L402Client(
    backend="mock",
    token_store=InMemoryTokenStore(),
    budget=budget,
    max_fee_sats=100,
)

# Make an L402-aware request (async)
response = await client.get("https://api.example.com/resource")
print(response.status)
print(response.text())

# Check spending
print(client.total_spent())
receipts = client.receipts()
```

### Architecture

```
Python user code
      │
      ▼
L402sdk (PyO3 module)
├── L402Client     → wraps l402_core::L402Client
├── Budget         → wraps l402_core::budget::Budget
├── Receipt        → wraps l402_core::receipt::Receipt
├── TokenStore     → wraps l402_core::cache::InMemoryTokenStore
└── MockBackend    → wraps l402_mock (for testing)
      │
      ▼
l402-core (Rust) + l402-mock (Rust)
```

### Key Decisions

1. **Async via `pyo3-async-runtimes`**: The Rust core is async (tokio). We use `pyo3-async-runtimes` with `tokio-runtime` feature to bridge Rust futures to Python `asyncio`. Python users can `await` L402 operations naturally.

2. **Minimal surface area**: Expose only what Python developers need — `L402Client`, `Budget`, `Receipt`, `InMemoryTokenStore`, and backend constructors. Internal types stay internal.

3. **Backend as enum parameter**: Instead of exposing the `LnBackend` trait to Python (complex), backends are selected via string parameter (`"mock"`, `"lnd"`, `"swissknife"`) with backend-specific config. This is more Pythonic.

4. **Error mapping**: Rust `ClientError` variants map to Python exceptions: `L402Error` (base), `BudgetExceededError`, `PaymentFailedError`, `ChallengeParseError`.

5. **Maturin build**: Uses `maturin` with `pyo3` backend. Generates wheels for Linux (x86_64, aarch64), macOS (x86_64, aarch64), and Windows (x86_64). CI handles cross-compilation.

### Alternatives Considered

**Pure Python implementation** (like l402-ai-sdk in TypeScript): Faster to ship but means maintaining two implementations of the same logic. The whole point of "Rust first, FFI everywhere" is to write the protocol engine once. Rejected.

**cffi bindings**: Lower-level than PyO3, requires manual memory management in Python. PyO3 provides a much better developer experience with native Python objects. Rejected.

### Testing Plan

1. **Rust-side PyO3 tests**: Verify Python objects are constructed correctly
2. **Python pytest suite**: End-to-end tests using the mock backend
3. **CI**: GitHub Actions with maturin build + pytest
4. **Integration test**: Full L402 flow using `l402-mock` server from Python

### Files

```
crates/l402-python/
├── Cargo.toml
├── pyproject.toml
├── src/
│   └── lib.rs          # PyO3 module definition + all Python-facing types
├── python/
│   └── l402/
│       ├── __init__.py  # Re-exports + type stubs
│       └── py.typed     # PEP 561 marker
└── tests/
    └── test_L402sdk.py  # pytest test suite
```
