# 009: BOLT11 Invoice Amount Decoding for Budget Enforcement

**Issue:** #20
**Author:** Dario Anongba Varela
**Date:** 2026-03-17
**Status:** Proposed

## Problem

The `L402Client` calls `budget_tracker.check_and_record(0, domain)` with a hardcoded amount of 0 before paying an invoice. This means all amount-based budget limits (`per_request_max`, `hourly_max`, `daily_max`, `total_max`) are ineffective for pre-payment enforcement. The budget tracker records 0 spent, and the real cost is only captured in the receipt after payment has already happened.

For autonomous AI agents spending real money, this is a critical gap. An agent could blow past all budget limits because the check always passes with amount 0.

## Proposed Design

### 1. Lightweight BOLT11 amount decoder in `bolt402-proto`

Add a `bolt11` module to `bolt402-proto` with a single function that extracts the amount from a BOLT11 invoice string without full invoice validation.

**Why not `lightning-invoice` crate?**
- `bolt402-proto` is the lightweight protocol types crate with minimal dependencies (only `base64`, `thiserror`, `tracing`)
- `lightning-invoice` pulls in `secp256k1`, `bitcoin_hashes`, and other crypto deps
- We only need amount extraction, not signature verification or full parsing
- Full validation is the job of the Lightning backend (LND/CLN), not the client SDK

**BOLT11 human-readable part format:**
```
ln + <network> + <amount><multiplier>
```

Where:
- Network: `bc` (mainnet), `tb` (testnet), `bcrt` (regtest)
- Amount: positive decimal number (integer or float)
- Multiplier: `m` (milli, 10⁻³), `u` (micro, 10⁻⁶), `n` (nano, 10⁻⁹), `p` (pico, 10⁻¹²)
- No multiplier means the amount is in whole BTC

Conversion to satoshis: 1 BTC = 100,000,000 sats

| Multiplier | BTC value | Satoshi value |
|------------|-----------|---------------|
| (none)     | 1         | 100,000,000   |
| m          | 0.001     | 100,000       |
| u          | 0.000001  | 100           |
| n          | 10⁻⁹      | 0.1           |
| p          | 10⁻¹²     | 0.0001        |

Note: `n` and `p` multipliers can result in sub-satoshi amounts. We round up to the nearest satoshi (ceil) for budget enforcement — it's conservative and prevents underestimating costs.

### 2. API

```rust
// bolt402-proto/src/bolt11.rs

/// Amount decoded from a BOLT11 invoice, in millisatoshis.
/// Using millisatoshis preserves sub-satoshi precision from n/p multipliers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvoiceAmount {
    /// Amount in millisatoshis (1 sat = 1000 msat).
    pub millisatoshis: u64,
}

impl InvoiceAmount {
    /// Convert to satoshis, rounding up (ceil).
    pub fn satoshis(&self) -> u64 ...

    /// Convert to satoshis, rounding down (floor).
    pub fn satoshis_floor(&self) -> u64 ...
}

/// Decode the amount from a BOLT11 invoice string.
///
/// Returns `None` for zero-amount invoices (invoices without an amount field).
/// Returns `Err` if the invoice format is invalid.
pub fn decode_bolt11_amount(invoice: &str) -> Result<Option<InvoiceAmount>, L402Error>;
```

### 3. Client integration

In `L402Client::request()`, after parsing the L402 challenge and before paying:

```rust
// Decode the invoice amount for budget enforcement
let invoice_amount = bolt402_proto::bolt11::decode_bolt11_amount(&challenge.invoice)?;
let amount_sats = invoice_amount
    .map(|a| a.satoshis())
    .unwrap_or(0); // Zero-amount invoices: can't enforce amount budget

// Check budget with the real amount (+ max fee as worst case)
let budget_amount = amount_sats.saturating_add(self.config.max_fee_sats);
self.budget_tracker.check_and_record(budget_amount, domain.as_deref()).await?;
```

After payment, we also update the budget tracker with the actual amount if it differs. However, since `check_and_record` already recorded the estimated amount, we need a way to correct it. Two options:

**Option A (chosen):** Record only the decoded amount (without fee) in `check_and_record`, and accept that the fee isn't tracked in the budget. The receipt system captures the exact fee. Budget limits represent "amount paid for services" not "total including routing fees."

**Option B:** Add a `correct_last_record` method to adjust. More complex, and routing fees are generally small relative to the invoice amount.

Going with **Option A** — simpler, and the budget is about service costs, not routing overhead.

### 4. Update mock server

The mock server generates invoices like `lnbc{amount}n1mock...`. This already encodes the amount using the `n` (nano) multiplier convention. We should verify that our decoder handles this correctly, and potentially update the mock to generate more realistic invoice strings.

Looking at the mock: `lnbc{amount_sats}n1mock{hash}` — this means `amount_sats` nanobitcoin, which is NOT the same as `amount_sats` satoshis. We need to fix the mock to generate correct amounts.

For 100 sats: 100 sats = 1u BTC (microbitcoin). So the invoice should be `lnbc1u1mock...`.

Or more simply, we can express in millisatoshis: 100 sats = 100,000 msat. But BOLT11 doesn't have an msat multiplier directly.

The mock should generate: `lnbc{amount_in_correct_unit}{multiplier}1...` where the `1` separator divides the human-readable part from the data part. We'll update the mock to produce correct BOLT11-format amounts.

## Key Decisions

1. **Lightweight decoder over `lightning-invoice` crate** — keeps `bolt402-proto` lean
2. **Millisatoshi internal representation** — preserves precision from `n`/`p` multipliers
3. **Ceil rounding for satoshi conversion** — conservative for budget enforcement
4. **Zero-amount invoices return `None`** — caller decides how to handle (pass 0 to budget)
5. **Budget checks use invoice amount only (no fee)** — simpler, fee is routing overhead
6. **Fix mock server** — generate BOLT11-correct amount encoding

## Alternatives Considered

- **Use `lightning-invoice` crate**: Full validation, but heavy dependency for `bolt402-proto`. Could be added later behind a feature flag if needed.
- **Move decoding to `bolt402-core`**: Breaks the pattern of protocol types living in `proto`.
- **Track fees in budget**: Adds complexity for minimal benefit since routing fees are typically <1% of the payment.

## Testing Plan

1. **Unit tests in `bolt402-proto`** for the amount decoder:
   - Mainnet/testnet/regtest prefixes
   - All multipliers (m, u, n, p, none)
   - Decimal amounts (e.g., `2500u`)
   - Zero-amount invoices
   - Invalid formats (no `ln` prefix, unknown multiplier, etc.)
   - Edge cases: very large amounts, sub-satoshi precision
2. **Unit tests in `bolt402-core`** for client budget integration:
   - Verify `check_and_record` called with decoded amount
   - Verify budget blocks high-amount invoices
   - Verify zero-amount invoices pass budget check
3. **Integration tests** with updated mock server:
   - End-to-end flow with correct invoice amounts
   - Budget exceeded scenario with real amounts
