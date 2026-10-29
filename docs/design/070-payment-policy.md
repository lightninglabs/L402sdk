# Fee limits, budgets, and payment outcomes

Issue: https://github.com/lightninglabs/L402sdk/issues/28

## Guarantees and compatibility

Automatic payments require `LnBackend::supports_fee_limit() == true`. This
additive trait method defaults to false, so custom adapters still compile but
must explicitly implement the enforcement contract. LND and CLN pass absolute
fee caps to their payment RPCs, including an explicit zero cap. The protocol
specifies CLN `maxfee` as overriding percentage and exempt-fee defaults:
https://docs.corelightning.org/reference/pay

NIP-47 and the SwissKnife 0.3 payment API used here do not expose an enforceable
per-payment routing-fee cap. Their `pay_invoice` methods now return
`FeeLimitUnsupported` before submission, including through Python and WASM.
Rust callers deliberately accepting wallet-managed fees can use the separate
`pay_invoice_unbounded` method. That path does not promise SDK fee limits or
budgets and is never selected by `L402Client`. NIP-47 reference:
https://github.com/nostr-protocol/nips/blob/master/47.md

Restoring capped SwissKnife/NWC automatic payments requires a versioned server
capability that enforces the requested limit before settlement, followed by
adapter and end-to-end coverage. Merely sending an unknown JSON field is not
capability negotiation. Existing balance and information queries still work.

## Reservations and concurrency

All budgets now include invoice amount and routing fees. Reserve the invoice
plus the lesser of the configured maximum fee and the remaining budget. Pass
that reduced cap to the backend. Reservations and checks share one lock, so
concurrent payments cannot spend the same remaining allowance. Millisatoshi
amounts and fees each round up; accounting can therefore conservatively exceed
the exact combined cost by less than two sats per payment.

Global limits always apply. A domain budget adds restrictions with independent
domain counters; it cannot replace or relax the global budget. Host matching
uses the URL parser's normalized hostname. Pending reservations count against
current hourly/daily limits even across window boundaries. Confirmed settlements
are recorded in the window when the result is received.

A successful payment replaces its reservation with reported amount plus fee.
A confirmed `PaymentRejected` releases the reservation. Legacy `PaymentFailed`,
transport errors, interrupted futures, and unknown results keep the reservation.
They are not evidence that no money moved. `total_spent()` includes these
outstanding reservations. The same normalized invoice is never submitted again
while its outcome is unresolved. Concurrent callers receive `PaymentUncertain`
instead of initiating another payment. Different invoices can proceed within
the remaining shared allowance.

These protections are local to one client instance and its lifetime. They do
not coordinate multiple clients/processes or survive restarts. Unknown outcomes
must be reconciled with the wallet; there is deliberately no automatic timeout
refund or retry. Restarting a client is not reconciliation. Durable multi-process
payment coordination needs a separate persistence/lookup contract.

## Receipts and post-payment failures

Store the receipt immediately after a successful wallet result, before token
storage or the paid HTTP retry. `receipts()` remains available if persistence,
HTTP transport, token rejection, or cancellation fails afterward. Existing
receipt fields remain compatible: `response_status == 0` means no paid response
was received; otherwise it records the actual HTTP status, including 402/5xx.

Keep the settled result and token in memory so retrying after a token-storage
failure first reuses the paid proof, even if the server would otherwise issue
a fresh invoice. After a successful write the configured token store owns
eviction policy. The normalized invoice also identifies known settlements. Do not
create duplicate receipts for proof reuse. If a backend violates the authorized
amount or fee cap, retain the payment receipt and account for the reported
spending, then freeze further new payments. A post-settlement check cannot undo
a payment or substitute for wallet-side enforcement.

The default native HTTP client disables redirects. Custom native clients must
also disable them. Final-origin checks reject redirected challenges, including
on WASM, where Fetch controls redirect handling. Such checks cannot undo a
credential already sent by a caller-supplied redirect policy; use the default
native client for the SDK's redirect-isolation guarantee.

## Verification

`cargo test --workspace --exclude l402-python --exclude l402-regtest --all-features`
covers fee clipping, zero caps, overspending, atomic reservations, independent
domain limits, window rollover, unknown outcomes, cancellation, deduplication,
backend violations, and receipt preservation. Adapter tests inspect the actual
REST requests and sub-satoshi settlement accounting.

`make regtest-test` exercises funded LND/CLN payments, explicit uncapped
SwissKnife settlement, and automatic rejection in Rust/Python/WASM. CI also
runs a bounded, sanitizer-enabled protocol-parser fuzz check. Its unpublished
helper crate is excluded from the SDK workspace and depends only on the protocol
crate and the fuzzing engine; it adds no dependency to the shipped SDK. Four
small seed inputs exercise valid headers/tokens and invoice amount boundaries:

```
cargo install cargo-fuzz --version 0.13.2 --locked
cargo +nightly-2026-10-28 fuzz run protocol -- -max_total_time=30 -max_len=4096
```

The FFI fixes have deterministic regression tests in `l402-ffi`: last-error
strings remain owned by thread-local state, and an empty null endpoint array
uses an empty Rust slice. Those tests run in the regular Rust suite. There is
no separate FFI fuzz target or fuzz dependency on the FFI/HTTP/mock-server stack.
Fuzz smoke coverage and dependency scans are bounded checks, not a complete
security audit. Independent review is required before merge.
