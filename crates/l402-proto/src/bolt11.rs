//! BOLT11 invoice amount decoding.
//!
//! Provides lightweight extraction of the payment amount from a BOLT11 invoice
//! string without full invoice validation. This is sufficient for budget
//! enforcement — full validation is handled by the Lightning backend.
//!
//! # BOLT11 Human-Readable Part
//!
//! A BOLT11 invoice has the format: `ln<network><amount><multiplier>1<data><checksum>`
//!
//! - Network: `bc` (mainnet), `tb` (testnet), `bcrt` (regtest)
//! - Amount: positive decimal number (optional — omitted for zero-amount invoices)
//! - Multiplier: `m` (10⁻³), `u` (10⁻⁶), `n` (10⁻⁹), `p` (10⁻¹²)
//!
//! The `1` separator divides the human-readable part from the data part.
//!
//! # Example
//!
//! ```rust
//! use l402_proto::bolt11::decode_bolt11_amount;
//!
//! // 2500 microbitcoin = 250,000 satoshis
//! let amount = decode_bolt11_amount("lnbc2500u1pjtest").unwrap().unwrap();
//! assert_eq!(amount.satoshis(), 250_000);
//!
//! // 100 nanobitcoin = 10 satoshis
//! let amount = decode_bolt11_amount("lnbc100n1pjtest").unwrap().unwrap();
//! assert_eq!(amount.millisatoshis, 10_000);
//! assert_eq!(amount.satoshis(), 10);
//!
//! // Zero-amount invoice
//! let amount = decode_bolt11_amount("lnbc1pjtest").unwrap();
//! assert!(amount.is_none());
//! ```

use crate::L402Error;

/// Amount decoded from a BOLT11 invoice, stored in millisatoshis for precision.
///
/// BOLT11 invoices can express sub-satoshi amounts using the `n` (nano) and `p`
/// (pico) multipliers. Millisatoshis (1 sat = 1000 msat) preserve this precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvoiceAmount {
    /// Amount in millisatoshis (1 satoshi = 1,000 millisatoshis).
    pub millisatoshis: u64,
}

impl InvoiceAmount {
    /// Create a new [`InvoiceAmount`] from millisatoshis.
    pub fn from_millisatoshis(millisatoshis: u64) -> Self {
        Self { millisatoshis }
    }

    /// Create a new [`InvoiceAmount`] from satoshis.
    pub fn from_satoshis(satoshis: u64) -> Self {
        Self {
            millisatoshis: satoshis * 1_000,
        }
    }

    /// Convert to satoshis, rounding up (ceil).
    ///
    /// This is the conservative choice for budget enforcement — if an invoice
    /// is for 1.5 sats, we report 2 sats.
    pub fn satoshis(&self) -> u64 {
        self.millisatoshis.div_ceil(1_000)
    }

    /// Convert to satoshis, rounding down (floor).
    pub fn satoshis_floor(&self) -> u64 {
        self.millisatoshis / 1_000
    }
}

/// Multiplier suffixes for BOLT11 amounts.
///
/// Each multiplier specifies the amount as a fraction of 1 BTC.
/// We store the conversion factor to millisatoshis.
///
/// 1 BTC = 100,000,000 sats = 100,000,000,000 msat
const MULTIPLIERS: &[(char, u64)] = &[
    ('m', 100_000_000), // milli:  0.001 BTC = 100,000 sat = 100,000,000 msat
    ('u', 100_000),     // micro:  0.000001 BTC = 100 sat = 100,000 msat
    ('n', 100),         // nano:   10⁻⁹ BTC = 0.1 sat = 100 msat
    ('p', 0),           // pico:   10⁻¹² BTC = 0.0001 sat = 0.1 msat (special-cased)
];

/// Conversion factor from whole BTC to millisatoshis.
const BTC_TO_MSAT: u64 = 100_000_000_000; // 1 BTC = 10^11 msat

/// Decode the amount from a BOLT11 invoice string.
///
/// Returns `Ok(Some(amount))` if the invoice contains an amount,
/// `Ok(None)` for zero-amount invoices (no amount specified),
/// or `Err` if the invoice format is invalid.
///
/// # Errors
///
/// Returns [`L402Error::InvalidInvoice`] if:
/// - The string doesn't start with `lnbc`, `lntb`, or `lnbcrt`
/// - The string has no `1` separator between human-readable and data parts
/// - The amount string is not a valid number
/// - The `p` multiplier is used with a non-multiple-of-10 amount (BOLT11 spec requirement)
pub fn decode_bolt11_amount(invoice: &str) -> Result<Option<InvoiceAmount>, L402Error> {
    let invoice_lower = invoice.to_lowercase();

    // Strip the network prefix to find the amount portion
    let after_network = if let Some(rest) = invoice_lower.strip_prefix("lnbcrt") {
        rest
    } else if let Some(rest) = invoice_lower.strip_prefix("lnbc") {
        rest
    } else if let Some(rest) = invoice_lower.strip_prefix("lntbs") {
        rest
    } else if let Some(rest) = invoice_lower.strip_prefix("lntb") {
        rest
    } else {
        return Err(L402Error::InvalidInvoice {
            reason: "invoice must start with 'lnbc', 'lntb', 'lnbcrt', or 'lntbs'".to_string(),
        });
    };

    // Find the `1` separator between human-readable and data parts.
    // Per BOLT11 spec, the last `1` in the string is the separator.
    let separator_pos = after_network.rfind('1').ok_or(L402Error::InvalidInvoice {
        reason: "missing '1' separator between human-readable and data parts".to_string(),
    })?;

    let amount_str = &after_network[..separator_pos];

    // Empty amount = zero-amount invoice
    if amount_str.is_empty() {
        return Ok(None);
    }

    // Check for multiplier suffix
    let last_char = amount_str
        .chars()
        .next_back()
        .expect("amount_str is non-empty");

    let (number_str, multiplier_msat) =
        if let Some(&(_, factor)) = MULTIPLIERS.iter().find(|&&(c, _)| c == last_char) {
            (
                &amount_str[..amount_str.len() - 1],
                Some((last_char, factor)),
            )
        } else if last_char.is_ascii_digit() {
            // No multiplier — amount is in whole BTC
            (amount_str, None)
        } else {
            return Err(L402Error::InvalidInvoice {
                reason: format!("unknown amount multiplier: '{last_char}'"),
            });
        };

    if number_str.is_empty() {
        return Err(L402Error::InvalidInvoice {
            reason: "amount multiplier present but no numeric value".to_string(),
        });
    }

    // Parse the numeric part (may be integer or decimal)
    let millisatoshis = if let Some((_, factor)) = multiplier_msat {
        parse_amount_with_multiplier(number_str, last_char, factor)?
    } else {
        parse_amount_btc(number_str)?
    };

    if millisatoshis == 0 {
        return Err(L402Error::InvalidInvoice {
            reason: "invoice amount must be positive (got 0)".to_string(),
        });
    }

    Ok(Some(InvoiceAmount { millisatoshis }))
}

/// Parse amount with a multiplier suffix.
///
/// The BOLT11 spec says: the amount is a positive decimal integer optionally
/// followed by a decimal point and more digits.
fn parse_amount_with_multiplier(
    number_str: &str,
    multiplier: char,
    factor: u64,
) -> Result<u64, L402Error> {
    // Handle picobitcoin specially since 1p = 0.1 msat
    if multiplier == 'p' {
        return parse_pico_amount(number_str);
    }

    if number_str.contains('.') {
        // Decimal amount: e.g., "2.5m" = 2.5 * 100,000,000 msat
        let value: f64 = number_str.parse().map_err(|_| L402Error::InvalidInvoice {
            reason: format!("invalid amount number: '{number_str}'"),
        })?;
        // Use f64 math and round to avoid precision issues
        #[allow(
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation,
            clippy::cast_precision_loss
        )]
        let msat = (value * factor as f64).round() as u64;
        Ok(msat)
    } else {
        let value: u64 = number_str.parse().map_err(|_| L402Error::InvalidInvoice {
            reason: format!("invalid amount number: '{number_str}'"),
        })?;
        Ok(value * factor)
    }
}

/// Parse picobitcoin amount.
///
/// 1 picobitcoin = 0.1 millisatoshi. BOLT11 spec requires picobitcoin amounts
/// to be multiples of 10 (so that the result is a whole number of millisatoshis).
fn parse_pico_amount(number_str: &str) -> Result<u64, L402Error> {
    let value: u64 = number_str.parse().map_err(|_| L402Error::InvalidInvoice {
        reason: format!("invalid amount number: '{number_str}'"),
    })?;

    // BOLT11 spec: pico amounts must be multiples of 10
    if value % 10 != 0 {
        return Err(L402Error::InvalidInvoice {
            reason: format!("picobitcoin amount must be a multiple of 10, got {value}"),
        });
    }

    // value picobitcoin = value * 0.1 millisatoshi = value / 10 millisatoshi
    Ok(value / 10)
}

/// Parse a plain BTC amount (no multiplier).
///
/// E.g., "1" = 1 BTC = 100,000,000,000 msat.
fn parse_amount_btc(number_str: &str) -> Result<u64, L402Error> {
    if number_str.contains('.') {
        let value: f64 = number_str.parse().map_err(|_| L402Error::InvalidInvoice {
            reason: format!("invalid amount number: '{number_str}'"),
        })?;
        #[allow(
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation,
            clippy::cast_precision_loss
        )]
        let msat = (value * BTC_TO_MSAT as f64).round() as u64;
        Ok(msat)
    } else {
        let value: u64 = number_str.parse().map_err(|_| L402Error::InvalidInvoice {
            reason: format!("invalid amount number: '{number_str}'"),
        })?;
        Ok(value * BTC_TO_MSAT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Basic multiplier tests ---

    #[test]
    fn decode_milli_btc() {
        // 1m = 0.001 BTC = 100,000 sats
        let amount = decode_bolt11_amount("lnbc1m1pjtest").unwrap().unwrap();
        assert_eq!(amount.millisatoshis, 100_000_000);
        assert_eq!(amount.satoshis(), 100_000);
    }

    #[test]
    fn decode_micro_btc() {
        // 1u = 0.000001 BTC = 100 sats
        let amount = decode_bolt11_amount("lnbc1u1pjtest").unwrap().unwrap();
        assert_eq!(amount.millisatoshis, 100_000);
        assert_eq!(amount.satoshis(), 100);
    }

    #[test]
    fn decode_nano_btc() {
        // 1n = 10^-9 BTC = 0.1 sats = 100 msat
        let amount = decode_bolt11_amount("lnbc1n1pjtest").unwrap().unwrap();
        assert_eq!(amount.millisatoshis, 100);
        assert_eq!(amount.satoshis(), 1); // ceil(0.1) = 1
    }

    #[test]
    fn decode_pico_btc() {
        // 10p = 10 * 10^-12 BTC = 10 * 0.1 msat = 1 msat
        let amount = decode_bolt11_amount("lnbc10p1pjtest").unwrap().unwrap();
        assert_eq!(amount.millisatoshis, 1);
        assert_eq!(amount.satoshis(), 1); // ceil(0.001) = 1
    }

    #[test]
    fn decode_whole_btc() {
        // 1 BTC = 100,000,000 sats
        let amount = decode_bolt11_amount("lnbc11pjtest").unwrap().unwrap();
        assert_eq!(amount.satoshis(), 100_000_000);
    }

    // --- Larger amounts ---

    #[test]
    fn decode_2500_micro() {
        // 2500u = 2500 * 100 sats = 250,000 sats
        let amount = decode_bolt11_amount("lnbc2500u1pjtest").unwrap().unwrap();
        assert_eq!(amount.satoshis(), 250_000);
    }

    #[test]
    fn decode_500_nano() {
        // 500n = 500 * 0.1 sat = 50 sats
        let amount = decode_bolt11_amount("lnbc500n1pjtest").unwrap().unwrap();
        assert_eq!(amount.millisatoshis, 50_000);
        assert_eq!(amount.satoshis(), 50);
    }

    #[test]
    fn decode_20_milli() {
        // 20m = 20 * 100,000 sats = 2,000,000 sats
        let amount = decode_bolt11_amount("lnbc20m1pjtest").unwrap().unwrap();
        assert_eq!(amount.satoshis(), 2_000_000);
    }

    // --- Testnet / regtest ---

    #[test]
    fn decode_testnet_invoice() {
        let amount = decode_bolt11_amount("lntb1500n1pjtest").unwrap().unwrap();
        assert_eq!(amount.satoshis(), 150);
    }

    #[test]
    fn decode_regtest_invoice() {
        let amount = decode_bolt11_amount("lnbcrt1000u1pjtest").unwrap().unwrap();
        assert_eq!(amount.satoshis(), 100_000);
    }

    // --- Zero-amount invoices ---

    #[test]
    fn decode_zero_amount_mainnet() {
        // lnbc1... with no amount between network and separator
        let amount = decode_bolt11_amount("lnbc1pjtest").unwrap();
        assert!(amount.is_none());
    }

    #[test]
    fn decode_zero_amount_testnet() {
        let amount = decode_bolt11_amount("lntb1pjtest").unwrap();
        assert!(amount.is_none());
    }

    // --- Sub-satoshi precision ---

    #[test]
    fn subsatoshi_ceil_rounding() {
        // 1n = 100 msat = 0.1 sat → ceil = 1 sat
        let amount = decode_bolt11_amount("lnbc1n1pjtest").unwrap().unwrap();
        assert_eq!(amount.satoshis(), 1);
        assert_eq!(amount.satoshis_floor(), 0);
    }

    #[test]
    fn exact_satoshi_no_rounding() {
        // 10n = 1000 msat = 1 sat exactly
        let amount = decode_bolt11_amount("lnbc10n1pjtest").unwrap().unwrap();
        assert_eq!(amount.millisatoshis, 1_000);
        assert_eq!(amount.satoshis(), 1);
        assert_eq!(amount.satoshis_floor(), 1);
    }

    // --- Pico edge cases ---

    #[test]
    fn pico_non_multiple_of_ten_rejected() {
        let err = decode_bolt11_amount("lnbc5p1pjtest").unwrap_err();
        assert!(matches!(err, L402Error::InvalidInvoice { .. }));
    }

    #[test]
    fn pico_large_amount() {
        // 10000p = 1000 msat = 1 sat
        let amount = decode_bolt11_amount("lnbc10000p1pjtest").unwrap().unwrap();
        assert_eq!(amount.millisatoshis, 1_000);
        assert_eq!(amount.satoshis(), 1);
    }

    // --- Error cases ---

    #[test]
    fn reject_invalid_prefix() {
        let err = decode_bolt11_amount("lnxy1u1pjtest").unwrap_err();
        assert!(matches!(err, L402Error::InvalidInvoice { .. }));
    }

    #[test]
    fn reject_no_separator() {
        let err = decode_bolt11_amount("lnbc2500u").unwrap_err();
        assert!(matches!(err, L402Error::InvalidInvoice { .. }));
    }

    #[test]
    fn reject_unknown_multiplier() {
        let err = decode_bolt11_amount("lnbc100x1pjtest").unwrap_err();
        assert!(matches!(err, L402Error::InvalidInvoice { .. }));
    }

    #[test]
    fn reject_empty_number_with_multiplier() {
        let err = decode_bolt11_amount("lnbcm1pjtest").unwrap_err();
        assert!(matches!(err, L402Error::InvalidInvoice { .. }));
    }

    // --- InvoiceAmount constructors ---

    #[test]
    fn from_satoshis() {
        let amount = InvoiceAmount::from_satoshis(100);
        assert_eq!(amount.millisatoshis, 100_000);
        assert_eq!(amount.satoshis(), 100);
    }

    #[test]
    fn from_millisatoshis() {
        let amount = InvoiceAmount::from_millisatoshis(1_500);
        assert_eq!(amount.satoshis(), 2); // ceil(1.5)
        assert_eq!(amount.satoshis_floor(), 1);
    }

    // --- Real-world-ish invoice formats ---

    #[test]
    fn realistic_mainnet_invoice() {
        // Typical small L402 payment: 10 sats = 100n
        // Note: bech32 data part doesn't contain '1', so rfind('1') finds the separator
        let amount = decode_bolt11_amount(
            "lnbc100n1pj9nr7mpp5qgk4v0gg3f0e25jqvz5p5x38s9yc7k5v4a2c3w6q7r8t9u0vwxy4z5",
        )
        .unwrap()
        .unwrap();
        assert_eq!(amount.satoshis(), 10);
    }

    #[test]
    fn case_insensitive() {
        let amount = decode_bolt11_amount("LNBC2500U1pjtest").unwrap().unwrap();
        assert_eq!(amount.satoshis(), 250_000);
    }
}
