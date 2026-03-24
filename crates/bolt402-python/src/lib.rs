//! Python bindings for the bolt402 L402 client SDK.
//!
//! Exposes the Rust core to Python via `PyO3`, enabling Python AI agent
//! frameworks (`LangChain`, `CrewAI`, `AutoGen`, `LlamaIndex`) to use
//! L402-gated APIs with Lightning payments.

use std::collections::HashMap;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

use bolt402_core::budget::Budget as RustBudget;
use bolt402_core::cache::InMemoryTokenStore;
use bolt402_core::receipt::Receipt as RustReceipt;
use bolt402_core::{L402Client as RustClient, L402ClientConfig};

/// Runtime handle shared across Python bindings.
///
/// We create one tokio runtime and reuse it for all async operations,
/// running them from Python synchronous context via `block_on`.
fn get_runtime() -> &'static tokio::runtime::Runtime {
    use std::sync::OnceLock;
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
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
        format!("L402Response(status={}, paid={})", self.status, self.paid,)
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
#[pyclass(name = "L402Client")]
struct PyL402Client {
    inner: RustClient,
}

#[pymethods]
impl PyL402Client {
    /// Create a new `L402Client`.
    #[new]
    #[pyo3(signature = (*, backend="mock", budget=None, max_fee_sats=100, mock_server_url=None))]
    fn new(
        backend: &str,
        budget: Option<PyBudget>,
        max_fee_sats: u64,
        mock_server_url: Option<&str>,
    ) -> PyResult<Self> {
        let _budget = budget.unwrap_or_else(|| PyBudget {
            inner: RustBudget::unlimited(),
        });

        let _config = L402ClientConfig {
            max_fee_sats,
            max_retries: 1,
            user_agent: format!("bolt402-python/{}", env!("CARGO_PKG_VERSION")),
        };

        match backend {
            "mock" => {
                let _url = mock_server_url.ok_or_else(|| {
                    PyValueError::new_err(
                        "mock_server_url is required when backend='mock'. \
                         Use create_mock_client() for a connected pair instead.",
                    )
                })?;

                // The mock backend needs direct access to the server's challenge
                // registry. A standalone URL connection is not possible.
                // Users should use create_mock_client() instead.
                Err(PyValueError::new_err(
                    "use create_mock_client() for a properly connected mock setup. \
                     Direct L402Client(backend='mock') is not supported because the \
                     mock backend requires a shared challenge registry with the server.",
                ))
            }
            other => Err(PyValueError::new_err(format!(
                "unsupported backend: '{other}'. Available: 'mock' (via create_mock_client()). \
                 LND and SwissKnife backends coming soon."
            ))),
        }
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
    /// See `get()` for the full L402 flow description.
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

/// Convert a Rust `L402Response` result to a Python `PyL402Response`.
fn convert_response(
    rt: &tokio::runtime::Runtime,
    result: Result<bolt402_core::L402Response, bolt402_proto::ClientError>,
) -> PyResult<PyL402Response> {
    match result {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let paid = resp.paid();
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
                receipt,
                body,
                headers,
            })
        }
        Err(e) => Err(map_client_error(&e)),
    }
}

// ---------------------------------------------------------------------------
// MockL402Server
// ---------------------------------------------------------------------------

/// Mock L402 server for testing and development.
///
/// Starts a local HTTP server that responds with L402 challenges for
/// configured endpoints. Use with `create_mock_client()` for testing
/// without real Lightning infrastructure.
#[pyclass(name = "MockL402Server")]
struct PyMockL402Server {
    url: String,
    // The server and backend live on the tokio runtime; we hold Arcs
    // to keep them alive as long as the Python object exists.
    _server: std::sync::Arc<bolt402_mock::MockL402Server>,
    _backend: std::sync::Arc<bolt402_mock::MockLnBackend>,
}

#[pymethods]
impl PyMockL402Server {
    /// Create and start a mock L402 server.
    #[new]
    #[pyo3(signature = (endpoints))]
    #[allow(clippy::needless_pass_by_value)] // PyO3 requires owned HashMap
    fn new(endpoints: HashMap<String, u64>) -> PyResult<Self> {
        let rt = get_runtime();

        let result = rt.block_on(async {
            let mut builder = bolt402_mock::MockL402Server::builder();
            for (path, price) in &endpoints {
                builder = builder.endpoint(path, bolt402_mock::EndpointConfig::new(*price));
            }
            builder.build().await
        });

        match result {
            Ok(server) => {
                let url = server.url();
                let backend = server.mock_backend();
                Ok(Self {
                    url,
                    _server: std::sync::Arc::new(server),
                    _backend: std::sync::Arc::new(backend),
                })
            }
            Err(e) => Err(PyRuntimeError::new_err(format!(
                "failed to start mock server: {e}"
            ))),
        }
    }

    /// Base URL of the running mock server.
    #[getter]
    fn url(&self) -> &str {
        &self.url
    }

    fn __repr__(&self) -> String {
        format!("MockL402Server(url='{}')", self.url)
    }
}

// ---------------------------------------------------------------------------
// create_mock_client
// ---------------------------------------------------------------------------

/// Create a mock L402 client and server pair for testing.
///
/// Returns a tuple of (`L402Client`, `MockL402Server`). The client is
/// pre-configured with the mock Lightning backend connected to the
/// server's challenge registry.
#[pyfunction]
#[pyo3(signature = (endpoints, budget=None, max_fee_sats=100))]
#[allow(clippy::needless_pass_by_value)] // PyO3 requires owned HashMap
fn create_mock_client(
    endpoints: HashMap<String, u64>,
    budget: Option<PyBudget>,
    max_fee_sats: u64,
) -> PyResult<(PyL402Client, PyMockL402Server)> {
    let rt = get_runtime();
    let budget = budget.map_or_else(RustBudget::unlimited, |b| b.inner);

    let config = L402ClientConfig {
        max_fee_sats,
        max_retries: 1,
        user_agent: format!("bolt402-python/{}", env!("CARGO_PKG_VERSION")),
    };

    let result: Result<
        (
            RustClient,
            String,
            std::sync::Arc<bolt402_mock::MockL402Server>,
            std::sync::Arc<bolt402_mock::MockLnBackend>,
        ),
        String,
    > = rt.block_on(async {
        let mut builder = bolt402_mock::MockL402Server::builder();
        for (path, price) in &endpoints {
            builder = builder.endpoint(path, bolt402_mock::EndpointConfig::new(*price));
        }
        let server = builder
            .build()
            .await
            .map_err(|e| format!("failed to start server: {e}"))?;

        let url = server.url();
        let mock_backend = server.mock_backend();
        let token_store = InMemoryTokenStore::default();

        let client = RustClient::builder()
            .ln_backend(mock_backend.clone())
            .token_store(token_store)
            .budget(budget)
            .config(config)
            .build()
            .map_err(|e| format!("failed to build client: {e}"))?;

        Ok((
            client,
            url,
            std::sync::Arc::new(server),
            std::sync::Arc::new(mock_backend),
        ))
    });

    match result {
        Ok((client, url, server_arc, backend_arc)) => Ok((
            PyL402Client { inner: client },
            PyMockL402Server {
                url,
                _server: server_arc,
                _backend: backend_arc,
            },
        )),
        Err(e) => Err(PyRuntimeError::new_err(e)),
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
    m.add_class::<PyL402Response>()?;
    m.add_class::<PyL402Client>()?;
    m.add_class::<PyMockL402Server>()?;
    m.add_function(wrap_pyfunction!(create_mock_client, m)?)?;
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
}
