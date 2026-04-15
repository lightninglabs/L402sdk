//! MPP protocol types, parsing, and credential construction.
//!
//! This module adds the Machine Payments Protocol (MPP) wire-format types used
//! for HTTP 402 flows with the `Payment` authentication scheme.
//!
//! Phase 1 support is intentionally Lightning-charge focused:
//! - parse `WWW-Authenticate: Payment ...` challenges
//! - decode Lightning charge request objects
//! - construct `Authorization: Payment ...` credentials
//! - parse `Payment-Receipt` headers

use base64::Engine;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};
use serde::{Deserialize, Serialize};

use crate::L402Error;

/// Base64url encoder/decoder that accepts both padded and unpadded input.
const BASE64_URL_SAFE: GeneralPurpose = GeneralPurpose::new(
    &base64::alphabet::URL_SAFE,
    GeneralPurposeConfig::new()
        .with_encode_padding(false)
        .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent),
);

/// A parsed MPP challenge from a `WWW-Authenticate` header.
///
/// MPP uses the `Payment` authentication scheme instead of `L402`/`LSAT`.
/// The challenge parameters are then echoed back inside the client-submitted
/// credential, with the method-specific payment proof in `payload`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MppChallenge {
    /// Unique challenge identifier.
    pub id: String,

    /// Protection space identifier, typically the API domain.
    pub realm: String,

    /// Payment method identifier, for example `lightning`.
    pub method: String,

    /// Payment intent, for example `charge`.
    pub intent: String,

    /// Base64url-encoded JSON request object.
    pub request: String,

    /// Optional RFC 3339 / ISO 8601 expiry timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,

    /// Optional human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl MppChallenge {
    /// Parse an MPP challenge from a `WWW-Authenticate` header value.
    ///
    /// # Errors
    ///
    /// Returns [`L402Error::InvalidChallenge`] if the header is malformed or
    /// does not use the `Payment` authentication scheme.
    pub fn from_header(header: &str) -> Result<Self, L402Error> {
        let header = header.trim();
        let params =
            header
                .strip_prefix("Payment ")
                .ok_or_else(|| L402Error::InvalidChallenge {
                    reason: format!(
                        "header must start with 'Payment' scheme, got: {}",
                        header.chars().take(20).collect::<String>()
                    ),
                })?;

        let mut id = None;
        let mut realm = None;
        let mut method = None;
        let mut intent = None;
        let mut request = None;
        let mut expires = None;
        let mut description = None;

        for part in parse_params(params) {
            let (key, value) =
                parse_kv(&part).map_err(|reason| L402Error::InvalidChallenge { reason })?;

            match key.as_str() {
                "id" => id = Some(value),
                "realm" => realm = Some(value),
                "method" => method = Some(value),
                "intent" => intent = Some(value),
                "request" => request = Some(value),
                "expires" => expires = Some(value),
                "description" => description = Some(value),
                _ => {
                    tracing::debug!(key = %key, "ignoring unknown MPP challenge parameter");
                }
            }
        }

        let challenge = Self {
            id: required_field(id, "id")?,
            realm: required_field(realm, "realm")?,
            method: required_field(method, "method")?,
            intent: required_field(intent, "intent")?,
            request: required_field(request, "request")?,
            expires,
            description,
        };

        let _: serde_json::Value = decode_base64url_json(&challenge.request).map_err(|reason| {
            L402Error::InvalidChallenge {
                reason: format!("invalid request object: {reason}"),
            }
        })?;

        Ok(challenge)
    }

    /// Decode the challenge's method-specific request object as a Lightning
    /// `charge` request.
    ///
    /// # Errors
    ///
    /// Returns [`L402Error::InvalidChallenge`] when the challenge does not offer
    /// Lightning `charge`, the request JSON is invalid, or the decoded request
    /// fails basic Lightning-specific validation.
    pub fn lightning_charge_request(&self) -> Result<MppLightningChargeRequest, L402Error> {
        if self.method != "lightning" {
            return Err(L402Error::InvalidChallenge {
                reason: format!("unsupported MPP payment method: {}", self.method),
            });
        }

        if self.intent != "charge" {
            return Err(L402Error::InvalidChallenge {
                reason: format!("unsupported MPP payment intent: {}", self.intent),
            });
        }

        let request: MppLightningChargeRequest =
            decode_base64url_json(&self.request).map_err(|reason| L402Error::InvalidChallenge {
                reason: format!("invalid Lightning charge request: {reason}"),
            })?;

        request
            .validate()
            .map_err(|reason| L402Error::InvalidChallenge {
                reason: format!("invalid Lightning charge request: {reason}"),
            })?;

        Ok(request)
    }
}

/// Decoded request object for the MPP Lightning `charge` method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MppLightningChargeRequest {
    /// Invoice amount in satoshis, encoded as a decimal string.
    pub amount: String,

    /// Optional currency code. If present, it must be `BTC`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,

    /// Optional human-readable memo or purchase description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Lightning-specific payment details.
    #[serde(rename = "methodDetails")]
    pub method_details: MppLightningMethodDetails,
}

impl MppLightningChargeRequest {
    /// Validate the decoded Lightning request.
    fn validate(&self) -> Result<(), String> {
        if self.amount.is_empty() {
            return Err("missing amount".to_string());
        }

        self.amount.parse::<u64>().map_err(|_| {
            format!(
                "amount must be an unsigned integer string, got {}",
                self.amount
            )
        })?;

        if let Some(currency) = &self.currency
            && !currency.eq_ignore_ascii_case("BTC")
        {
            return Err(format!(
                "currency must be 'BTC' when present, got {currency}"
            ));
        }

        validate_invoice_prefix(&self.method_details.invoice)?;

        if let Some(network) = &self.method_details.network {
            let expected_prefix = network_invoice_prefix(network)?;
            let invoice_lower = self.method_details.invoice.to_ascii_lowercase();
            if !invoice_lower.starts_with(expected_prefix) {
                return Err(format!(
                    "invoice prefix does not match network {network}: expected {expected_prefix}"
                ));
            }
        }

        if let Some(payment_hash) = &self.method_details.payment_hash {
            validate_hex_32(payment_hash, "paymentHash")?;
        }

        Ok(())
    }
}

/// Lightning-specific MPP request details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MppLightningMethodDetails {
    /// Full BOLT11 invoice.
    pub invoice: String,

    /// Optional lowercase hex payment hash convenience field.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "paymentHash"
    )]
    pub payment_hash: Option<String>,

    /// Optional network hint, one of `mainnet`, `regtest`, or `signet`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
}

/// A client-submitted MPP credential for the `Authorization` header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MppCredential {
    /// The challenge being answered.
    pub challenge: MppChallenge,

    /// Optional payer identity, included by some payment methods.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// Method-specific proof payload.
    pub payload: MppCredentialPayload,
}

impl MppCredential {
    /// Create a Lightning-charge credential from a challenge and payment
    /// preimage.
    pub fn new(challenge: MppChallenge, preimage: String) -> Self {
        Self {
            challenge,
            source: None,
            payload: MppCredentialPayload { preimage },
        }
    }

    /// Format the credential as an `Authorization` header value.
    ///
    /// # Errors
    ///
    /// Returns [`L402Error::InvalidToken`] if the credential cannot be
    /// serialized to JSON.
    pub fn to_header_value(&self) -> Result<String, L402Error> {
        let encoded = encode_base64url_json(self).map_err(|reason| L402Error::InvalidToken {
            reason: format!("failed to serialize MPP credential: {reason}"),
        })?;

        Ok(format!("Payment {encoded}"))
    }

    /// Parse an MPP credential from an `Authorization` header value.
    ///
    /// # Errors
    ///
    /// Returns [`L402Error::InvalidToken`] if the scheme is wrong, the payload
    /// is not valid base64url JSON, or the Lightning preimage is malformed.
    pub fn from_header(header: &str) -> Result<Self, L402Error> {
        let encoded =
            header
                .trim()
                .strip_prefix("Payment ")
                .ok_or_else(|| L402Error::InvalidToken {
                    reason: "authorization header must start with 'Payment'".to_string(),
                })?;

        let credential: Self =
            decode_base64url_json(encoded).map_err(|reason| L402Error::InvalidToken {
                reason: format!("invalid MPP credential JSON: {reason}"),
            })?;

        validate_hex_32(&credential.payload.preimage, "preimage").map_err(|reason| {
            L402Error::InvalidToken {
                reason: format!("invalid MPP credential payload: {reason}"),
            }
        })?;

        Ok(credential)
    }
}

/// Lightning charge credential payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MppCredentialPayload {
    /// 32-byte payment preimage, lowercase hex.
    pub preimage: String,
}

/// Parsed `Payment-Receipt` header payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MppReceipt {
    /// Challenge identifier this receipt acknowledges.
    #[serde(rename = "challengeId")]
    pub challenge_id: String,

    /// Payment method that settled the challenge.
    pub method: String,

    /// Method-specific settlement reference.
    pub reference: String,

    /// Optional settlement amount and currency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement: Option<MppSettlement>,

    /// Outcome status, typically `success`.
    pub status: String,

    /// RFC 3339 / ISO 8601 processing timestamp.
    pub timestamp: String,
}

impl MppReceipt {
    /// Parse an MPP receipt from a `Payment-Receipt` header value.
    ///
    /// # Errors
    ///
    /// Returns [`L402Error::InvalidToken`] if the receipt is not valid
    /// base64url JSON.
    pub fn from_header(header: &str) -> Result<Self, L402Error> {
        decode_base64url_json(header).map_err(|reason| L402Error::InvalidToken {
            reason: format!("invalid MPP receipt JSON: {reason}"),
        })
    }
}

/// Settlement details from an MPP receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MppSettlement {
    /// Settled amount in the method's base unit.
    pub amount: String,

    /// Settled currency code.
    pub currency: String,
}

fn required_field(value: Option<String>, name: &str) -> Result<String, L402Error> {
    value.ok_or_else(|| L402Error::InvalidChallenge {
        reason: format!("missing '{name}' parameter"),
    })
}

fn validate_invoice_prefix(invoice: &str) -> Result<(), String> {
    let invoice_lower = invoice.to_ascii_lowercase();
    if invoice_lower.starts_with("lnbc")
        || invoice_lower.starts_with("lntb")
        || invoice_lower.starts_with("lntbs")
        || invoice_lower.starts_with("lnbcrt")
    {
        Ok(())
    } else {
        Err(format!(
            "invoice must start with 'lnbc', 'lntb', 'lntbs', or 'lnbcrt', got: {}",
            invoice.chars().take(10).collect::<String>()
        ))
    }
}

fn network_invoice_prefix(network: &str) -> Result<&'static str, String> {
    match network {
        "mainnet" => Ok("lnbc"),
        "regtest" => Ok("lnbcrt"),
        "signet" => Ok("lntbs"),
        _ => Err(format!(
            "network must be one of mainnet, regtest, or signet, got {network}"
        )),
    }
}

fn validate_hex_32(value: &str, field: &str) -> Result<(), String> {
    if value.len() != 64 {
        return Err(format!("{field} must be 64 lowercase hex characters"));
    }

    let decoded = hex::decode(value)
        .map_err(|_| format!("{field} must be valid lowercase hex, got {value}"))?;

    if decoded.len() != 32 {
        return Err(format!("{field} must decode to 32 bytes"));
    }

    if value.chars().any(|ch| ch.is_ascii_uppercase()) {
        return Err(format!("{field} must use lowercase hex"));
    }

    Ok(())
}

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

fn parse_kv(param: &str) -> Result<(String, String), String> {
    let (key, rest) = param
        .split_once('=')
        .ok_or_else(|| format!("expected key=value pair, got: {param}"))?;

    Ok((
        key.trim().to_lowercase(),
        rest.trim().trim_matches('"').to_string(),
    ))
}

fn decode_base64url_json<T>(input: &str) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    let bytes = BASE64_URL_SAFE
        .decode(input)
        .map_err(|err| format!("base64url decode failed: {err}"))?;

    serde_json::from_slice(&bytes).map_err(|err| format!("JSON decode failed: {err}"))
}

fn encode_base64url_json<T>(value: &T) -> Result<String, String>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec(value).map_err(|err| err.to_string())?;
    Ok(BASE64_URL_SAFE.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request() -> String {
        encode_base64url_json(&serde_json::json!({
            "amount": "100",
            "currency": "BTC",
            "description": "Premium API access",
            "methodDetails": {
                "invoice": "lnbc100n1pjtest",
                "paymentHash": "0000000000000000000000000000000000000000000000000000000000000000",
                "network": "mainnet"
            }
        }))
        .unwrap()
    }

    fn sample_challenge() -> MppChallenge {
        MppChallenge {
            id: "qB3wErTyU7iOpAsD9fGhJk".to_string(),
            realm: "mpp.dev".to_string(),
            method: "lightning".to_string(),
            intent: "charge".to_string(),
            request: sample_request(),
            expires: Some("2025-01-15T12:05:00Z".to_string()),
            description: Some("Premium API access".to_string()),
        }
    }

    #[test]
    fn parse_valid_payment_challenge() {
        let header = format!(
            "Payment id=\"qB3wErTyU7iOpAsD9fGhJk\", realm=\"mpp.dev\", method=\"lightning\", intent=\"charge\", expires=\"2025-01-15T12:05:00Z\", request=\"{}\", description=\"Premium API access\"",
            sample_request()
        );
        let challenge = MppChallenge::from_header(&header).unwrap();

        assert_eq!(challenge.id, "qB3wErTyU7iOpAsD9fGhJk");
        assert_eq!(challenge.method, "lightning");
        assert_eq!(challenge.intent, "charge");
        assert_eq!(challenge.realm, "mpp.dev");
        assert_eq!(challenge.expires.as_deref(), Some("2025-01-15T12:05:00Z"));
        assert_eq!(challenge.description.as_deref(), Some("Premium API access"));
    }

    #[test]
    fn reject_non_payment_scheme() {
        let err = MppChallenge::from_header("L402 macaroon=abc").unwrap_err();
        assert!(matches!(err, L402Error::InvalidChallenge { .. }));
    }

    #[test]
    fn reject_missing_required_field() {
        let header = r#"Payment id="abc", realm="mpp.dev", method="lightning", intent="charge""#;
        let err = MppChallenge::from_header(header).unwrap_err();
        assert!(matches!(err, L402Error::InvalidChallenge { .. }));
    }

    #[test]
    fn reject_invalid_request_json() {
        let header = r#"Payment id="abc", realm="mpp.dev", method="lightning", intent="charge", request="bm90LWpzb24""#;
        let err = MppChallenge::from_header(header).unwrap_err();
        assert!(matches!(err, L402Error::InvalidChallenge { .. }));
    }

    #[test]
    fn decode_lightning_charge_request() {
        let request = sample_challenge().lightning_charge_request().unwrap();

        assert_eq!(request.amount, "100");
        assert_eq!(request.currency.as_deref(), Some("BTC"));
        assert_eq!(request.method_details.invoice, "lnbc100n1pjtest");
        assert_eq!(
            request.method_details.payment_hash.as_deref(),
            Some("0000000000000000000000000000000000000000000000000000000000000000")
        );
    }

    #[test]
    fn reject_unsupported_method_for_lightning_request() {
        let mut challenge = sample_challenge();
        challenge.method = "tempo".to_string();

        let err = challenge.lightning_charge_request().unwrap_err();
        assert!(matches!(err, L402Error::InvalidChallenge { .. }));
    }

    #[test]
    fn reject_unsupported_intent_for_lightning_request() {
        let mut challenge = sample_challenge();
        challenge.intent = "session".to_string();

        let err = challenge.lightning_charge_request().unwrap_err();
        assert!(matches!(err, L402Error::InvalidChallenge { .. }));
    }

    #[test]
    fn reject_invalid_lightning_currency() {
        let challenge = MppChallenge {
            request: encode_base64url_json(&serde_json::json!({
                "amount": "100",
                "currency": "usd",
                "methodDetails": { "invoice": "lnbc100n1pjtest" }
            }))
            .unwrap(),
            ..sample_challenge()
        };

        let err = challenge.lightning_charge_request().unwrap_err();
        assert!(matches!(err, L402Error::InvalidChallenge { .. }));
    }

    #[test]
    fn reject_lightning_network_mismatch() {
        let challenge = MppChallenge {
            request: encode_base64url_json(&serde_json::json!({
                "amount": "100",
                "methodDetails": {
                    "invoice": "lnbc100n1pjtest",
                    "network": "regtest"
                }
            }))
            .unwrap(),
            ..sample_challenge()
        };

        let err = challenge.lightning_charge_request().unwrap_err();
        assert!(matches!(err, L402Error::InvalidChallenge { .. }));
    }

    #[test]
    fn roundtrip_mpp_credential() {
        let credential = MppCredential::new(
            sample_challenge(),
            "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        );

        let header = credential.to_header_value().unwrap();
        let parsed = MppCredential::from_header(&header).unwrap();

        assert_eq!(parsed, credential);
    }

    #[test]
    fn reject_invalid_mpp_credential_preimage() {
        let encoded = "eyJjaGFsbGVuZ2UiOnsiaWQiOiJhYmMiLCJyZWFsbSI6Im1wcC5kZXYiLCJtZXRob2QiOiJsaWdodG5pbmciLCJpbnRlbnQiOiJjaGFyZ2UiLCJyZXF1ZXN0IjoiZXlKaGJXOTFiblFpT2lJeE1EQWlMQ0p0WlhSb2IyUkVaWFJoYVd4eklqcDdJbWx1ZG05cFkyVWlPaUpzYm1Kak1UQXdibkZxZEdWemRDSjlmUSJ9LCJwYXlsb2FkIjp7InByZWltYWdlIjoibm90LWhleCJ9fQ";
        let err = MppCredential::from_header(&format!("Payment {encoded}")).unwrap_err();
        assert!(matches!(err, L402Error::InvalidToken { .. }));
    }

    #[test]
    fn parse_payment_receipt() {
        let header = "eyJjaGFsbGVuZ2VJZCI6InFCM3dFclR5VTdpT3BBc0Q5ZkdoSmsiLCJtZXRob2QiOiJsaWdodG5pbmciLCJyZWZlcmVuY2UiOiIwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMCIsInNldHRsZW1lbnQiOnsiYW1vdW50IjoiMTAwIiwiY3VycmVuY3kiOiJCVEMifSwic3RhdHVzIjoic3VjY2VzcyIsInRpbWVzdGFtcCI6IjIwMjUtMDEtMTVUMTI6MDA6MDBaIn0";
        let receipt = MppReceipt::from_header(header).unwrap();

        assert_eq!(receipt.challenge_id, "qB3wErTyU7iOpAsD9fGhJk");
        assert_eq!(receipt.method, "lightning");
        assert_eq!(receipt.status, "success");
        assert_eq!(receipt.settlement.unwrap().currency, "BTC");
    }
}
