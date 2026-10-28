#!/usr/bin/env bash
# CI uses the verified Linux release archive, rather than executing a remote installer.
set -euo pipefail
version=0.15.0
archive="wasm-pack-v${version}-x86_64-unknown-linux-musl"
checksum=c09f971ecaed9a2efc80fdcea7a00ef6b53c7fadc8c57d1f61b53a6aa66b668a
work_dir=$(mktemp -d)
trap 'rm -rf "$work_dir"' EXIT
curl --fail --show-error --location --retry 3 \
  "https://github.com/wasm-bindgen/wasm-pack/releases/download/v${version}/${archive}.tar.gz" \
  -o "$work_dir/archive.tar.gz"
printf '%s  %s\n' "$checksum" "$work_dir/archive.tar.gz" | sha256sum --check
tar xzf "$work_dir/archive.tar.gz" -C "$work_dir"
install -m 755 "$work_dir/$archive/wasm-pack" /usr/local/bin/wasm-pack
