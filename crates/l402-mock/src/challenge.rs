//! Challenge generation and validation for the mock server.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use sha2::{Digest, Sha256};

/// A pending L402 challenge issued by the mock server.
///
/// Contains the preimage (secret) and all data needed to validate
/// an authorization token.
#[derive(Debug, Clone)]
pub struct PendingChallenge {
    /// Hex-encoded preimage (proof of payment).
    pub preimage: String,

    /// SHA256 of the preimage (hex-encoded).
    pub payment_hash: String,

    /// Base64-encoded macaroon.
    pub macaroon: String,

    /// Fake BOLT11 invoice string.
    pub invoice: String,

    /// Amount in satoshis.
    pub amount_sats: u64,
}

impl PendingChallenge {
    /// Generate a new challenge with a random preimage.
    pub fn generate(amount_sats: u64) -> Self {
        let preimage_bytes: [u8; 32] = rand_bytes();
        let preimage = hex::encode(preimage_bytes);

        let mut hasher = Sha256::new();
        hasher.update(preimage_bytes);
        let hash_bytes = hasher.finalize();
        let payment_hash = hex::encode(hash_bytes);

        // Simple test macaroon: base64-encoded JSON with payment hash
        let macaroon_data = format!(r#"{{"payment_hash":"{payment_hash}"}}"#);
        let macaroon = BASE64.encode(macaroon_data.as_bytes());

        // Encode amount as microbitcoin: 1u = 100 sats, so amount_sats / 100 = value in u.
        // For amounts not divisible by 100, use nanobitcoin: 1n = 0.1 sats, so amount_sats * 10 = value in n.
        // For sub-sat precision, we default to nanobitcoin.
        //
        // The data portion uses bech32-safe characters (no '1', 'b', 'i', 'o')
        // so that rfind('1') correctly finds the separator in BOLT11 parsing.
        let safe_hash = payment_hash
            .chars()
            .map(|c| if c == '1' { 'x' } else { c })
            .take(20)
            .collect::<String>();

        let invoice = if amount_sats >= 100 && amount_sats % 100 == 0 {
            format!("lnbc{}u1mock{safe_hash}", amount_sats / 100)
        } else {
            // Use nanobitcoin: amount_sats sats = amount_sats * 10 nanobitcoin
            format!("lnbc{}n1mock{safe_hash}", amount_sats * 10)
        };

        Self {
            preimage,
            payment_hash,
            macaroon,
            invoice,
            amount_sats,
        }
    }

    /// Format as a `WWW-Authenticate` header value.
    pub fn to_www_authenticate(&self) -> String {
        format!(
            r#"L402 macaroon="{}", invoice="{}""#,
            self.macaroon, self.invoice
        )
    }

    /// Validate a preimage against this challenge's payment hash.
    pub fn validate_preimage(&self, preimage_hex: &str) -> bool {
        let Ok(preimage_bytes) = hex::decode(preimage_hex) else {
            return false;
        };
        let mut hasher = Sha256::new();
        hasher.update(&preimage_bytes);
        hex::encode(hasher.finalize()) == self.payment_hash
    }

    /// Validate an authorization header (macaroon + preimage).
    pub fn validate_auth(&self, macaroon: &str, preimage_hex: &str) -> bool {
        macaroon == self.macaroon && self.validate_preimage(preimage_hex)
    }
}

/// Generate 32 random bytes from `/dev/urandom`.
fn rand_bytes() -> [u8; 32] {
    let mut buf = [0u8; 32];

    #[cfg(unix)]
    {
        use std::io::Read;
        if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
            let _ = f.read_exact(&mut buf);
            return buf;
        }
    }

    // Fallback: timestamp-based (not cryptographic, but fine for tests)
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_nanos();
    #[allow(clippy::cast_possible_truncation)]
    for (i, byte) in buf.iter_mut().enumerate() {
        let shift_a = i % 16;
        let shift_b = (i + 7) % 16;
        *byte = ((seed >> shift_a) ^ (seed >> shift_b)) as u8;
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_and_validate() {
        let challenge = PendingChallenge::generate(100);
        assert!(challenge.validate_preimage(&challenge.preimage));
        assert!(challenge.validate_auth(&challenge.macaroon, &challenge.preimage));
    }

    #[test]
    fn reject_wrong_preimage() {
        let challenge = PendingChallenge::generate(100);
        let fake = "0".repeat(64);
        assert!(!challenge.validate_preimage(&fake));
    }

    #[test]
    fn reject_wrong_macaroon() {
        let challenge = PendingChallenge::generate(100);
        assert!(!challenge.validate_auth("wrong", &challenge.preimage));
    }

    #[test]
    fn www_authenticate_format() {
        let challenge = PendingChallenge::generate(100);
        let header = challenge.to_www_authenticate();
        assert!(header.starts_with("L402 macaroon=\""));
        // 100 sats = 1u (microbitcoin)
        assert!(header.contains("invoice=\"lnbc1u1mock"));
    }

    #[test]
    fn invoice_amount_encoding_micro() {
        // Amounts divisible by 100 use microbitcoin
        let challenge = PendingChallenge::generate(500);
        assert!(challenge.invoice.starts_with("lnbc5u1mock"));
    }

    #[test]
    fn invoice_amount_encoding_nano() {
        // Amounts not divisible by 100 use nanobitcoin
        let challenge = PendingChallenge::generate(50);
        // 50 sats = 500 nanobitcoin
        assert!(challenge.invoice.starts_with("lnbc500n1mock"));
    }
}
