.PHONY: build test lint fmt check doc clean ci wasm \
       regtest-certs regtest-up regtest-init regtest-test regtest-down regtest-clean regtest \
       regtest-logs regtest-status

# Default target
all: check

# Build all crates
build:
	cargo build --workspace

# Run all tests
test:
	cargo test --workspace

# Run clippy lints
lint:
	cargo clippy --workspace --all-targets -- -D warnings

# Check formatting
fmt:
	cargo fmt --all -- --check

# Format code (fix in place)
fmt-fix:
	cargo fmt --all

# Full check: fmt + lint + test
check: fmt lint test

# Build documentation
doc:
	cargo doc --workspace --no-deps

# Open documentation in browser
doc-open:
	cargo doc --workspace --no-deps --open

# Build Python bindings (maturin develop)
python:
	cd crates/l402-python && .venv/bin/maturin develop

# Build WASM package (default: bundler target)
WASM_TARGET ?= bundler
wasm:
	wasm-pack build crates/l402-wasm --target $(WASM_TARGET) --scope lightninglabs

# Clean build artifacts
clean:
	cargo clean
	rm -rf crates/l402-wasm/pkg
	rm -rf bindings/l402-go/lib
	rm -rf crates/l402-python/.venv crates/l402-python/.pytest_cache crates/l402-python/tests/__pycache__
	rm -rf packages/*/node_modules packages/*/dist packages/*/coverage packages/*/.cache packages/*/*.tsbuildinfo
	rm -rf packages/*/.pytest_cache packages/*/tests/__pycache__
	rm -rf examples/*/node_modules examples/*/dist examples/*/build examples/*/coverage examples/*/.cache
	rm -rf demos/*/node_modules demos/*/.next demos/*/out demos/*/dist demos/*/build demos/*/coverage demos/*/.cache
	rm -rf tests/regtest/lnd/tls
	rm -f tests/regtest/.env tests/regtest/.env.regtest
	find . -name '*.log' -type f -delete
	find . -name '*.test' -type f -delete
	find . -name '*.profraw' -type f -delete

# CI pipeline (same as GitHub Actions)
ci: fmt lint test doc
	@echo "CI checks passed."

# ─── Regtest integration tests ───────────────────────────────────────

REGTEST_COMPOSE := tests/regtest/docker-compose.yml

# Generate TLS certs for LND nodes (idempotent)
regtest-certs:
	./tests/regtest/scripts/gen-lnd-tls.sh

# Start the regtest Docker stack
regtest-up: regtest-certs
	docker compose -f $(REGTEST_COMPOSE) up -d

# Initialize the regtest network (fund wallets, open channels, export env)
regtest-init:
	./tests/regtest/scripts/init-regtest.sh

# Run regtest integration tests (all languages)
regtest-test: regtest-test-rust regtest-test-python regtest-test-wasm

regtest-test-rust:
	cargo test -p l402-regtest -- --nocapture

regtest-test-python:
	cd crates/l402-python && .venv/bin/pytest tests/test_regtest.py -v

regtest-test-wasm:
	cd crates/l402-wasm/tests/node && yarn vitest run tests/regtest.test.ts

# Tear down the regtest stack and remove volumes
regtest-down:
	docker compose -f $(REGTEST_COMPOSE) down -v

# Full cleanup: remove containers, volumes, generated certs, and env file
regtest-clean: regtest-down
	rm -rf tests/regtest/lnd/tls
	rm -f tests/regtest/.env.regtest

# Full regtest cycle: up → init → test → down
regtest: regtest-up regtest-init regtest-test
	@echo "Regtest tests passed. Run 'make regtest-down' to clean up."

# Show regtest Docker logs (useful for debugging)
regtest-logs:
	docker compose -f $(REGTEST_COMPOSE) logs --tail=100

# Show regtest service status
regtest-status:
	docker compose -f $(REGTEST_COMPOSE) ps
