//! Verify wallet REST server identity against the running regtest nodes.
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use l402_proto::LnBackend;
use regtest_helpers::*;

#[tokio::test]
async fn lnd_rest_requires_trusted_ca() {
    skip_if_no_regtest!();
    // Establish availability before testing the rejection path.
    lnd_rest_backend().get_info().await.unwrap();
    let url = std::env::var("LND_REST_HOST").unwrap();
    let macaroon = std::env::var("LND_MACAROON_HEX").unwrap();
    let untrusted = l402_lnd::LndRestBackend::new(&url, &macaroon).unwrap();
    assert!(untrusted.get_info().await.is_err());

    let ca = STANDARD
        .decode(std::env::var("LND_TLS_CERT_BASE64").unwrap())
        .unwrap();
    let client = reqwest::Client::builder()
        .tls_certs_only([reqwest::Certificate::from_pem(&ca).unwrap()])
        .no_proxy()
        .resolve("wrong-host.invalid", "127.0.0.1:8080".parse().unwrap())
        .build()
        .unwrap();
    let wrong_name =
        l402_lnd::LndRestBackend::with_client("https://wrong-host.invalid:8080", &macaroon, client);
    assert!(wrong_name.get_info().await.is_err());
}

#[tokio::test]
async fn cln_rest_requires_trusted_ca() {
    skip_if_no_regtest!();
    let Some(trusted) = cln_rest_backend().await else {
        return;
    };
    trusted.get_info().await.unwrap();
    let url = std::env::var("CLN_REST_URL").unwrap();
    let rune = std::env::var("CLN_RUNE").unwrap();
    let untrusted = l402_cln::ClnRestBackend::new(&url, &rune).unwrap();
    assert!(untrusted.get_info().await.is_err());
}

#[tokio::test]
async fn wallet_credentials_do_not_follow_redirects() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    init_tracing();
    for is_lnd in [true, false] {
        let destination = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let source = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", source.local_addr().unwrap());
        let reply = format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{}/collect\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            destination.local_addr().unwrap(),
        );
        let server = tokio::spawn(async move {
            let (mut socket, _) = source.accept().await.unwrap();
            let mut buffer = [0; 4096];
            assert!(socket.read(&mut buffer).await.unwrap() > 0);
            socket.write_all(reply.as_bytes()).await.unwrap();
        });
        let backend: Box<dyn LnBackend> = if is_lnd {
            Box::new(l402_lnd::LndRestBackend::new(&url, "dummy-macaroon").unwrap())
        } else {
            Box::new(l402_cln::ClnRestBackend::new(&url, "dummy-rune").unwrap())
        };
        assert!(backend.get_info().await.is_err());
        server.await.unwrap();
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), destination.accept())
                .await
                .is_err()
        );
    }
}
