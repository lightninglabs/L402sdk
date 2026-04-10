#![allow(clippy::doc_markdown)]

//! Python bindings for the bolt402 L402 client SDK.
//!
//! Exposes the Rust core to Python via `PyO3`, enabling Python AI agent
//! frameworks (`LangChain`, `CrewAI`, `AutoGen`, `LlamaIndex`) to use
//! L402-gated APIs with Lightning payments.
//!
//! Supports LND (gRPC + REST), CLN (gRPC + REST), and `SwissKnife` backends.

use std::collections::HashMap;
use std::sync::Arc;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

use bolt402_cln::{ClnGrpcBackend, ClnRestBackend};
use bolt402_core::budget::Budget as RustBudget;
use bolt402_core::cache::InMemoryTokenStore;
use bolt402_core::receipt::Receipt as RustReceipt;
use bolt402_core::{L402Client as RustClient, L402ClientConfig};
use bolt402_lnd::{LndGrpcBackend, LndRestBackend};
use bolt402_proto::{LnBackend, NodeInfo, PaymentResult};
use bolt402_swissknife::SwissKnifeBackend;

/// Runtime handle shared across Python bindings.
///
/// We create one tokio runtime and reuse it for all async operations,
/// running them from Python synchronous context via `block_on`.
fn get_runtime() -> &'static tokio::runtime::Runtime {
    use std::sync::OnceLock;
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        // Install rustls crypto provider (needed for gRPC TLS).
        // Ignore errors if already installed by another thread.
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to create tokio runtime")
    })
}

// ---------------------------------------------------------------------------
// Budget
// ---------------------------------------------------------------------------

/// Budget configuration for L402 payment limits.
///
/// Prevents runaway spending by enforcing caps at multiple granularities.
#[pyclass(name = "Budget", from_py_object)]
#[derive(Debug, Clone)]
struct PyBudget {
    inner: RustBudget,
}

#[pymethods]
impl PyBudget {
    /// Create a new budget with optional limits.
    #[new]
    #[pyo3(signature = (*, per_request_max=None, hourly_max=None, daily_max=None, total_max=None, domain_budgets=None))]
    fn new(
        per_request_max: Option<u64>,
        hourly_max: Option<u64>,
        daily_max: Option<u64>,
        total_max: Option<u64>,
        domain_budgets: Option<HashMap<String, PyBudget>>,
    ) -> Self {
        let rust_domain_budgets = domain_budgets
            .unwrap_or_default()
            .into_iter()
            .map(|(k, v)| (k, v.inner))
            .collect();

        Self {
            inner: RustBudget {
                per_request_max,
                hourly_max,
                daily_max,
                total_max,
                domain_budgets: rust_domain_budgets,
            },
        }
    }

    /// Create an unlimited budget with no restrictions.
    #[staticmethod]
    fn unlimited() -> Self {
        Self {
            inner: RustBudget::unlimited(),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Budget(per_request_max={}, hourly_max={}, daily_max={}, total_max={})",
            fmt_opt(self.inner.per_request_max),
            fmt_opt(self.inner.hourly_max),
            fmt_opt(self.inner.daily_max),
            fmt_opt(self.inner.total_max),
        )
    }
}

fn fmt_opt(v: Option<u64>) -> String {
    match v {
        Some(n) => n.to_string(),
        None => "None".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Receipt
// ---------------------------------------------------------------------------

/// A payment receipt for an L402 transaction.
///
/// Contains all details of a Lightning payment made to access an
/// L402-gated resource, useful for audit trails and cost analysis.
#[pyclass(name = "Receipt", from_py_object)]
#[derive(Debug, Clone)]
struct PyReceipt {
    inner: RustReceipt,
}

#[pymethods]
impl PyReceipt {
    /// Unix timestamp (seconds) of the payment.
    #[getter]
    fn timestamp(&self) -> u64 {
        self.inner.timestamp
    }

    /// The endpoint that was accessed.
    #[getter]
    fn endpoint(&self) -> &str {
        &self.inner.endpoint
    }

    /// Amount paid in satoshis (excluding routing fees).
    #[getter]
    fn amount_sats(&self) -> u64 {
        self.inner.amount_sats
    }

    /// Routing fee paid in satoshis.
    #[getter]
    fn fee_sats(&self) -> u64 {
        self.inner.fee_sats
    }

    /// Hex-encoded payment hash.
    #[getter]
    fn payment_hash(&self) -> &str {
        &self.inner.payment_hash
    }

    /// Hex-encoded preimage (proof of payment).
    #[getter]
    fn preimage(&self) -> &str {
        &self.inner.preimage
    }

    /// HTTP response status code.
    #[getter]
    fn response_status(&self) -> u16 {
        self.inner.response_status
    }

    /// Total latency in milliseconds.
    #[getter]
    fn latency_ms(&self) -> u64 {
        self.inner.latency_ms
    }

    /// Total cost (amount + routing fee) in satoshis.
    fn total_cost_sats(&self) -> u64 {
        self.inner.total_cost_sats()
    }

    fn __repr__(&self) -> String {
        format!(
            "Receipt(endpoint='{}', amount_sats={}, fee_sats={}, status={})",
            self.inner.endpoint,
            self.inner.amount_sats,
            self.inner.fee_sats,
            self.inner.response_status,
        )
    }

    /// Serialize the receipt to a JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("serialization error: {e}")))
    }
}

// ---------------------------------------------------------------------------
// PaymentResult
// ---------------------------------------------------------------------------

/// Result of a Lightning payment.
///
/// Contains proof-of-payment data returned by the backend after
/// successfully paying a BOLT11 invoice.
#[pyclass(name = "PaymentResult", from_py_object)]
#[derive(Debug, Clone)]
struct PyPaymentResult {
    inner: PaymentResult,
}

#[pymethods]
impl PyPaymentResult {
    /// Hex-encoded payment preimage (proof of payment).
    #[getter]
    fn preimage(&self) -> &str {
        &self.inner.preimage
    }

    /// Hex-encoded payment hash.
    #[getter]
    fn payment_hash(&self) -> &str {
        &self.inner.payment_hash
    }

    /// Amount paid in satoshis (excluding routing fees).
    #[getter]
    fn amount_sats(&self) -> u64 {
        self.inner.amount_sats
    }

    /// Routing fee paid in satoshis.
    #[getter]
    fn fee_sats(&self) -> u64 {
        self.inner.fee_sats
    }

    /// Total cost (amount + fee) in satoshis.
    fn total_cost_sats(&self) -> u64 {
        self.inner.amount_sats + self.inner.fee_sats
    }

    fn __repr__(&self) -> String {
        format!(
            "PaymentResult(amount_sats={}, fee_sats={}, hash='{}')",
            self.inner.amount_sats, self.inner.fee_sats, self.inner.payment_hash,
        )
    }
}

// ---------------------------------------------------------------------------
// NodeInfo
// ---------------------------------------------------------------------------

/// Information about a Lightning node.
///
/// Returned by backend `get_info()` calls. Contains the node's identity
/// and channel count.
#[pyclass(name = "NodeInfo", from_py_object)]
#[derive(Debug, Clone)]
struct PyNodeInfo {
    inner: NodeInfo,
}

#[pymethods]
impl PyNodeInfo {
    /// Node public key (hex-encoded).
    #[getter]
    fn pubkey(&self) -> &str {
        &self.inner.pubkey
    }

    /// Node alias.
    #[getter]
    fn alias(&self) -> &str {
        &self.inner.alias
    }

    /// Number of active channels.
    #[getter]
    fn num_active_channels(&self) -> u32 {
        self.inner.num_active_channels
    }

    fn __repr__(&self) -> String {
        format!(
            "NodeInfo(alias='{}', pubkey='{}', channels={})",
            self.inner.alias, self.inner.pubkey, self.inner.num_active_channels,
        )
    }
}

// ---------------------------------------------------------------------------
// Backend wrappers
// ---------------------------------------------------------------------------

/// LND REST backend for Lightning payments.
///
/// Connects to an LND node via its REST API (default port 8080).
/// Authenticated with a hex-encoded admin macaroon.
///
/// Example::
///
///     from bolt402 import LndRestBackend
///
///     backend = LndRestBackend("https://localhost:8080", "deadbeef...")
///     info = backend.get_info()
///     print(info.alias)
#[pyclass(name = "LndRestBackend", from_py_object)]
#[derive(Debug, Clone)]
struct PyLndRestBackend {
    inner: LndRestBackend,
}

#[pymethods]
impl PyLndRestBackend {
    /// Create a new LND REST backend.
    ///
    /// Args:
    ///     url: LND REST API URL (e.g. ``https://localhost:8080``)
    ///     macaroon: Hex-encoded admin macaroon
    #[new]
    fn new(url: &str, macaroon: &str) -> PyResult<Self> {
        let inner = LndRestBackend::new(url, macaroon)
            .map_err(|e| PyRuntimeError::new_err(format!("failed to create LND backend: {e}")))?;
        Ok(Self { inner })
    }

    /// Pay a BOLT11 Lightning invoice.
    ///
    /// Args:
    ///     bolt11: BOLT11 invoice string
    ///     max_fee_sats: Maximum routing fee in satoshis
    ///
    /// Returns:
    ///     PaymentResult with preimage, hash, amount, and fee.
    fn pay_invoice(&self, bolt11: &str, max_fee_sats: u64) -> PyResult<PyPaymentResult> {
        let rt = get_runtime();
        let inner = self.inner.clone();
        let bolt11 = bolt11.to_string();

        let result = rt.block_on(async move { inner.pay_invoice(&bolt11, max_fee_sats).await });

        match result {
            Ok(r) => Ok(PyPaymentResult { inner: r }),
            Err(e) => Err(PyRuntimeError::new_err(format!("payment failed: {e}"))),
        }
    }

    /// Get the current spendable balance in satoshis.
    fn get_balance(&self) -> PyResult<u64> {
        let rt = get_runtime();
        let inner = self.inner.clone();

        rt.block_on(async move { inner.get_balance().await })
            .map_err(|e| PyRuntimeError::new_err(format!("get_balance failed: {e}")))
    }

    /// Get information about the connected Lightning node.
    fn get_info(&self) -> PyResult<PyNodeInfo> {
        let rt = get_runtime();
        let inner = self.inner.clone();

        let result = rt.block_on(async move { inner.get_info().await });

        match result {
            Ok(info) => Ok(PyNodeInfo { inner: info }),
            Err(e) => Err(PyRuntimeError::new_err(format!("get_info failed: {e}"))),
        }
    }

    #[allow(clippy::unused_self)]
    fn __repr__(&self) -> String {
        "LndRestBackend(...)".to_string()
    }
}

/// LND gRPC backend for Lightning payments.
///
/// Connects to an LND node via its gRPC interface (default port 10009).
/// Authenticated with TLS certificates and admin macaroon.
///
/// Example::
///
///     from bolt402 import LndGrpcBackend
///
///     backend = LndGrpcBackend("https://localhost:10009", "/path/to/tls.cert", "/path/to/admin.macaroon")
///     info = backend.get_info()
///     print(info.alias)
#[pyclass(name = "LndGrpcBackend", from_py_object)]
#[derive(Debug, Clone)]
struct PyLndGrpcBackend {
    inner: Arc<LndGrpcBackend>,
}

#[pymethods]
impl PyLndGrpcBackend {
    /// Create a new LND gRPC backend.
    ///
    /// Args:
    ///     address: LND gRPC address (e.g. ``https://localhost:10009``)
    ///     tls_cert_path: Path to LND's ``tls.cert`` file
    ///     macaroon_path: Path to an admin macaroon file
    #[new]
    fn new(address: &str, tls_cert_path: &str, macaroon_path: &str) -> PyResult<Self> {
        let rt = get_runtime();
        let backend = rt
            .block_on(LndGrpcBackend::connect(
                address,
                tls_cert_path,
                macaroon_path,
            ))
            .map_err(|e| PyRuntimeError::new_err(format!("failed to connect to LND gRPC: {e}")))?;
        Ok(Self {
            inner: Arc::new(backend),
        })
    }

    /// Pay a BOLT11 Lightning invoice.
    fn pay_invoice(&self, bolt11: &str, max_fee_sats: u64) -> PyResult<PyPaymentResult> {
        let rt = get_runtime();
        let inner = Arc::clone(&self.inner);
        let bolt11 = bolt11.to_string();
        let result = rt.block_on(async move { inner.pay_invoice(&bolt11, max_fee_sats).await });
        match result {
            Ok(r) => Ok(PyPaymentResult { inner: r }),
            Err(e) => Err(PyRuntimeError::new_err(format!("payment failed: {e}"))),
        }
    }

    /// Get the current spendable balance in satoshis.
    fn get_balance(&self) -> PyResult<u64> {
        let rt = get_runtime();
        let inner = self.inner.clone();
        rt.block_on(async move { inner.get_balance().await })
            .map_err(|e| PyRuntimeError::new_err(format!("get_balance failed: {e}")))
    }

    /// Get information about the connected Lightning node.
    fn get_info(&self) -> PyResult<PyNodeInfo> {
        let rt = get_runtime();
        let inner = self.inner.clone();
        let result = rt.block_on(async move { inner.get_info().await });
        match result {
            Ok(info) => Ok(PyNodeInfo { inner: info }),
            Err(e) => Err(PyRuntimeError::new_err(format!("get_info failed: {e}"))),
        }
    }

    #[allow(clippy::unused_self)]
    fn __repr__(&self) -> String {
        "LndGrpcBackend(...)".to_string()
    }
}

/// CLN REST backend for Lightning payments.
///
/// Connects to a Core Lightning node via the CLN REST interface.
/// Authenticated with a rune token (CLN's native bearer token system).
///
/// Example::
///
///     from bolt402 import ClnRestBackend
///
///     backend = ClnRestBackend("https://localhost:3001", "rune_token...")
///     info = backend.get_info()
///     print(info.alias)
#[pyclass(name = "ClnRestBackend", from_py_object)]
#[derive(Debug, Clone)]
struct PyClnRestBackend {
    inner: ClnRestBackend,
}

#[pymethods]
impl PyClnRestBackend {
    /// Create a new CLN REST backend using rune authentication.
    ///
    /// Args:
    ///     url: CLN REST API URL (e.g. ``https://localhost:3001``)
    ///     rune: Rune token string
    #[new]
    fn new(url: &str, rune: &str) -> PyResult<Self> {
        let inner = ClnRestBackend::new(url, rune)
            .map_err(|e| PyRuntimeError::new_err(format!("failed to create CLN backend: {e}")))?;
        Ok(Self { inner })
    }

    /// Pay a BOLT11 Lightning invoice.
    ///
    /// Args:
    ///     bolt11: BOLT11 invoice string
    ///     max_fee_sats: Maximum routing fee in satoshis
    ///
    /// Returns:
    ///     PaymentResult with preimage, hash, amount, and fee.
    fn pay_invoice(&self, bolt11: &str, max_fee_sats: u64) -> PyResult<PyPaymentResult> {
        let rt = get_runtime();
        let inner = self.inner.clone();
        let bolt11 = bolt11.to_string();

        let result = rt.block_on(async move { inner.pay_invoice(&bolt11, max_fee_sats).await });

        match result {
            Ok(r) => Ok(PyPaymentResult { inner: r }),
            Err(e) => Err(PyRuntimeError::new_err(format!("payment failed: {e}"))),
        }
    }

    /// Get the current spendable balance in satoshis.
    fn get_balance(&self) -> PyResult<u64> {
        let rt = get_runtime();
        let inner = self.inner.clone();

        rt.block_on(async move { inner.get_balance().await })
            .map_err(|e| PyRuntimeError::new_err(format!("get_balance failed: {e}")))
    }

    /// Get information about the connected Lightning node.
    fn get_info(&self) -> PyResult<PyNodeInfo> {
        let rt = get_runtime();
        let inner = self.inner.clone();

        let result = rt.block_on(async move { inner.get_info().await });

        match result {
            Ok(info) => Ok(PyNodeInfo { inner: info }),
            Err(e) => Err(PyRuntimeError::new_err(format!("get_info failed: {e}"))),
        }
    }

    #[allow(clippy::unused_self)]
    fn __repr__(&self) -> String {
        "ClnRestBackend(...)".to_string()
    }
}

/// CLN gRPC backend for Lightning payments.
///
/// Connects to a Core Lightning node via gRPC with mTLS authentication.
///
/// Example::
///
///     from bolt402 import ClnGrpcBackend
///
///     backend = ClnGrpcBackend(
///         "https://localhost:9736",
///         "/path/to/ca.pem",
///         "/path/to/client.pem",
///         "/path/to/client-key.pem",
///     )
///     info = backend.get_info()
///     print(info.alias)
#[pyclass(name = "ClnGrpcBackend", from_py_object)]
#[derive(Debug, Clone)]
struct PyClnGrpcBackend {
    inner: Arc<ClnGrpcBackend>,
}

#[pymethods]
impl PyClnGrpcBackend {
    /// Create a new CLN gRPC backend using mTLS.
    ///
    /// Args:
    ///     address: CLN gRPC address (e.g. ``https://localhost:9736``)
    ///     ca_cert_path: Path to the CA certificate (``ca.pem``)
    ///     client_cert_path: Path to the client certificate (``client.pem``)
    ///     client_key_path: Path to the client key (``client-key.pem``)
    #[new]
    fn new(
        address: &str,
        ca_cert_path: &str,
        client_cert_path: &str,
        client_key_path: &str,
    ) -> PyResult<Self> {
        let rt = get_runtime();
        let backend = rt
            .block_on(ClnGrpcBackend::connect(
                address,
                ca_cert_path,
                client_cert_path,
                client_key_path,
            ))
            .map_err(|e| PyRuntimeError::new_err(format!("failed to connect to CLN gRPC: {e}")))?;
        Ok(Self {
            inner: Arc::new(backend),
        })
    }

    /// Pay a BOLT11 Lightning invoice.
    fn pay_invoice(&self, bolt11: &str, max_fee_sats: u64) -> PyResult<PyPaymentResult> {
        let rt = get_runtime();
        let inner = self.inner.clone();
        let bolt11 = bolt11.to_string();
        let result = rt.block_on(async move { inner.pay_invoice(&bolt11, max_fee_sats).await });
        match result {
            Ok(r) => Ok(PyPaymentResult { inner: r }),
            Err(e) => Err(PyRuntimeError::new_err(format!("payment failed: {e}"))),
        }
    }

    /// Get the current spendable balance in satoshis.
    fn get_balance(&self) -> PyResult<u64> {
        let rt = get_runtime();
        let inner = self.inner.clone();
        rt.block_on(async move { inner.get_balance().await })
            .map_err(|e| PyRuntimeError::new_err(format!("get_balance failed: {e}")))
    }

    /// Get information about the connected Lightning node.
    fn get_info(&self) -> PyResult<PyNodeInfo> {
        let rt = get_runtime();
        let inner = self.inner.clone();
        let result = rt.block_on(async move { inner.get_info().await });
        match result {
            Ok(info) => Ok(PyNodeInfo { inner: info }),
            Err(e) => Err(PyRuntimeError::new_err(format!("get_info failed: {e}"))),
        }
    }

    #[allow(clippy::unused_self)]
    fn __repr__(&self) -> String {
        "ClnGrpcBackend(...)".to_string()
    }
}

/// SwissKnife REST backend for Lightning payments.
///
/// Connects to a Numeraire SwissKnife instance using API key
/// authentication. SwissKnife wallets are custodial Lightning accounts.
///
/// Example::
///
///     from bolt402 import SwissKnifeBackend
///
///     backend = SwissKnifeBackend("https://api.numeraire.tech", "sk-...")
///     balance = backend.get_balance()
///     print(f"Balance: {balance} sats")
#[pyclass(name = "SwissKnifeBackend", from_py_object)]
#[derive(Debug, Clone)]
struct PySwissKnifeBackend {
    inner: SwissKnifeBackend,
}

#[pymethods]
impl PySwissKnifeBackend {
    /// Create a new SwissKnife backend.
    ///
    /// Args:
    ///     url: SwissKnife API URL (e.g. ``https://api.numeraire.tech``)
    ///     api_key: API key for authentication
    #[new]
    fn new(url: &str, api_key: &str) -> Self {
        Self {
            inner: SwissKnifeBackend::new(url, api_key),
        }
    }

    /// Pay a BOLT11 Lightning invoice.
    ///
    /// Args:
    ///     bolt11: BOLT11 invoice string
    ///     max_fee_sats: Maximum routing fee in satoshis
    ///
    /// Returns:
    ///     PaymentResult with preimage, hash, amount, and fee.
    fn pay_invoice(&self, bolt11: &str, max_fee_sats: u64) -> PyResult<PyPaymentResult> {
        let rt = get_runtime();
        let inner = self.inner.clone();
        let bolt11 = bolt11.to_string();

        let result = rt.block_on(async move { inner.pay_invoice(&bolt11, max_fee_sats).await });

        match result {
            Ok(r) => Ok(PyPaymentResult { inner: r }),
            Err(e) => Err(PyRuntimeError::new_err(format!("payment failed: {e}"))),
        }
    }

    /// Get the current spendable balance in satoshis.
    fn get_balance(&self) -> PyResult<u64> {
        let rt = get_runtime();
        let inner = self.inner.clone();

        rt.block_on(async move { inner.get_balance().await })
            .map_err(|e| PyRuntimeError::new_err(format!("get_balance failed: {e}")))
    }

    /// Get information about the connected SwissKnife wallet.
    fn get_info(&self) -> PyResult<PyNodeInfo> {
        let rt = get_runtime();
        let inner = self.inner.clone();

        let result = rt.block_on(async move { inner.get_info().await });

        match result {
            Ok(info) => Ok(PyNodeInfo { inner: info }),
            Err(e) => Err(PyRuntimeError::new_err(format!("get_info failed: {e}"))),
        }
    }

    #[allow(clippy::unused_self)]
    fn __repr__(&self) -> String {
        "SwissKnifeBackend(...)".to_string()
    }
}

// ---------------------------------------------------------------------------
// L402Response
// ---------------------------------------------------------------------------

/// Response from an L402-aware HTTP request.
///
/// Wraps the HTTP response with metadata about whether a Lightning payment
/// was made to obtain access.
#[pyclass(name = "L402Response")]
struct PyL402Response {
    status: u16,
    paid: bool,
    cached_token: bool,
    receipt: Option<PyReceipt>,
    body: String,
    headers: HashMap<String, String>,
}

#[pymethods]
impl PyL402Response {
    /// HTTP status code.
    #[getter]
    fn status(&self) -> u16 {
        self.status
    }

    /// Whether a Lightning payment was made for this request.
    #[getter]
    fn paid(&self) -> bool {
        self.paid
    }

    /// Whether a cached L402 token was reused (no new payment needed).
    #[getter]
    fn cached_token(&self) -> bool {
        self.cached_token
    }

    /// Payment receipt, if a payment was made.
    #[getter]
    fn receipt(&self) -> Option<PyReceipt> {
        self.receipt.clone()
    }

    /// Response body as text.
    fn text(&self) -> &str {
        &self.body
    }

    /// Parse the response body as JSON.
    fn json<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let json_module = py.import("json")?;
        json_module.call_method1("loads", (&self.body,))
    }

    /// Response headers as a dictionary.
    #[getter]
    fn headers(&self) -> HashMap<String, String> {
        self.headers.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "L402Response(status={}, paid={}, cached_token={})",
            self.status, self.paid, self.cached_token,
        )
    }
}

// ---------------------------------------------------------------------------
// L402Client
// ---------------------------------------------------------------------------

/// L402 client that handles the full payment-gated HTTP flow.
///
/// Intercepts HTTP 402 responses, parses L402 challenges, pays Lightning
/// invoices via the configured backend, caches tokens, enforces budgets,
/// and records receipts.
///
/// Use one of the static constructor methods to create a client with a
/// specific backend:
///
/// - ``L402Client.with_lnd_grpc(...)``
/// - ``L402Client.with_lnd_rest(...)``
/// - ``L402Client.with_cln_grpc(...)``
/// - ``L402Client.with_cln_rest(...)``
/// - ``L402Client.with_swissknife(...)``
///
/// Example::
///
///     from bolt402 import L402Client, Budget
///
///     client = L402Client.with_lnd_rest(
///         "https://localhost:8080",
///         "deadbeef...",
///     )
///     response = client.get("https://api.example.com/data")
///     print(response.status, response.paid)
#[pyclass(name = "L402Client")]
struct PyL402Client {
    inner: RustClient,
}

#[pymethods]
impl PyL402Client {
    /// Create an L402 client backed by LND REST.
    ///
    /// Args:
    ///     url: LND REST API URL (e.g. ``https://localhost:8080``)
    ///     macaroon: Hex-encoded admin macaroon
    ///     budget: Optional budget configuration (default: unlimited)
    ///     max_fee_sats: Maximum routing fee in satoshis (default: 100)
    #[staticmethod]
    #[pyo3(signature = (url, macaroon, budget=None, max_fee_sats=100))]
    fn with_lnd_rest(
        url: &str,
        macaroon: &str,
        budget: Option<PyBudget>,
        max_fee_sats: u64,
    ) -> PyResult<Self> {
        let backend = LndRestBackend::new(url, macaroon)
            .map_err(|e| PyRuntimeError::new_err(format!("failed to create LND backend: {e}")))?;

        build_client(backend, budget, max_fee_sats)
    }

    /// Create an L402 client backed by LND gRPC.
    ///
    /// Args:
    ///     address: LND gRPC address (e.g. ``https://localhost:10009``)
    ///     tls_cert_path: Path to LND's ``tls.cert`` file
    ///     macaroon_path: Path to an admin macaroon file
    ///     budget: Optional budget configuration (default: unlimited)
    ///     max_fee_sats: Maximum routing fee in satoshis (default: 100)
    #[staticmethod]
    #[pyo3(signature = (address, tls_cert_path, macaroon_path, budget=None, max_fee_sats=100))]
    fn with_lnd_grpc(
        address: &str,
        tls_cert_path: &str,
        macaroon_path: &str,
        budget: Option<PyBudget>,
        max_fee_sats: u64,
    ) -> PyResult<Self> {
        let rt = get_runtime();
        let backend = rt
            .block_on(LndGrpcBackend::connect(
                address,
                tls_cert_path,
                macaroon_path,
            ))
            .map_err(|e| PyRuntimeError::new_err(format!("failed to connect to LND gRPC: {e}")))?;

        build_client(backend, budget, max_fee_sats)
    }

    /// Create an L402 client backed by CLN REST with rune auth.
    ///
    /// Args:
    ///     url: CLN REST API URL (e.g. ``https://localhost:3001``)
    ///     rune: Rune token string
    ///     budget: Optional budget configuration (default: unlimited)
    ///     max_fee_sats: Maximum routing fee in satoshis (default: 100)
    #[staticmethod]
    #[pyo3(signature = (url, rune, budget=None, max_fee_sats=100))]
    fn with_cln_rest(
        url: &str,
        rune: &str,
        budget: Option<PyBudget>,
        max_fee_sats: u64,
    ) -> PyResult<Self> {
        let backend = ClnRestBackend::new(url, rune)
            .map_err(|e| PyRuntimeError::new_err(format!("failed to create CLN backend: {e}")))?;

        build_client(backend, budget, max_fee_sats)
    }

    /// Create an L402 client backed by CLN gRPC with mTLS.
    ///
    /// Args:
    ///     address: CLN gRPC address (e.g. ``https://localhost:9736``)
    ///     ca_cert_path: Path to the CA certificate (``ca.pem``)
    ///     client_cert_path: Path to the client certificate (``client.pem``)
    ///     client_key_path: Path to the client key (``client-key.pem``)
    ///     budget: Optional budget configuration (default: unlimited)
    ///     max_fee_sats: Maximum routing fee in satoshis (default: 100)
    #[staticmethod]
    #[pyo3(signature = (address, ca_cert_path, client_cert_path, client_key_path, budget=None, max_fee_sats=100))]
    fn with_cln_grpc(
        address: &str,
        ca_cert_path: &str,
        client_cert_path: &str,
        client_key_path: &str,
        budget: Option<PyBudget>,
        max_fee_sats: u64,
    ) -> PyResult<Self> {
        let rt = get_runtime();
        let backend = rt
            .block_on(ClnGrpcBackend::connect(
                address,
                ca_cert_path,
                client_cert_path,
                client_key_path,
            ))
            .map_err(|e| PyRuntimeError::new_err(format!("failed to connect to CLN gRPC: {e}")))?;

        build_client(backend, budget, max_fee_sats)
    }

    /// Create an L402 client backed by SwissKnife REST.
    ///
    /// Args:
    ///     url: SwissKnife API URL (e.g. ``https://api.numeraire.tech``)
    ///     api_key: API key for authentication
    ///     budget: Optional budget configuration (default: unlimited)
    ///     max_fee_sats: Maximum routing fee in satoshis (default: 100)
    #[staticmethod]
    #[pyo3(signature = (url, api_key, budget=None, max_fee_sats=100))]
    fn with_swissknife(
        url: &str,
        api_key: &str,
        budget: Option<PyBudget>,
        max_fee_sats: u64,
    ) -> PyResult<Self> {
        let backend = SwissKnifeBackend::new(url, api_key);
        build_client(backend, budget, max_fee_sats)
    }

    /// Send a GET request, automatically handling L402 payment challenges.
    ///
    /// If the server responds with HTTP 402, the client will parse the
    /// challenge, check the budget, pay the Lightning invoice, cache the
    /// token, and retry.
    fn get(&self, url: &str) -> PyResult<PyL402Response> {
        let rt = get_runtime();
        let result = rt.block_on(async { self.inner.get(url).await });
        convert_response(rt, result)
    }

    /// Send a POST request with an optional JSON body.
    ///
    /// See ``get()`` for the full L402 flow description.
    #[pyo3(signature = (url, body=None))]
    fn post(&self, url: &str, body: Option<&str>) -> PyResult<PyL402Response> {
        let rt = get_runtime();
        let result = rt.block_on(async { self.inner.post(url, body).await });
        convert_response(rt, result)
    }

    /// Get all recorded payment receipts.
    fn receipts(&self) -> Vec<PyReceipt> {
        let rt = get_runtime();
        let receipts = rt.block_on(async { self.inner.receipts().await });
        receipts
            .into_iter()
            .map(|r| PyReceipt { inner: r })
            .collect()
    }

    /// Get the total amount spent in satoshis.
    fn total_spent(&self) -> u64 {
        let rt = get_runtime();
        rt.block_on(async { self.inner.total_spent().await })
    }

    #[allow(clippy::unused_self)]
    fn __repr__(&self) -> String {
        "L402Client(...)".to_string()
    }
}

/// Build an `L402Client` from any `LnBackend` implementation.
fn build_client<B: LnBackend + 'static>(
    backend: B,
    budget: Option<PyBudget>,
    max_fee_sats: u64,
) -> PyResult<PyL402Client> {
    let budget = budget.map_or_else(RustBudget::unlimited, |b| b.inner);

    let config = L402ClientConfig {
        max_fee_sats,
        max_retries: 1,
        user_agent: format!("bolt402-python/{}", env!("CARGO_PKG_VERSION")),
    };

    let client = RustClient::builder()
        .ln_backend(backend)
        .token_store(InMemoryTokenStore::default())
        .budget(budget)
        .config(config)
        .build()
        .map_err(|e| PyRuntimeError::new_err(format!("failed to build L402Client: {e}")))?;

    Ok(PyL402Client { inner: client })
}

/// Convert a Rust `L402Response` result to a Python `PyL402Response`.
fn convert_response(
    rt: &tokio::runtime::Runtime,
    result: Result<bolt402_core::L402Response, bolt402_proto::ClientError>,
) -> PyResult<PyL402Response> {
    match result {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let paid = resp.paid();
            let cached_token = resp.cached_token();
            let receipt = resp.receipt().map(|r| PyReceipt { inner: r.clone() });
            let headers: HashMap<String, String> = resp
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();
            let body = rt.block_on(async { resp.text().await }).unwrap_or_default();

            Ok(PyL402Response {
                status,
                paid,
                cached_token,
                receipt,
                body,
                headers,
            })
        }
        Err(e) => Err(map_client_error(&e)),
    }
}

// ---------------------------------------------------------------------------
// Error mapping
// ---------------------------------------------------------------------------

/// Map Rust `ClientError` to Python exceptions.
fn map_client_error(err: &bolt402_proto::ClientError) -> PyErr {
    match err {
        bolt402_proto::ClientError::BudgetExceeded { .. } => {
            PyValueError::new_err(format!("BudgetExceeded: {err}"))
        }
        bolt402_proto::ClientError::PaymentFailed { .. } => {
            PyRuntimeError::new_err(format!("PaymentFailed: {err}"))
        }
        bolt402_proto::ClientError::MissingChallenge => {
            PyRuntimeError::new_err(format!("MissingChallenge: {err}"))
        }
        _ => PyRuntimeError::new_err(err.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Module definition
// ---------------------------------------------------------------------------

/// bolt402: L402 client SDK for AI agent frameworks.
///
/// Pay for APIs with Lightning. Built in Rust, available in Python.
#[pymodule]
fn _bolt402(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBudget>()?;
    m.add_class::<PyReceipt>()?;
    m.add_class::<PyPaymentResult>()?;
    m.add_class::<PyNodeInfo>()?;
    m.add_class::<PyLndGrpcBackend>()?;
    m.add_class::<PyLndRestBackend>()?;
    m.add_class::<PyClnGrpcBackend>()?;
    m.add_class::<PyClnRestBackend>()?;
    m.add_class::<PySwissKnifeBackend>()?;
    m.add_class::<PyL402Response>()?;
    m.add_class::<PyL402Client>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_unlimited() {
        let budget = PyBudget::unlimited();
        assert!(budget.inner.per_request_max.is_none());
        assert!(budget.inner.hourly_max.is_none());
        assert!(budget.inner.daily_max.is_none());
        assert!(budget.inner.total_max.is_none());
    }

    #[test]
    fn budget_with_limits() {
        let budget = PyBudget::new(Some(100), Some(1000), Some(5000), Some(50000), None);
        assert_eq!(budget.inner.per_request_max, Some(100));
        assert_eq!(budget.inner.hourly_max, Some(1000));
        assert_eq!(budget.inner.daily_max, Some(5000));
        assert_eq!(budget.inner.total_max, Some(50000));
    }

    #[test]
    fn budget_repr() {
        let budget = PyBudget::new(Some(100), None, None, Some(50000), None);
        let repr = budget.__repr__();
        assert!(repr.contains("per_request_max=100"));
        assert!(repr.contains("total_max=50000"));
        assert!(repr.contains("hourly_max=None"));
    }

    #[test]
    fn receipt_total_cost() {
        let receipt = PyReceipt {
            inner: RustReceipt::new(
                "https://api.example.com".to_string(),
                100,
                5,
                "hash".to_string(),
                "preimage".to_string(),
                200,
                450,
            ),
        };
        assert_eq!(receipt.total_cost_sats(), 105);
        assert_eq!(receipt.amount_sats(), 100);
        assert_eq!(receipt.fee_sats(), 5);
        assert_eq!(receipt.response_status(), 200);
    }

    #[test]
    fn receipt_json() {
        let receipt = PyReceipt {
            inner: RustReceipt::new(
                "https://api.example.com".to_string(),
                100,
                5,
                "abc123".to_string(),
                "def456".to_string(),
                200,
                450,
            ),
        };
        let json = receipt.to_json().unwrap();
        assert!(json.contains("\"amount_sats\": 100"));
        assert!(json.contains("\"endpoint\": \"https://api.example.com\""));
    }

    #[test]
    fn fmt_opt_helper() {
        assert_eq!(fmt_opt(Some(42)), "42");
        assert_eq!(fmt_opt(None), "None");
    }

    #[test]
    fn lnd_backend_constructor_valid() {
        let backend = PyLndRestBackend::new("https://localhost:8080", "deadbeef");
        assert!(backend.is_ok());
    }

    #[test]
    fn cln_backend_constructor_valid() {
        let backend = PyClnRestBackend::new("https://localhost:3001", "test_rune");
        assert!(backend.is_ok());
    }

    #[test]
    fn swissknife_backend_constructor() {
        let backend = PySwissKnifeBackend::new("https://api.numeraire.tech", "sk-test");
        assert_eq!(backend.__repr__(), "SwissKnifeBackend(...)");
    }

    #[test]
    fn payment_result_total_cost() {
        let result = PyPaymentResult {
            inner: PaymentResult {
                preimage: "abc".to_string(),
                payment_hash: "def".to_string(),
                amount_sats: 100,
                fee_sats: 5,
            },
        };
        assert_eq!(result.total_cost_sats(), 105);
        assert_eq!(result.amount_sats(), 100);
        assert_eq!(result.fee_sats(), 5);
    }

    #[test]
    fn node_info_properties() {
        let info = PyNodeInfo {
            inner: NodeInfo {
                pubkey: "02abc".to_string(),
                alias: "mynode".to_string(),
                num_active_channels: 5,
            },
        };
        assert_eq!(info.pubkey(), "02abc");
        assert_eq!(info.alias(), "mynode");
        assert_eq!(info.num_active_channels(), 5);
        assert!(info.__repr__().contains("mynode"));
    }

    #[test]
    fn client_with_lnd_rest_constructor() {
        let result = PyL402Client::with_lnd_rest("https://localhost:8080", "deadbeef", None, 100);
        assert!(result.is_ok());
    }

    #[test]
    fn client_with_cln_rest_constructor() {
        let result = PyL402Client::with_cln_rest("https://localhost:3001", "test_rune", None, 100);
        assert!(result.is_ok());
    }

    #[test]
    fn client_with_swissknife_constructor() {
        let result =
            PyL402Client::with_swissknife("https://api.numeraire.tech", "sk-test", None, 100);
        assert!(result.is_ok());
    }

    #[test]
    fn client_with_budget() {
        let budget = PyBudget::new(Some(100), None, None, Some(10000), None);
        let result =
            PyL402Client::with_lnd_rest("https://localhost:8080", "deadbeef", Some(budget), 50);
        assert!(result.is_ok());
    }
}
