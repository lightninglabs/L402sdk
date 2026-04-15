use crate::L402Error;

/// An L402 authorization token.
///
/// Combines a macaroon (proof of payment authorization) with a preimage
/// (proof of payment) to form a complete L402 credential.
///
/// ## Authorization Header Format
///
/// ```text
/// Authorization: L402 <base64-macaroon>:<hex-preimage>
/// ```
#[derive(Debug, Clone)]
pub struct L402Token {
    /// Base64-encoded macaroon (received from the 402 challenge).
    pub macaroon: String,

    /// Hex-encoded preimage (obtained by paying the Lightning invoice).
    pub preimage: String,
}

impl L402Token {
    /// Create a new L402 token from a macaroon and preimage.
    ///
    /// # Arguments
    ///
    /// * `macaroon` - Base64-encoded macaroon from the L402 challenge
    /// * `preimage` - Hex-encoded payment preimage from paying the invoice
    pub fn new(macaroon: String, preimage: String) -> Self {
        Self { macaroon, preimage }
    }

    /// Format the token as an `Authorization` header value.
    ///
    /// Returns a string in the format: `L402 <macaroon>:<preimage>`
    pub fn to_header_value(&self) -> String {
        format!("L402 {}:{}", self.macaroon, self.preimage)
    }

    /// Parse an L402 token from an `Authorization` header value.
    ///
    /// # Errors
    ///
    /// Returns [`L402Error::InvalidToken`] if the header format is invalid.
    pub fn from_header(header: &str) -> Result<Self, L402Error> {
        let header = header.trim();

        let credentials = if let Some(rest) = header.strip_prefix("L402 ") {
            rest
        } else if let Some(rest) = header.strip_prefix("LSAT ") {
            rest
        } else {
            return Err(L402Error::InvalidToken {
                reason: "authorization header must start with 'L402' or 'LSAT'".to_string(),
            });
        };

        let (macaroon, preimage) =
            credentials
                .split_once(':')
                .ok_or_else(|| L402Error::InvalidToken {
                    reason: "expected format: L402 <macaroon>:<preimage>".to_string(),
                })?;

        if macaroon.is_empty() {
            return Err(L402Error::InvalidToken {
                reason: "macaroon part is empty".to_string(),
            });
        }

        if preimage.is_empty() {
            return Err(L402Error::InvalidToken {
                reason: "preimage part is empty".to_string(),
            });
        }

        Ok(Self {
            macaroon: macaroon.to_string(),
            preimage: preimage.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_token() {
        let token = L402Token::new("YWJjZGVm".to_string(), "abcdef1234".to_string());
        let header = token.to_header_value();
        assert_eq!(header, "L402 YWJjZGVm:abcdef1234");

        let parsed = L402Token::from_header(&header).unwrap();
        assert_eq!(parsed.macaroon, "YWJjZGVm");
        assert_eq!(parsed.preimage, "abcdef1234");
    }

    #[test]
    fn parse_lsat_token() {
        let header = "LSAT YWJjZGVm:abcdef1234";
        let token = L402Token::from_header(header).unwrap();
        assert_eq!(token.macaroon, "YWJjZGVm");
    }

    #[test]
    fn reject_invalid_format() {
        assert!(L402Token::from_header("Bearer token").is_err());
        assert!(L402Token::from_header("L402 no-colon-here").is_err());
        assert!(L402Token::from_header("L402 :preimage").is_err());
        assert!(L402Token::from_header("L402 macaroon:").is_err());
    }
}
