//! Mock L402 HTTP server.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

use crate::MockLnBackend;
use crate::challenge::PendingChallenge;

/// Configuration for a protected endpoint.
#[derive(Debug, Clone)]
pub struct EndpointConfig {
    /// Price in satoshis to access this endpoint.
    pub price_sats: u64,
    /// Response body when authenticated.
    pub response_body: String,
}

impl EndpointConfig {
    /// Create a new endpoint config with the given price.
    pub fn new(price_sats: u64) -> Self {
        Self {
            price_sats,
            response_body: format!(r#"{{"ok":true,"price":{price_sats}}}"#),
        }
    }

    /// Set a custom response body.
    #[must_use]
    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.response_body = body.into();
        self
    }
}

/// Shared state between the server and mock backend.
#[derive(Debug, Clone)]
pub(crate) struct SharedState {
    /// Protected endpoints and their configs.
    pub endpoints: HashMap<String, EndpointConfig>,
    /// Issued challenges, keyed by invoice string.
    pub challenges: Arc<RwLock<HashMap<String, PendingChallenge>>>,
}

/// Builder for [`MockL402Server`].
pub struct MockL402ServerBuilder {
    endpoints: HashMap<String, EndpointConfig>,
}

impl MockL402ServerBuilder {
    /// Add a protected endpoint.
    #[must_use]
    pub fn endpoint(mut self, path: &str, config: EndpointConfig) -> Self {
        self.endpoints.insert(path.to_string(), config);
        self
    }

    /// Build and start the mock server on a random port.
    pub async fn build(self) -> Result<MockL402Server, std::io::Error> {
        let state = SharedState {
            endpoints: self.endpoints,
            challenges: Arc::new(RwLock::new(HashMap::new())),
        };

        let shared = Arc::new(state);

        let app = Router::new()
            .fallback(get(handle_request).post(handle_request))
            .with_state(shared.clone());

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        Ok(MockL402Server {
            addr,
            state: shared,
            _handle: handle,
        })
    }
}

/// A running mock L402 server.
///
/// The server listens on a random local port and responds to requests
/// for configured endpoints with L402 challenges.
pub struct MockL402Server {
    addr: SocketAddr,
    state: Arc<SharedState>,
    _handle: tokio::task::JoinHandle<()>,
}

impl MockL402Server {
    /// Create a new builder.
    pub fn builder() -> MockL402ServerBuilder {
        MockL402ServerBuilder {
            endpoints: HashMap::new(),
        }
    }

    /// Get the base URL of the running server.
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Create a [`MockLnBackend`] connected to this server's challenge registry.
    ///
    /// The mock backend "pays" invoices by looking up the preimage from the
    /// server's pending challenges.
    pub fn mock_backend(&self) -> MockLnBackend {
        MockLnBackend::new(self.state.challenges.clone())
    }
}

/// Handle all incoming requests with L402 authentication.
async fn handle_request(
    State(state): State<Arc<SharedState>>,
    headers: HeaderMap,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> impl IntoResponse {
    let path = uri.path().to_string();

    // Check if this endpoint is protected
    let Some(config) = state.endpoints.get(&path) else {
        return (
            StatusCode::NOT_FOUND,
            HeaderMap::new(),
            "not found".to_string(),
        );
    };

    // Check for existing authorization
    if let Some(auth) = headers.get("authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if let Some(result) = validate_l402_auth(auth_str, &state).await {
                if result {
                    return (
                        StatusCode::OK,
                        HeaderMap::new(),
                        config.response_body.clone(),
                    );
                }
                return (
                    StatusCode::UNAUTHORIZED,
                    HeaderMap::new(),
                    "invalid L402 token".to_string(),
                );
            }
        }
    }

    // No valid auth: issue a new challenge
    let challenge = PendingChallenge::generate(config.price_sats);
    let www_auth = challenge.to_www_authenticate();

    // Store the challenge so the mock backend can look up the preimage
    {
        let mut challenges = state.challenges.write().await;
        challenges.insert(challenge.invoice.clone(), challenge);
    }

    let mut resp_headers = HeaderMap::new();
    resp_headers.insert("www-authenticate", www_auth.parse().expect("valid header"));

    (
        StatusCode::PAYMENT_REQUIRED,
        resp_headers,
        "payment required".to_string(),
    )
}

/// Validate an `Authorization: L402 <macaroon>:<preimage>` header.
///
/// Returns `Some(true)` if valid, `Some(false)` if invalid format/token,
/// `None` if not an L402 auth header.
async fn validate_l402_auth(auth: &str, state: &SharedState) -> Option<bool> {
    let stripped = auth
        .strip_prefix("L402 ")
        .or_else(|| auth.strip_prefix("LSAT "))?;

    let (macaroon, preimage) = stripped.split_once(':')?;

    let challenges = state.challenges.read().await;
    for challenge in challenges.values() {
        if challenge.validate_auth(macaroon, preimage) {
            return Some(true);
        }
    }

    Some(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn server_returns_402() {
        let server = MockL402Server::builder()
            .endpoint("/api/test", EndpointConfig::new(100))
            .build()
            .await
            .unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{}/api/test", server.url()))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 402);
        let www_auth = resp
            .headers()
            .get("www-authenticate")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(www_auth.starts_with("L402 macaroon=\""));
    }

    #[tokio::test]
    async fn server_returns_404_for_unknown() {
        let server = MockL402Server::builder()
            .endpoint("/api/test", EndpointConfig::new(100))
            .build()
            .await
            .unwrap();

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{}/unknown", server.url()))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 404);
    }

    #[tokio::test]
    async fn server_accepts_valid_token() {
        let server = MockL402Server::builder()
            .endpoint("/api/test", EndpointConfig::new(100))
            .build()
            .await
            .unwrap();

        let client = reqwest::Client::new();

        // First request: get the challenge
        let resp = client
            .get(format!("{}/api/test", server.url()))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 402);

        // Extract challenge and look up preimage from the server's registry
        let challenges = server.state.challenges.read().await;
        let challenge = challenges.values().next().unwrap().clone();
        drop(challenges);

        // Retry with valid token
        let auth = format!("L402 {}:{}", challenge.macaroon, challenge.preimage);
        let resp = client
            .get(format!("{}/api/test", server.url()))
            .header("authorization", auth)
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn server_rejects_invalid_preimage() {
        let server = MockL402Server::builder()
            .endpoint("/api/test", EndpointConfig::new(100))
            .build()
            .await
            .unwrap();

        let client = reqwest::Client::new();

        // Get challenge
        let _ = client
            .get(format!("{}/api/test", server.url()))
            .send()
            .await
            .unwrap();

        let challenges = server.state.challenges.read().await;
        let challenge = challenges.values().next().unwrap().clone();
        drop(challenges);

        // Retry with wrong preimage
        let fake_preimage = "0".repeat(64);
        let auth = format!("L402 {}:{fake_preimage}", challenge.macaroon);
        let resp = client
            .get(format!("{}/api/test", server.url()))
            .header("authorization", auth)
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 401);
    }
}
