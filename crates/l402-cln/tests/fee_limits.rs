//! Verify actual wire fee limits and conservative settlement rounding.
#![cfg(feature = "rest")]
use l402_cln::ClnRestBackend;
use l402_proto::LnBackend;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn forwards_zero_and_nonzero_caps_and_rounds_subsat_amounts_up() {
    for cap in [0_u64, 7] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut buf = [0; 1024];
                let count = socket.read(&mut buf).await.unwrap();
                assert!(count > 0);
                bytes.extend_from_slice(&buf[..count]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                    let length: usize = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length: "))
                        .unwrap()
                        .parse()
                        .unwrap();
                    if bytes.len() >= end + 4 + length {
                        let request: serde_json::Value =
                            serde_json::from_slice(&bytes[end + 4..end + 4 + length]).unwrap();
                        assert_eq!(request["maxfee"], format!("{}msat", cap * 1000));
                        break;
                    }
                }
            }
            let body = r#"{"status":"complete","payment_preimage":"0000000000000000000000000000000000000000000000000000000000000000","payment_hash":"1111111111111111111111111111111111111111111111111111111111111111","amount_msat":"100001msat","amount_sent_msat":"100002msat"}"#;
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(reply.as_bytes()).await.unwrap();
        });
        let backend = ClnRestBackend::new(&url, "test").unwrap();
        assert!(backend.supports_fee_limit());
        let paid = backend.pay_invoice("lnbc1000010p1test", cap).await.unwrap();
        assert_eq!(paid.amount_sats, 101);
        assert_eq!(paid.fee_sats, 1);
        server.await.unwrap();
    }
}
