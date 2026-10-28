#!/usr/bin/env bash
# Refresh the vendored schemas and their protoc-free client stubs together.
set -euo pipefail
cd "$(dirname "$0")/.."
LND_VERSION=${LND_VERSION:-v0.21.5-beta}
CLN_VERSION=${CLN_VERSION:-v26.06.9}
command -v protoc >/dev/null
curl -fsSL "https://raw.githubusercontent.com/lightningnetwork/lnd/${LND_VERSION}/lnrpc/lightning.proto" -o crates/l402-lnd/proto/lightning.proto
curl -fsSL "https://raw.githubusercontent.com/lightningnetwork/lnd/${LND_VERSION}/lnrpc/routerrpc/router.proto" -o crates/l402-lnd/proto/router.proto
curl -fsSL "https://raw.githubusercontent.com/ElementsProject/lightning/${CLN_VERSION}/cln-grpc/proto/node.proto" -o crates/l402-cln/proto/node.proto
curl -fsSL "https://raw.githubusercontent.com/ElementsProject/lightning/${CLN_VERSION}/cln-grpc/proto/primitives.proto" -o crates/l402-cln/proto/primitives.proto
# Normalize upstream mixed indentation without changing schema contents.
python3 - <<'PYTHON'
from pathlib import Path
p = Path("crates/l402-cln/proto/primitives.proto")
p.write_text(p.read_text().replace("  \toneof kind", "\toneof kind"))
PYTHON
L402_UPDATE_PROTO_STUBS=1 cargo check -p l402-lnd -p l402-cln --all-features
