# Invoice and wallet transport boundaries

Tracking: [#27](https://github.com/lightninglabs/L402sdk/issues/27).

Automatic L402 payments require an explicit, positive invoice amount that fits
in millisatoshi accounting. The BOLT11 amount parser accepts decimal integers
and the standard multipliers, rejects fractional/exponent/signed input, and uses
checked arithmetic. Full signature/checksum verification remains the Lightning
backend's responsibility. An amount parsing error or amountless invoice must
return before a backend payment or budget reservation. Budget counter updates
are checked and atomic even at the numeric boundary.

Native LND and CLN REST constructors verify server certificates and do not follow
HTTP redirects carrying wallet bearer credentials. Existing constructor signatures
remain available; connections that relied on accepting arbitrary certificates now
fail. Use a valid server certificate with the endpoint's hostname in its SAN.
Self-signed deployments can pass their trusted PEM CA explicitly:

```rust,ignore
let backend = LndRestBackend::with_ca_pem(url, macaroon, &ca_bytes)?;
```

`with_ca_pem` trusts only the supplied CA and retains chain and hostname checks.
`from_env` also accepts `LND_REST_CA_CERT_PATH` or `CLN_REST_CA_CERT_PATH`.
Python backend constructors and `L402Client.with_lnd_rest` / `with_cln_rest` accept
an optional `ca_pem` string. For example:

```python
client = L402Client.with_lnd_rest(url, macaroon, ca_pem=Path("ca.pem").read_text())
```

WASM uses the browser or Node trust configuration; it cannot install native
certificates or override Fetch's redirect policy through these constructors.
Custom Rust HTTP clients remain an explicit caller responsibility, including
their TLS trust and redirect policy. Native defaults do not change that escape
hatch. Debug invoice prefixes are sliced on character boundaries to avoid UTF-8
panics when adapters receive malformed input directly.

Tests exercise rejected invoice challenges with no payment call, conversion and
counter boundaries, real TLS connections with trusted and untrusted CAs, hostname
mismatch, and refusal to follow credential-bearing redirects. The funded regtest
suite and Python bindings pass with explicit trust rather than TLS bypasses.

This work does not complete a security audit. Fee-cap capability and fee-inclusive
reservation/settlement behavior remain explicitly tracked in
[#28](https://github.com/lightninglabs/L402sdk/issues/28). That issue also covers
redirect/domain policy and durable payment receipts after downstream failures.
