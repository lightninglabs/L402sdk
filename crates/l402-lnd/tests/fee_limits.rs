//! Verify actual wire fee limits and conservative settlement rounding.
#![cfg(feature = "rest")]
use l402_lnd::LndRestBackend;
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
                        assert_eq!(request["fee_limit_sat"], cap);
                        break;
                    }
                }
            }
            let body = r#"{"result":{"status":"SUCCEEDED","payment_preimage":"0000000000000000000000000000000000000000000000000000000000000000","payment_hash":"66687aadf862bd776c8fc18b8e9f8e20089714856ee233b3902a591d0d5f2925","value_msat":"100001","fee_msat":"1"}}"#;
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(reply.as_bytes()).await.unwrap();
        });
        let backend = LndRestBackend::new(&url, "00").unwrap();
        assert!(backend.supports_fee_limit());
        let paid = backend.pay_invoice("lnbc1000010p1test", cap).await.unwrap();
        assert_eq!(paid.amount_sats, 101);
        assert_eq!(paid.fee_sats, 1);
        server.await.unwrap();
    }
}
