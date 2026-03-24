use base64::Engine;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};

/// Base64 decoder that accepts both padded and unpadded input.
///
/// Many L402 servers omit base64 padding (`=`) from macaroons.
/// The L402 spec doesn't mandate padding, so we accept both.
const BASE64: GeneralPurpose = GeneralPurpose::new(
    &base64::alphabet::STANDARD,
    GeneralPurposeConfig::new()
        .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent),
);

use crate::L402Error;

/// A parsed L402 challenge from a `WWW-Authenticate` header.
///
/// When a server returns `HTTP 402 Payment Required`, it includes a
/// `WWW-Authenticate` header with the L402 scheme containing a macaroon
/// and a Lightning invoice.
///
/// ## Header Format
///
/// ```text
/// WWW-Authenticate: L402 macaroon="<base64>", invoice="<bolt11>"
/// ```
///
/// Some servers may also include additional fields like `address` (on-chain)
/// or custom metadata.
#[derive(Debug, Clone)]
pub struct L402Challenge {
    /// Base64-encoded macaroon from the challenge.
    pub macaroon: String,

    /// BOLT11 Lightning invoice to pay.
    pub invoice: String,

    /// Optional on-chain address (some L402 implementations support this).
    pub address: Option<String>,
}

impl L402Challenge {
    /// Parse an L402 challenge from a `WWW-Authenticate` header value.
    ///
    /// Supports both the `L402` and legacy `LSAT` schemes.
    ///
    /// # Errors
    ///
    /// Returns [`L402Error::InvalidChallenge`] if the header is malformed
    /// or missing required fields (macaroon, invoice).
    pub fn from_header(header: &str) -> Result<Self, L402Error> {
        let header = header.trim();

        // Strip the scheme prefix (L402 or LSAT)
        let params = if let Some(rest) = header.strip_prefix("L402 ") {
            rest
        } else if let Some(rest) = header.strip_prefix("LSAT ") {
            rest
        } else {
            return Err(L402Error::InvalidChallenge {
                reason: format!(
                    "header must start with 'L402' or 'LSAT' scheme, got: {}",
                    header.chars().take(20).collect::<String>()
                ),
            });
        };

        let mut macaroon = None;
        let mut invoice = None;
        let mut address = None;

        // Parse key="value" pairs, handling commas within values
        for part in Self::parse_params(params) {
            let (key, value) = Self::parse_kv(&part)?;
            match key.as_str() {
                "macaroon" => macaroon = Some(value),
                "invoice" => invoice = Some(value),
                "address" => address = Some(value),
                _ => {
                    tracing::debug!(key = %key, "ignoring unknown L402 challenge parameter");
                }
            }
        }

        let macaroon = macaroon.ok_or_else(|| L402Error::InvalidChallenge {
            reason: "missing 'macaroon' parameter".to_string(),
        })?;

        let invoice = invoice.ok_or_else(|| L402Error::InvalidChallenge {
            reason: "missing 'invoice' parameter".to_string(),
        })?;

        // Validate macaroon is valid base64
        BASE64
            .decode(&macaroon)
            .map_err(|e| L402Error::InvalidMacaroon {
                reason: format!("base64 decode failed: {e}"),
            })?;

        // Validate invoice looks like a BOLT11 invoice
        if !invoice.starts_with("lnbc") && !invoice.starts_with("lntb") {
            return Err(L402Error::InvalidInvoice {
                reason: format!(
                    "invoice must start with 'lnbc' (mainnet) or 'lntb' (testnet), got: {}",
                    invoice.chars().take(10).collect::<String>()
                ),
            });
        }

        Ok(Self {
            macaroon,
            invoice,
            address,
        })
    }

    /// Parse comma-separated key="value" parameters.
    fn parse_params(input: &str) -> Vec<String> {
        let mut params = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;

        for ch in input.chars() {
            match ch {
                '"' => {
                    in_quotes = !in_quotes;
                    current.push(ch);
                }
                ',' if !in_quotes => {
                    let trimmed = current.trim().to_string();
                    if !trimmed.is_empty() {
                        params.push(trimmed);
                    }
                    current.clear();
                }
                _ => current.push(ch),
            }
        }

        let trimmed = current.trim().to_string();
        if !trimmed.is_empty() {
            params.push(trimmed);
        }

        params
    }

    /// Parse a single `key="value"` pair.
    fn parse_kv(param: &str) -> Result<(String, String), L402Error> {
        let (key, rest) = param
            .split_once('=')
            .ok_or_else(|| L402Error::InvalidChallenge {
                reason: format!("expected key=value pair, got: {param}"),
            })?;

        let value = rest.trim().trim_matches('"').to_string();
        Ok((key.trim().to_lowercase(), value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_l402_challenge() {
        let header = r#"L402 macaroon="YWJjZGVm", invoice="lnbc100n1pj9nr7mpp5test""#;
        let challenge = L402Challenge::from_header(header).unwrap();

        assert_eq!(challenge.macaroon, "YWJjZGVm");
        assert_eq!(challenge.invoice, "lnbc100n1pj9nr7mpp5test");
        assert!(challenge.address.is_none());
    }

    #[test]
    fn parse_valid_lsat_challenge() {
        let header = r#"LSAT macaroon="YWJjZGVm", invoice="lnbc100n1pj9nr7mpp5test""#;
        let challenge = L402Challenge::from_header(header).unwrap();

        assert_eq!(challenge.macaroon, "YWJjZGVm");
    }

    #[test]
    fn parse_challenge_with_address() {
        let header =
            r#"L402 macaroon="YWJjZGVm", invoice="lnbc100n1pj9nr7mpp5test", address="bc1qtest""#;
        let challenge = L402Challenge::from_header(header).unwrap();

        assert_eq!(challenge.address.as_deref(), Some("bc1qtest"));
    }

    #[test]
    fn reject_missing_macaroon() {
        let header = r#"L402 invoice="lnbc100n1pj9nr7mpp5test""#;
        let err = L402Challenge::from_header(header).unwrap_err();
        assert!(matches!(err, L402Error::InvalidChallenge { .. }));
    }

    #[test]
    fn reject_missing_invoice() {
        let header = r#"L402 macaroon="YWJjZGVm""#;
        let err = L402Challenge::from_header(header).unwrap_err();
        assert!(matches!(err, L402Error::InvalidChallenge { .. }));
    }

    #[test]
    fn reject_invalid_scheme() {
        let header = r#"Bearer token="abc""#;
        let err = L402Challenge::from_header(header).unwrap_err();
        assert!(matches!(err, L402Error::InvalidChallenge { .. }));
    }

    #[test]
    fn reject_invalid_base64_macaroon() {
        let header = r#"L402 macaroon="not-valid-base64!!!", invoice="lnbc100n1pj9nr7mpp5test""#;
        let err = L402Challenge::from_header(header).unwrap_err();
        assert!(matches!(err, L402Error::InvalidMacaroon { .. }));
    }

    #[test]
    fn reject_invalid_invoice_prefix() {
        let header = r#"L402 macaroon="YWJjZGVm", invoice="invalid_invoice""#;
        let err = L402Challenge::from_header(header).unwrap_err();
        assert!(matches!(err, L402Error::InvalidInvoice { .. }));
    }
}
