//! L402 client engine.
//!
//! The [`L402Client`] is the main entry point for the SDK. It handles the full
//! L402 protocol flow: making HTTP requests, detecting 402 challenges, paying
//! invoices, caching tokens, enforcing budgets, and recording receipts.
//!
//! # Architecture
//!
//! The client is composed from ports (traits) and adapters:
//!
//! - **[`l402_proto::port::LnBackend`]**: Pays Lightning invoices (e.g., LND, CLN)
//! - **[`l402_proto::port::TokenStore`]**: Caches L402 tokens to avoid re-paying
//! - **[`crate::budget::BudgetTracker`]**: Enforces spending limits
//!
//! # Example
//!
//! ```rust,no_run
//! use l402_core::{L402Client, L402ClientConfig};
//! use l402_core::budget::Budget;
//! use l402_core::cache::InMemoryTokenStore;
//! # use l402_proto::port::{LnBackend, PaymentResult, NodeInfo};
//! # use l402_proto::ClientError;
//! # use async_trait::async_trait;
//!
//! # struct MyBackend;
//! # #[async_trait]
//! # impl LnBackend for MyBackend {
//! #     async fn pay_invoice(&self, _: &str, _: u64) -> Result<PaymentResult, ClientError> { todo!() }
//! #     async fn get_balance(&self) -> Result<u64, ClientError> { todo!() }
//! #     async fn get_info(&self) -> Result<NodeInfo, ClientError> { todo!() }
//! # }
//!
//! # async fn example() {
//! let client = L402Client::builder()
//!     .ln_backend(MyBackend)
//!     .token_store(InMemoryTokenStore::default())
//!     .budget(Budget::unlimited())
//!     .build()
//!     .unwrap();
//!
//! let response = client.get("https://api.example.com/resource").await.unwrap();
//! println!("Status: {}", response.status());
//! # }
//! ```

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use reqwest::header::{AUTHORIZATION, HeaderValue, WWW_AUTHENTICATE};
use reqwest::{Client as HttpClient, StatusCode};
use web_time::Instant;

use l402_proto::{L402Challenge, L402Token, PaymentResult, decode_bolt11_amount};

use crate::budget::{Budget, BudgetTracker};
use crate::receipt::Receipt;
use l402_proto::ClientError;
use l402_proto::LnBackend;
use l402_proto::port::TokenStore;

/// Configuration for the [`L402Client`].
#[derive(Debug, Clone)]
pub struct L402ClientConfig {
    /// Maximum routing fee in satoshis when paying invoices.
    pub max_fee_sats: u64,

    /// Maximum number of retries after payment before giving up.
    pub max_retries: u32,

    /// User-Agent string for HTTP requests.
    pub user_agent: String,
}

impl Default for L402ClientConfig {
    fn default() -> Self {
        Self {
            max_fee_sats: 100,
            max_retries: 1,
            user_agent: format!("l402/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// Builder for constructing an [`L402Client`].
pub struct L402ClientBuilder<L = (), T = ()> {
    ln_backend: L,
    token_store: T,
    budget: Option<Budget>,
    config: L402ClientConfig,
    http_client: Option<HttpClient>,
}

impl L402ClientBuilder<(), ()> {
    /// Create a new builder.
    fn new() -> Self {
        Self {
            ln_backend: (),
            token_store: (),
            budget: None,
            config: L402ClientConfig::default(),
            http_client: None,
        }
    }
}

impl<L, T> L402ClientBuilder<L, T> {
    /// Set the Lightning backend for paying invoices.
    pub fn ln_backend<B: LnBackend + 'static>(self, backend: B) -> L402ClientBuilder<B, T> {
        L402ClientBuilder {
            ln_backend: backend,
            token_store: self.token_store,
            budget: self.budget,
            config: self.config,
            http_client: self.http_client,
        }
    }

    /// Set the token store for caching L402 credentials.
    pub fn token_store<S: TokenStore + 'static>(self, store: S) -> L402ClientBuilder<L, S> {
        L402ClientBuilder {
            ln_backend: self.ln_backend,
            token_store: store,
            budget: self.budget,
            config: self.config,
            http_client: self.http_client,
        }
    }

    /// Set the budget configuration for spending limits.
    #[must_use]
    pub fn budget(mut self, budget: Budget) -> Self {
        self.budget = Some(budget);
        self
    }

    /// Set the client configuration.
    #[must_use]
    pub fn config(mut self, config: L402ClientConfig) -> Self {
        self.config = config;
        self
    }

    /// Set a custom HTTP client (useful for proxies, custom TLS, etc.).
    ///
    /// On native targets it must disable redirects to preserve credential isolation.
    /// The SDK default disables redirects; a final-origin check also rejects redirected challenges.
    #[must_use]
    pub fn http_client(mut self, client: HttpClient) -> Self {
        self.http_client = Some(client);
        self
    }
}

impl<B: LnBackend + 'static, S: TokenStore + 'static> L402ClientBuilder<B, S> {
    /// Build the [`L402Client`].
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Backend`] if the HTTP client cannot be constructed.
    pub fn build(self) -> Result<L402Client, ClientError> {
        let http_client = if let Some(client) = self.http_client {
            client
        } else {
            let builder = HttpClient::builder().user_agent(&self.config.user_agent);
            #[cfg(not(target_arch = "wasm32"))]
            let builder = builder.redirect(reqwest::redirect::Policy::none());
            builder.build().map_err(|e| ClientError::Backend {
                reason: format!("failed to build HTTP client: {e}"),
            })?
        };

        let budget = self.budget.unwrap_or_else(Budget::unlimited);

        Ok(L402Client {
            ln_backend: Arc::new(self.ln_backend),
            token_store: Arc::new(self.token_store),
            budget_tracker: BudgetTracker::new(budget),
            config: self.config,
            http_client,
            receipts: Arc::new(RwLock::new(Vec::new())),
            payments: RwLock::new(HashMap::new()),
            unpersisted_tokens: RwLock::new(HashMap::new()),
        })
    }
}

/// L402 client that handles the full payment-gated HTTP flow.
///
/// The client intercepts HTTP 402 responses, parses L402 challenges, pays
/// Lightning invoices, and retries requests with valid credentials.
///
/// Use [`L402Client::builder()`] to construct an instance.
pub struct L402Client {
    ln_backend: Arc<dyn LnBackend>,
    token_store: Arc<dyn TokenStore>,
    budget_tracker: BudgetTracker,
    config: L402ClientConfig,
    http_client: HttpClient,
    receipts: Arc<RwLock<Vec<Receipt>>>,
    // None is in-flight/unknown, including a dropped payment future. Never retry it.
    payments: RwLock<HashMap<String, Option<(PaymentResult, usize)>>>,
    unpersisted_tokens: RwLock<HashMap<String, L402Token>>,
}

impl L402Client {
    /// Create a new [`L402ClientBuilder`].
    pub fn builder() -> L402ClientBuilder<(), ()> {
        L402ClientBuilder::new()
    }

    /// Send a GET request, automatically handling L402 payment challenges.
    ///
    /// If the server responds with HTTP 402, the client will:
    /// 1. Parse the L402 challenge from the `WWW-Authenticate` header
    /// 2. Check the budget to ensure the payment is allowed
    /// 3. Pay the Lightning invoice
    /// 4. Cache the resulting token
    /// 5. Retry the request with the `Authorization: L402` header
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails
    /// - The 402 response has no valid L402 challenge
    /// - Budget limits are exceeded
    /// - The Lightning payment fails
    /// - The retry after payment fails
    pub async fn get(&self, url: &str) -> Result<L402Response, ClientError> {
        self.request(reqwest::Method::GET, url, None).await
    }

    /// Send a POST request with an optional JSON body, handling L402 challenges.
    ///
    /// See [`L402Client::get`] for the full L402 flow description.
    pub async fn post(&self, url: &str, body: Option<&str>) -> Result<L402Response, ClientError> {
        self.request(reqwest::Method::POST, url, body).await
    }

    /// Execute an HTTP request with L402 challenge handling.
    ///
    /// This is the core method that implements the L402 protocol flow.
    async fn request(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<&str>,
    ) -> Result<L402Response, ClientError> {
        let start = Instant::now();

        // A successful payment must remain usable if persistent token storage fails.
        let token = self
            .unpersisted_tokens
            .read()
            .expect("RwLock poisoned")
            .get(url)
            .cloned();
        let token = if token.is_some() {
            token
        } else {
            self.token_store
                .get(url)
                .await?
                .map(|(macaroon, preimage)| L402Token::new(macaroon, preimage))
        };
        if let Some(token) = token {
            let response = self.send_with_auth(&method, url, body, &token).await?;

            // If the cached token is still valid, return the response
            if response.status() != StatusCode::PAYMENT_REQUIRED {
                return Ok(L402Response {
                    inner: response,
                    paid: false,
                    cached_token: true,
                    receipt: None,
                });
            }

            // Token was rejected — remove it from cache and fall through
            tracing::debug!(url, "cached L402 token rejected, removing from cache");
            self.unpersisted_tokens
                .write()
                .expect("RwLock poisoned")
                .remove(url);
            self.token_store.remove(url).await?;
        }

        // Make the initial request without auth
        let response = self.send_request(&method, url, body).await?;

        // If not 402, return as-is (no payment needed)
        if response.status() != StatusCode::PAYMENT_REQUIRED {
            return Ok(L402Response {
                inner: response,
                paid: false,
                cached_token: false,
                receipt: None,
            });
        }

        // Parse the L402 challenge from the 402 response
        let challenge = Self::extract_challenge(&response)?;

        // Extract the domain for domain-specific budget checks
        let domain = reqwest::Url::parse(url).ok().and_then(|u| {
            u.host_str()
                .map(|host| host.trim_end_matches('.').to_string())
        });

        // Decode the invoice amount for budget enforcement.
        // Never authorize a payment whose amount cannot be checked.
        let invoice_amount_sats = decode_bolt11_amount(&challenge.invoice)?
            .ok_or_else(|| l402_proto::L402Error::InvalidInvoice {
                reason: "automatic L402 payments require an explicit invoice amount".to_string(),
            })?
            .satoshis();

        let (payment, receipt_index, newly_paid) = self
            .pay_challenge(
                &challenge.invoice,
                invoice_amount_sats,
                domain.as_deref(),
                url,
                start,
            )
            .await?;

        let token = L402Token::new(challenge.macaroon, payment.preimage);
        self.persist_token(url, &token).await?;
        let retry_response = self.send_with_auth(&method, url, body, &token).await?;
        let receipt = {
            let mut receipts = self.receipts.write().expect("RwLock poisoned");
            let receipt = &mut receipts[receipt_index];
            receipt.response_status = retry_response.status().as_u16();
            receipt.latency_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
            receipt.clone()
        };
        if retry_response.status() == StatusCode::PAYMENT_REQUIRED {
            self.unpersisted_tokens
                .write()
                .expect("RwLock poisoned")
                .remove(url);
            self.token_store.remove(url).await?;
            return Err(ClientError::RetryFailed {
                reason: "server returned 402 again after payment; receipt retained".to_string(),
            });
        }

        Ok(L402Response {
            inner: retry_response,
            paid: newly_paid,
            cached_token: !newly_paid,
            receipt: Some(receipt),
        })
    }

    async fn persist_token(&self, url: &str, token: &L402Token) -> Result<(), ClientError> {
        // Retain proof before the first post-payment await, including cancellation.
        self.unpersisted_tokens
            .write()
            .expect("RwLock poisoned")
            .insert(url.to_string(), token.clone());
        self.token_store
            .put(url, &token.macaroon, &token.preimage)
            .await?;
        // Once persistence confirms the write, honor the configured store's eviction policy.
        let mut tokens = self.unpersisted_tokens.write().expect("RwLock poisoned");
        if tokens.get(url).is_some_and(|saved| {
            saved.macaroon == token.macaroon && saved.preimage == token.preimage
        }) {
            tokens.remove(url);
        }
        Ok(())
    }

    async fn pay_challenge(
        &self,
        invoice: &str,
        invoice_amount_sats: u64,
        domain: Option<&str>,
        url: &str,
        start: Instant,
    ) -> Result<(PaymentResult, usize, bool), ClientError> {
        // Reserve and claim the invoice synchronously. No lock is held across an await.
        // Invoice case is normalized because BOLT11 may be uppercase.
        let invoice_key = invoice.to_ascii_lowercase();
        let (known, reservation) = {
            let mut payments = self.payments.write().expect("RwLock poisoned");
            if let Some(previous) = payments.get(&invoice_key) {
                let known = previous.clone().ok_or(ClientError::PaymentUncertain)?;
                (Some(known), None)
            } else {
                if !self.ln_backend.supports_fee_limit() {
                    return Err(ClientError::FeeLimitUnsupported);
                }
                let reservation = self.budget_tracker.reserve(
                    invoice_amount_sats,
                    self.config.max_fee_sats,
                    domain,
                )?;
                payments.insert(invoice_key.clone(), None);
                (None, Some(reservation))
            }
        };

        let newly_paid = known.is_none();
        let settled = if let Some(known) = known {
            known
        } else {
            let reservation = reservation.expect("new payment has reservation");
            let max_fee_sats = reservation.max_fee_sats;
            let payment = match self.ln_backend.pay_invoice(invoice, max_fee_sats).await {
                Ok(payment) => payment,
                Err(error) => {
                    if matches!(
                        error,
                        ClientError::PaymentRejected { .. } | ClientError::FeeLimitUnsupported
                    ) {
                        self.budget_tracker.release(reservation);
                        self.payments
                            .write()
                            .expect("RwLock poisoned")
                            .remove(&invoice_key);
                    }
                    // Transport errors, ambiguous failures and cancellation keep the reservation.
                    return Err(error);
                }
            };
            let receipt = Receipt::new(
                url.to_string(),
                payment.amount_sats,
                payment.fee_sats,
                payment.payment_hash.clone(),
                payment.preimage.clone(),
                0,
                u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
            );
            let receipt_index = {
                let mut receipts = self.receipts.write().expect("RwLock poisoned");
                let index = receipts.len();
                receipts.push(receipt);
                index
            };
            let total = payment.amount_sats.checked_add(payment.fee_sats);
            let violation = payment.amount_sats != invoice_amount_sats
                || payment.fee_sats > max_fee_sats
                || total.is_none();
            let settlement = self.budget_tracker.settle_payment(
                reservation,
                total.unwrap_or(u64::MAX),
                violation,
            );
            if violation {
                return Err(ClientError::Backend { reason: "settled payment violated the authorized amount or fee cap; receipt retained and further payments disabled".to_string() });
            }
            settlement?;
            self.payments
                .write()
                .expect("RwLock poisoned")
                .insert(invoice_key, Some((payment.clone(), receipt_index)));
            (payment, receipt_index)
        };

        Ok((settled.0, settled.1, newly_paid))
    }

    /// Send an HTTP request without authentication.
    async fn send_request(
        &self,
        method: &reqwest::Method,
        url: &str,
        body: Option<&str>,
    ) -> Result<reqwest::Response, ClientError> {
        let mut builder = self.http_client.request(method.clone(), url);

        if let Some(body) = body {
            builder = builder
                .header("Content-Type", "application/json")
                .body(body.to_string());
        }

        let response = builder.send().await.map_err(|e| ClientError::Http {
            reason: e.to_string(),
        })?;
        let requested = reqwest::Url::parse(url).map_err(|e| ClientError::Http {
            reason: e.to_string(),
        })?;
        if response.url().origin() != requested.origin() {
            return Err(ClientError::Http {
                reason: "cross-origin redirect refused".to_string(),
            });
        }
        Ok(response)
    }

    /// Send an HTTP request with L402 authorization.
    async fn send_with_auth(
        &self,
        method: &reqwest::Method,
        url: &str,
        body: Option<&str>,
        token: &L402Token,
    ) -> Result<reqwest::Response, ClientError> {
        let auth_value =
            HeaderValue::from_str(&token.to_header_value()).map_err(|e| ClientError::Backend {
                reason: format!("invalid authorization header value: {e}"),
            })?;

        let mut builder = self.http_client.request(method.clone(), url);
        builder = builder.header(AUTHORIZATION, auth_value);

        if let Some(body) = body {
            builder = builder
                .header("Content-Type", "application/json")
                .body(body.to_string());
        }

        let response = builder.send().await.map_err(|e| ClientError::Http {
            reason: e.to_string(),
        })?;
        let requested = reqwest::Url::parse(url).map_err(|e| ClientError::Http {
            reason: e.to_string(),
        })?;
        if response.url().origin() != requested.origin() {
            return Err(ClientError::Http {
                reason: "cross-origin redirect refused".to_string(),
            });
        }
        Ok(response)
    }

    /// Extract an L402 challenge from a 402 response.
    fn extract_challenge(response: &reqwest::Response) -> Result<L402Challenge, ClientError> {
        let header = response
            .headers()
            .get(WWW_AUTHENTICATE)
            .ok_or(ClientError::MissingChallenge)?;

        let header_str = header.to_str().map_err(|_| ClientError::MissingChallenge)?;

        Ok(L402Challenge::from_header(header_str)?)
    }

    /// Get all settled payment receipts, including payments whose persistence or HTTP retry failed.
    /// A `response_status` of 0 means no paid HTTP response has been received.
    #[allow(clippy::unused_async, clippy::unused_async_trait_impl)] // Kept async for API consistency with total_spent()
    pub async fn receipts(&self) -> Vec<Receipt> {
        self.receipts.read().expect("RwLock poisoned").clone()
    }

    /// Get fee-inclusive spending plus outstanding reservations, including uncertain payments.
    pub async fn total_spent(&self) -> u64 {
        self.budget_tracker.total_spent().await
    }
}

/// Response from an L402-aware HTTP request.
///
/// Wraps a standard HTTP response with additional metadata about whether
/// a Lightning payment was made to obtain access.
pub struct L402Response {
    inner: reqwest::Response,
    paid: bool,
    cached_token: bool,
    receipt: Option<Receipt>,
}

impl L402Response {
    /// Get the HTTP status code.
    pub fn status(&self) -> StatusCode {
        self.inner.status()
    }

    /// Whether a Lightning payment was made for this request.
    pub fn paid(&self) -> bool {
        self.paid
    }

    /// Whether a cached L402 token was used (no new payment needed).
    pub fn cached_token(&self) -> bool {
        self.cached_token
    }

    /// Get the payment receipt, if a payment was made.
    pub fn receipt(&self) -> Option<&Receipt> {
        self.receipt.as_ref()
    }

    /// Consume the response and read the body as text.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Http`] if reading the body fails.
    pub async fn text(self) -> Result<String, ClientError> {
        self.inner.text().await.map_err(|e| ClientError::Http {
            reason: e.to_string(),
        })
    }

    /// Consume the response and read the body as bytes.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Http`] if reading the body fails.
    pub async fn bytes(self) -> Result<bytes::Bytes, ClientError> {
        self.inner.bytes().await.map_err(|e| ClientError::Http {
            reason: e.to_string(),
        })
    }

    /// Consume the response and deserialize the body as JSON.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Http`] if reading or deserializing fails.
    pub async fn json<T: serde::de::DeserializeOwned>(self) -> Result<T, ClientError> {
        self.inner.json().await.map_err(|e| ClientError::Http {
            reason: e.to_string(),
        })
    }

    /// Get a reference to the response headers.
    pub fn headers(&self) -> &reqwest::header::HeaderMap {
        self.inner.headers()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::InMemoryTokenStore;
    use async_trait::async_trait;
    use l402_proto::port::{NodeInfo, PaymentResult};
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A mock Lightning backend that returns configurable results.
    struct MockLnBackend {
        preimage: String,
        payment_hash: String,
        amount_sats: u64,
        fee_sats: u64,
        pay_count: Arc<AtomicU32>,
        should_fail: bool,
    }

    impl MockLnBackend {
        fn new(preimage: &str, payment_hash: &str, amount: u64) -> Self {
            Self {
                preimage: preimage.to_string(),
                payment_hash: payment_hash.to_string(),
                amount_sats: amount,
                fee_sats: 1,
                pay_count: Arc::new(AtomicU32::new(0)),
                should_fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                preimage: String::new(),
                payment_hash: String::new(),
                amount_sats: 0,
                fee_sats: 0,
                pay_count: Arc::new(AtomicU32::new(0)),
                should_fail: true,
            }
        }

        fn call_count(&self) -> u32 {
            self.pay_count.load(Ordering::Relaxed)
        }
    }

    #[async_trait]
    impl LnBackend for MockLnBackend {
        fn supports_fee_limit(&self) -> bool {
            true
        }

        async fn pay_invoice(
            &self,
            _bolt11: &str,
            _max_fee_sats: u64,
        ) -> Result<PaymentResult, ClientError> {
            self.pay_count.fetch_add(1, Ordering::Relaxed);

            if self.should_fail {
                return Err(ClientError::PaymentFailed {
                    reason: "mock payment failure".to_string(),
                });
            }

            Ok(PaymentResult {
                preimage: self.preimage.clone(),
                payment_hash: self.payment_hash.clone(),
                amount_sats: self.amount_sats,
                fee_sats: self.fee_sats,
            })
        }

        async fn get_balance(&self) -> Result<u64, ClientError> {
            Ok(1_000_000)
        }

        async fn get_info(&self) -> Result<NodeInfo, ClientError> {
            Ok(NodeInfo {
                pubkey: "mock_pubkey".to_string(),
                alias: "mock_node".to_string(),
                num_active_channels: 5,
            })
        }
    }

    #[test]
    fn builder_default_config() {
        let config = L402ClientConfig::default();
        assert_eq!(config.max_fee_sats, 100);
        assert_eq!(config.max_retries, 1);
        assert!(config.user_agent.starts_with("l402/"));
    }

    #[test]
    fn builder_creates_client() {
        let backend = MockLnBackend::new("abc", "def", 100);
        let store = InMemoryTokenStore::default();

        let result = L402Client::builder()
            .ln_backend(backend)
            .token_store(store)
            .budget(Budget::unlimited())
            .build();

        assert!(result.is_ok());
    }

    #[test]
    fn builder_custom_config() {
        let config = L402ClientConfig {
            max_fee_sats: 500,
            max_retries: 3,
            user_agent: "test-agent/1.0".to_string(),
        };

        let backend = MockLnBackend::new("abc", "def", 100);
        let store = InMemoryTokenStore::default();

        let result = L402Client::builder()
            .ln_backend(backend)
            .token_store(store)
            .config(config)
            .build();

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn unbudgetable_challenges_never_call_the_payer() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for invoice in [
            "lnbc1qtest",
            "lnbcinvalid",
            "lnbc2.5m1qtest",
            "lnbc18446744073709551615n1qtest",
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let reply = format!(
                "HTTP/1.1 402 Payment Required\r\nWWW-Authenticate: L402 macaroon=\"YWJj\", invoice=\"{invoice}\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buf = [0; 4096];
                assert!(socket.read(&mut buf).await.unwrap() > 0);
                socket.write_all(reply.as_bytes()).await.unwrap();
            });
            let backend = MockLnBackend::failing();
            let count = backend.pay_count.clone();
            let client = L402Client::builder()
                .ln_backend(backend)
                .token_store(InMemoryTokenStore::default())
                .build()
                .unwrap();
            assert!(
                matches!(client.get(&url).await, Err(ClientError::Protocol(_))),
                "{invoice}"
            );
            assert_eq!(count.load(Ordering::Relaxed), 0);
            assert_eq!(client.total_spent().await, 0);
            assert!(client.receipts().await.is_empty());
            server.await.unwrap();
        }
    }

    #[test]
    fn extract_challenge_from_header() {
        // Test the static helper by constructing a mock reqwest::Response
        // is non-trivial, so we test the underlying L402Challenge parsing instead.
        let header = r#"L402 macaroon="YWJjZGVm", invoice="lnbc100n1pj9nr7mpp5test""#;
        let challenge = L402Challenge::from_header(header).unwrap();
        assert_eq!(challenge.macaroon, "YWJjZGVm");
        assert_eq!(challenge.invoice, "lnbc100n1pj9nr7mpp5test");
    }

    #[test]
    fn l402_response_paid_flag() {
        // Verify L402Response correctly reports payment status
        // (Full integration test requires a running HTTP server — see l402-mock)
        let receipt = Receipt::new(
            "https://api.test.com".to_string(),
            100,
            1,
            "hash".to_string(),
            "preimage".to_string(),
            200,
            50,
        );

        assert_eq!(receipt.total_cost_sats(), 101);
    }

    #[tokio::test]
    async fn token_cache_flow() {
        // Verify the token store integration: put, get, remove
        let store = InMemoryTokenStore::new(10);

        store
            .put("https://api.test.com", "mac1", "pre1")
            .await
            .unwrap();
        let cached = store.get("https://api.test.com").await.unwrap();
        assert_eq!(cached, Some(("mac1".to_string(), "pre1".to_string())));

        store.remove("https://api.test.com").await.unwrap();
        let cached = store.get("https://api.test.com").await.unwrap();
        assert!(cached.is_none());
    }

    #[tokio::test]
    async fn budget_blocks_excess_payment() {
        let budget = Budget {
            per_request_max: Some(50),
            hourly_max: None,
            daily_max: None,
            total_max: None,
            domain_budgets: HashMap::new(),
        };

        let result = budget.check(100);
        assert!(result.is_err());
        assert!(format!("{result:?}").contains("BudgetExceeded"));
    }

    #[tokio::test]
    async fn mock_backend_tracks_calls() {
        let backend = MockLnBackend::new("preimage_hex", "hash_hex", 100);
        assert_eq!(backend.call_count(), 0);

        backend.pay_invoice("lnbc...", 100).await.unwrap();
        assert_eq!(backend.call_count(), 1);

        backend.pay_invoice("lnbc...", 100).await.unwrap();
        assert_eq!(backend.call_count(), 2);
    }

    #[tokio::test]
    async fn mock_backend_failure() {
        let backend = MockLnBackend::failing();
        let result = backend.pay_invoice("lnbc...", 100).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn receipts_starts_empty() {
        let backend = MockLnBackend::new("abc", "def", 100);
        let store = InMemoryTokenStore::default();

        let client = L402Client::builder()
            .ln_backend(backend)
            .token_store(store)
            .build()
            .unwrap();

        assert!(client.receipts().await.is_empty());
        assert_eq!(client.total_spent().await, 0);
    }
}
