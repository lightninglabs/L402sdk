//! Payment policy tests use a local HTTP peer and an observable wallet.
use async_trait::async_trait;
use l402_core::{L402Client, L402ClientConfig, budget::Budget, cache::InMemoryTokenStore};
use l402_proto::{
    ClientError, LnBackend, NodeInfo, PaymentResult, TokenStore, decode_bolt11_amount,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{Notify, Semaphore},
};

#[derive(Clone)]
struct Wallet {
    calls: Arc<AtomicUsize>,
    cap: Arc<AtomicUsize>,
    mode: Arc<AtomicUsize>,
    started: Arc<Notify>,
    gate: Arc<Semaphore>,
    supported: bool,
}
impl Wallet {
    fn new(mode: usize) -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            cap: Arc::new(AtomicUsize::new(0)),
            mode: Arc::new(AtomicUsize::new(mode)),
            started: Arc::new(Notify::new()),
            gate: Arc::new(Semaphore::new(0)),
            supported: true,
        }
    }
}
#[async_trait]
impl LnBackend for Wallet {
    fn supports_fee_limit(&self) -> bool {
        self.supported
    }
    async fn pay_invoice(&self, invoice: &str, cap: u64) -> Result<PaymentResult, ClientError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.cap
            .store(usize::try_from(cap).unwrap(), Ordering::SeqCst);
        self.started.notify_one();
        match self.mode.load(Ordering::SeqCst) {
            1 => {
                return Err(ClientError::PaymentRejected {
                    reason: "wallet confirms no settlement".into(),
                });
            }
            2 => {
                return Err(ClientError::Http {
                    reason: "connection lost after submission".into(),
                });
            }
            3 => {
                let _permit = self.gate.acquire().await.unwrap();
            }
            _ => {}
        }
        let fee = if self.mode.load(Ordering::SeqCst) == 4 {
            cap + 1
        } else {
            3
        };
        if fee > cap && self.mode.load(Ordering::SeqCst) != 4 {
            return Err(ClientError::PaymentRejected {
                reason: "route exceeds fee limit".into(),
            });
        }
        Ok(PaymentResult {
            amount_sats: match self.mode.load(Ordering::SeqCst) {
                5 => u64::MAX,
                6 => 99,
                _ => decode_bolt11_amount(invoice).unwrap().unwrap().satoshis(),
            },
            fee_sats: fee,
            preimage: "00".repeat(32),
            payment_hash: "11".repeat(32),
        })
    }
    async fn get_balance(&self) -> Result<u64, ClientError> {
        Ok(1000)
    }
    async fn get_info(&self) -> Result<NodeInfo, ClientError> {
        unreachable!()
    }
}

struct Server {
    url: String,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn server(paid_status: u16, redirect: Option<String>) -> Server {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let redirect = redirect.clone();
            tokio::spawn(async move {
                let mut bytes = vec![0; 8192];
                let count = socket.read(&mut bytes).await.unwrap();
                let request = String::from_utf8_lossy(&bytes[..count]);
                let path = request.split_whitespace().nth(1).unwrap_or("/");
                let paid = request.to_ascii_lowercase().contains("authorization:");
                if paid && paid_status == 0 {
                    return;
                }
                let (status, header) = if let Some(location) = redirect {
                    (302, format!("Location: {location}\r\n"))
                } else if paid && paid_status != 402 {
                    (paid_status, String::new())
                } else {
                    (
                        402,
                        format!(
                            "WWW-Authenticate: L402 macaroon=\"YWJj\", invoice=\"lnbc1000n1{}\"\r\n",
                            path.replace('/', "x")
                        ),
                    )
                };
                let response = format!(
                    "HTTP/1.1 {status} Result\r\n{header}Content-Length: 0\r\nConnection: close\r\n\r\n"
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            });
        }
    });
    Server { url, task }
}
fn client(wallet: Wallet, budget: Budget) -> Arc<L402Client> {
    Arc::new(
        L402Client::builder()
            .ln_backend(wallet)
            .token_store(InMemoryTokenStore::default())
            .budget(budget)
            .config(L402ClientConfig {
                max_fee_sats: 10,
                ..Default::default()
            })
            .build()
            .unwrap(),
    )
}

#[tokio::test]
async fn cap_is_clipped_to_fee_inclusive_budget_and_unused_fees_are_released() {
    let server = server(200, None).await;
    let wallet = Wallet::new(0);
    let client = client(
        wallet.clone(),
        Budget {
            total_max: Some(105),
            ..Budget::unlimited()
        },
    );
    let response = client.get(&server.url).await.unwrap();
    assert_eq!(wallet.cap.load(Ordering::SeqCst), 5);
    assert_eq!(response.receipt().unwrap().total_cost_sats(), 103);
    assert_eq!(client.total_spent().await, 103);
    assert!(matches!(
        client.get(&format!("{}/other", server.url)).await,
        Err(ClientError::BudgetExceeded { .. })
    ));
    assert_eq!(wallet.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn exact_budget_only_allows_zero_fee_and_rejection_releases_reservation() {
    let server = server(200, None).await;
    let wallet = Wallet::new(0);
    let client = client(
        wallet.clone(),
        Budget {
            per_request_max: Some(100),
            ..Budget::unlimited()
        },
    );
    assert!(matches!(
        client.get(&server.url).await,
        Err(ClientError::PaymentRejected { .. })
    ));
    assert_eq!(wallet.cap.load(Ordering::SeqCst), 0);
    assert_eq!(client.total_spent().await, 0);
}

#[tokio::test]
async fn confirmed_failure_can_retry_but_unknown_failure_cannot() {
    let server = server(200, None).await;
    for mode in [1, 2] {
        let wallet = Wallet::new(mode);
        let client = client(wallet.clone(), Budget::unlimited());
        assert!(client.get(&server.url).await.is_err());
        wallet.mode.store(0, Ordering::SeqCst);
        if mode == 1 {
            assert_eq!(client.total_spent().await, 0);
            client.get(&server.url).await.unwrap();
            assert_eq!(wallet.calls.load(Ordering::SeqCst), 2);
        } else {
            assert_eq!(client.total_spent().await, 110);
            assert!(matches!(
                client.get(&server.url).await,
                Err(ClientError::PaymentUncertain)
            ));
            assert_eq!(wallet.calls.load(Ordering::SeqCst), 1);
        }
    }
}

#[tokio::test]
async fn concurrent_requests_cannot_pay_twice_or_overbook_budget() {
    let server = server(200, None).await;
    let wallet = Wallet::new(3);
    let client = client(
        wallet.clone(),
        Budget {
            total_max: Some(150),
            ..Budget::unlimited()
        },
    );
    let pending = tokio::spawn({
        let client = client.clone();
        let url = server.url.clone();
        async move { client.get(&url).await }
    });
    wallet.started.notified().await;
    assert!(matches!(
        client.get(&server.url).await,
        Err(ClientError::PaymentUncertain)
    ));
    assert!(matches!(
        client.get(&format!("{}/different", server.url)).await,
        Err(ClientError::BudgetExceeded { .. })
    ));
    assert_eq!(client.total_spent().await, 110);
    wallet.gate.add_permits(1);
    pending.await.unwrap().unwrap();
    assert_eq!(client.total_spent().await, 103);
    assert_eq!(wallet.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancelled_payment_keeps_reservation_and_blocks_resubmission() {
    let server = server(200, None).await;
    let wallet = Wallet::new(3);
    let client = client(wallet.clone(), Budget::unlimited());
    let pending = tokio::spawn({
        let client = client.clone();
        let url = server.url.clone();
        async move { client.get(&url).await }
    });
    wallet.started.notified().await;
    pending.abort();
    assert!(pending.await.err().unwrap().is_cancelled());
    assert_eq!(client.total_spent().await, 110);
    assert!(matches!(
        client.get(&server.url).await,
        Err(ClientError::PaymentUncertain)
    ));
    assert_eq!(wallet.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn backend_without_fee_cap_is_rejected_before_payment() {
    let server = server(200, None).await;
    let mut wallet = Wallet::new(0);
    wallet.supported = false;
    let client = client(wallet.clone(), Budget::unlimited());
    assert!(matches!(
        client.get(&server.url).await,
        Err(ClientError::FeeLimitUnsupported)
    ));
    assert_eq!(wallet.calls.load(Ordering::SeqCst), 0);
    assert_eq!(client.total_spent().await, 0);
}

#[tokio::test]
async fn broken_backend_retains_receipt_and_freezes_spending() {
    let server = server(200, None).await;
    let wallet = Wallet::new(4);
    let client = client(wallet.clone(), Budget::unlimited());
    assert!(client.get(&server.url).await.is_err());
    assert_eq!(client.receipts().await[0].fee_sats, 11);
    assert_eq!(client.total_spent().await, 111);
    assert!(matches!(
        client.get(&format!("{}/other", server.url)).await,
        Err(ClientError::BudgetExceeded { .. })
    ));
    assert_eq!(wallet.calls.load(Ordering::SeqCst), 1);
}

struct FaultyStore {
    block: bool,
    fail_put: bool,
}
#[async_trait]
impl TokenStore for FaultyStore {
    async fn put(&self, _: &str, _: &str, _: &str) -> Result<(), ClientError> {
        if self.block {
            std::future::pending::<()>().await;
        }
        if !self.fail_put {
            return Ok(());
        }
        Err(ClientError::Backend {
            reason: "disk full".into(),
        })
    }
    async fn get(&self, _: &str) -> Result<Option<(String, String)>, ClientError> {
        Ok(None)
    }
    async fn remove(&self, _: &str) -> Result<(), ClientError> {
        Err(ClientError::Backend {
            reason: "disk full".into(),
        })
    }
    async fn clear(&self) -> Result<(), ClientError> {
        Ok(())
    }
}

#[tokio::test]
async fn receipt_survives_failed_or_cancelled_persistence_without_repayment() {
    let server = server(200, None).await;
    for block in [false, true] {
        let wallet = Wallet::new(0);
        let client = L402Client::builder()
            .ln_backend(wallet.clone())
            .token_store(FaultyStore {
                block,
                fail_put: true,
            })
            .build()
            .unwrap();
        for attempt in 0..2 {
            let result = tokio::time::timeout(
                std::time::Duration::from_millis(100),
                client.get(&server.url),
            )
            .await;
            if attempt == 0 {
                assert!(result.is_err() || result.unwrap().is_err());
            } else {
                let response = result.unwrap().unwrap();
                assert!(response.cached_token());
                assert!(!response.paid());
            }
            assert_eq!(client.receipts().await.len(), 1);
            assert_eq!(client.receipts().await[0].response_status, 0);
            assert_eq!(client.total_spent().await, 103);
        }
        assert_eq!(wallet.calls.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn receipt_survives_http_failure_rejected_token_and_server_error() {
    for status in [0, 402, 500] {
        let server = server(status, None).await;
        let wallet = Wallet::new(0);
        let client = client(wallet.clone(), Budget::unlimited());
        let result = client.get(&server.url).await;
        assert_eq!(result.is_ok(), status == 500);
        let receipts = client.receipts().await;
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].response_status, status);
        assert_eq!(client.total_spent().await, 103);
        let _ = client.get(&server.url).await;
        assert_eq!(wallet.calls.load(Ordering::SeqCst), 1);
        assert_eq!(client.receipts().await.len(), 1);
    }
}

#[tokio::test]
async fn redirect_cannot_move_payment_to_another_origin() {
    let target = server(200, None).await;
    let redirect = server(200, Some(target.url.clone())).await;
    let wallet = Wallet::new(0);
    let client = client(wallet.clone(), Budget::unlimited());
    assert_eq!(
        client.get(&redirect.url).await.unwrap().status().as_u16(),
        302
    );
    assert_eq!(wallet.calls.load(Ordering::SeqCst), 0);
    // A caller-supplied redirect-following client still cannot authorize a challenge
    // from a different origin using the original domain's budget.
    let client = L402Client::builder()
        .ln_backend(wallet.clone())
        .token_store(InMemoryTokenStore::default())
        .http_client(reqwest::Client::new())
        .build()
        .unwrap();
    assert!(matches!(
        client.get(&redirect.url).await,
        Err(ClientError::Http { .. })
    ));
    assert_eq!(wallet.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn failed_token_removal_keeps_paid_receipt() {
    let server = server(402, None).await;
    let wallet = Wallet::new(0);
    let client = L402Client::builder()
        .ln_backend(wallet.clone())
        .token_store(FaultyStore {
            block: false,
            fail_put: false,
        })
        .build()
        .unwrap();
    for _ in 0..2 {
        assert!(client.get(&server.url).await.is_err());
    }
    assert_eq!(client.receipts().await.len(), 1);
    assert_eq!(client.receipts().await[0].response_status, 402);
    assert_eq!(client.total_spent().await, 103);
    assert_eq!(wallet.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn different_invoices_settle_concurrently_without_losing_receipts() {
    let server = server(200, None).await;
    let wallet = Wallet::new(3);
    let client = client(
        wallet.clone(),
        Budget {
            total_max: Some(220),
            ..Budget::unlimited()
        },
    );
    let mut requests = Vec::new();
    for suffix in ["a", "b"] {
        let client = client.clone();
        let url = format!("{}/{suffix}", server.url);
        requests.push(tokio::spawn(async move { client.get(&url).await }));
    }
    while wallet.calls.load(Ordering::SeqCst) < 2 {
        wallet.started.notified().await;
    }
    assert_eq!(client.total_spent().await, 220);
    wallet.gate.add_permits(2);
    for request in requests {
        request.await.unwrap().unwrap();
    }
    assert_eq!(client.total_spent().await, 206);
    let receipts = client.receipts().await;
    assert_eq!(receipts.len(), 2);
    assert_ne!(receipts[0].endpoint, receipts[1].endpoint);
    assert!(
        receipts
            .iter()
            .all(|receipt| receipt.response_status == 200 && receipt.total_cost_sats() == 103)
    );
}

#[tokio::test]
async fn malformed_settlement_amounts_cannot_reopen_budget() {
    let server = server(200, None).await;
    for mode in [5, 6] {
        let wallet = Wallet::new(mode);
        let client = client(wallet.clone(), Budget::unlimited());
        assert!(client.get(&server.url).await.is_err());
        assert_eq!(client.receipts().await.len(), 1);
        assert_eq!(
            client.total_spent().await,
            if mode == 5 { u64::MAX } else { 102 }
        );
        assert!(matches!(
            client.get(&format!("{}/other", server.url)).await,
            Err(ClientError::BudgetExceeded { .. })
        ));
        assert_eq!(wallet.calls.load(Ordering::SeqCst), 1);
    }
}
