# Dependency refresh and upstream compatibility

Date: 2026-10-28. Tracking: [L402sdk #25](https://github.com/lightninglabs/L402sdk/issues/25).

## Problem and approach

The existing dependency ranges select vulnerable PyO3 0.28 releases and an older
Nostr dependency tree containing an unsound LRU implementation and unmaintained
crates. Several CI and regtest dependencies are also behind current releases.
Upgrade the supported releases, compile every adapter feature, and test real
payments through the reference server. Dependency scans are evidence about known
advisories, not proof that the library is free of vulnerabilities.

## Compatibility decisions

- Rust: the latest rusqlite requires Rust 1.89; that becomes the workspace MSRV.
  PyO3 moves to 0.30.1, NWC to 0.45.1, and tonic/prost to 0.14. The tonic protobuf
  codec and generator now live in tonic-prost and tonic-prost-build. Both
  vendored schemas and checked-in fallback clients must be regenerated together
  with `scripts/update-protos.sh`. A present but failing protoc is an error.
- CLN gRPC uses Xpay, including its absolute maxfee parameter, because Pay is
  deprecated in the current schemas. Xpay errors are RPC errors; successful
  responses contain the preimage and paid/sent amounts. The receipt payment hash
  is derived from the preimage. The integration baseline is CLN v26.06.9.
- SwissKnife 0.3 requires an explicit account-owned wallet selector and nests
  Lightning payment proofs under `lightning`. Rust's `with_wallet`, Python's
  optional `wallet_id`, and WASM's `withSwissKnifeWallet` expose that selector.
  Existing constructors remain available for servers with the old API.
- LangChain now requires Python 3.10+ and langchain-core 1.x. The base Python
  binding retains its existing Python floor. CI exercises current Python 3.15.
- TypeScript uses AI SDK 7, Zod 4, Vitest 5 and Vite 8. The demos use current
  Next.js 16.4.3 and React 19.3.1. TypeScript 6.0.3 is the newest version supported
  by the demos' typescript-eslint dependency; other TS packages use 7.0.2.
  ESLint 10 has stale peer declarations in Next's plugin tree; both demos pass
  lint and production builds despite that peer metadata.
- The npm resolutions select patched Babel 7, humanfs, structured-clone and
  selector-parser releases where upstream manifests pin affected versions.
  rehype-katex still requests KaTeX 0.16, so the demo resolves KaTeX 0.19.1 and
  checks rendering and rejection of untrusted JavaScript links.

## Reproducibility and checks

All regtest images have explicit release tags and immutable registry digests:
Bitcoin 32.1, LND v0.21.5-beta, CLN v26.06.9, SwissKnife 0.3.0, Node 26 Alpine,
and Aperture v0.5.0. The newest Node image was verified independently of the
upstream Node release; an unavailable patch tag is not invented. Aperture's
published image requires amd64 emulation on ARM hosts. Ports bind to loopback.

CI actions are pinned to reviewed release commit IDs. wasm-pack's release archive
is checksum-verified. CI scans Cargo, Python and npm dependencies and reruns weekly;
Dependabot covers package ecosystems, actions and Compose images. Cargo.lock stays
untracked according to repository policy; fresh CI resolution is audited.

`make regtest-test` sets `L402_REQUIRE_REGTEST=1`: missing services, authentication,
funding or credentials fail the integration run instead of silently skipping it.
The health service explicitly uses `auth: "off"`; `price: 0` alone still creates
a paid challenge in both the v0.5.0 image and the tested master revision.
CLN startup waits for the actual Bitcoin block height before spending its funding
outputs. Python and WASM callers use the selected SwissKnife wallet too.

## L402 and Aperture upstream

The [L402 specification at 592203a](https://github.com/lightninglabs/L402/tree/592203a95ea297fad6d7e10c6de2d827bfd450b2)
still uses `macaroon=` and accepts both LSAT and L402 authentication schemes.
It has no binary dependency or newer release number to install.
[Proposal #21](https://github.com/lightninglabs/L402/pull/21) adds `token=` and
`version="0"` and remains open. Discovery and wallet requirements are also proposals.
Do not silently treat proposed wire formats as the current standard.

[Aperture v0.5.0](https://github.com/lightninglabs/aperture/releases/tag/v0.5.0)
is still the newest tagged release. Master at f6d91e7 contains post-release
resource binding, path validation, authentication cleanup, MPP fixes and metering.
An image bump cannot pick these up until a new release exists. Release tracking is
[Aperture #311](https://github.com/lightninglabs/aperture/issues/311).

Before adopting master or the next release:

1. Review unresolved upstream issues, including TLS identity validation (#265)
   and MPP receipt verification (#272); latest does not mean all findings are fixed.
2. Plan for old paid MPP challenges to be invalidated by resource binding.
3. Check service names and `auth` values against the stricter validation rules,
   and review public-path credential stripping and path normalization.
4. Back up the database and review migrations. MPP charge/session state requires
   SQLite or Postgres rather than etcd.
5. Keep MPP opt-in. SDK charge execution and sessions remain separate feature work
   under [#20](https://github.com/lightninglabs/L402sdk/issues/20) and
   [#22](https://github.com/lightninglabs/L402sdk/pull/22); a server upgrade does not
   implement them in this client.

## Local verification

On macOS ARM64, all workspace Rust tests and doctests, Clippy with every adapter
feature, the Rust 1.89 build, REST-only builds, and protoc-free fallback builds
passed. The funded regtest stack passed 33 Rust payment tests, 35 Python payment
tests, and 5 WASM payment tests. Python plus LangChain passed 76 tests overall;
WASM Node passed 30 tests and headless Chrome passed 8. Go FFI tests, AI SDK tests,
example typechecking, both demo builds and lints, and math rendering checks passed.
Cargo, Python and all five npm dependency trees have no reported advisories at
this snapshot. Hosted CI separately checks Linux/macOS and Python 3.9/3.15.

The WASM regtest runner trusts only the generated test CA via
`NODE_EXTRA_CA_CERTS`; global TLS verification is no longer disabled.

The Aperture master compatibility probe is recorded in
[Aperture #311](https://github.com/lightninglabs/aperture/issues/311): exact source
f6d91e78f22b445fea8e112f30172e5ea8b6cff3 passed ordinary L402 payments across Rust,
Python and WASM after the explicit health-route configuration. This does not test
MPP/session migration or upgrade a production database.
