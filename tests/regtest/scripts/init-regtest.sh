#!/usr/bin/env bash
# init-regtest.sh — Bootstrap the regtest Lightning network for L402sdk tests.
#
# Topology:
#   bitcoind          — regtest chain backend
#   lnd-alice (payer) — L402sdk LND client connects here (gRPC + REST)
#   lnd-bob (receiver)— Aperture uses this for invoices
#   cln (payer)       — L402sdk CLN client connects here (gRPC mTLS)
#   aperture          — reference L402 reverse proxy (backed by lnd-bob)
#   backend           — simple HTTP server behind aperture
#
# Channels:
#   alice → bob  (5M sats, push 1M)  — so alice can pay bob's invoices via Aperture
#   cln   → bob  (5M sats, push 1M)  — so cln can pay bob's invoices via Aperture
#
# Usage: ./init-regtest.sh [docker-compose-file]
# Requirements: docker compose, jq, bash 4+

set -euo pipefail
umask 077

COMPOSE_FILE="${1:-$(dirname "$0")/../docker-compose.yml}"
PROJECT_DIR="$(cd "$(dirname "$COMPOSE_FILE")" && pwd)"
ENV_FILE="${PROJECT_DIR}/.env.regtest"

# Shorthand helpers
btc()   { docker compose -f "$COMPOSE_FILE" exec -T bitcoind bitcoin-cli -regtest -rpcuser=regtest -rpcpassword=regtest "$@"; }
alice() { docker compose -f "$COMPOSE_FILE" exec -T lnd-alice lncli --network=regtest --tlscertpath=/tls/alice-tls.cert "$@"; }
bob()   { docker compose -f "$COMPOSE_FILE" exec -T lnd-bob lncli --network=regtest --tlscertpath=/tls/bob-tls.cert "$@"; }
cln()   { docker compose -f "$COMPOSE_FILE" exec -T cln lightning-cli --network=regtest "$@"; }

log() { echo "[init-regtest] $*"; }

open_cln_channel() {
  local peer_id="$1"
  local amount="$2"
  local push_msat="$3"
  local max_wait="${4:-60}"
  local elapsed=0

  while [ "$elapsed" -lt "$max_wait" ]; do
    local output
    if output=$(cln fundchannel id="$peer_id" amount="$amount" push_msat="$push_msat" 2>&1); then
      printf '%s\n' "$output"
      return 0
    fi

    if printf '%s' "$output" | grep -q "still syncing with bitcoin network"; then
      log "CLN still syncing, waiting before retrying channel open..."
      sleep 2
      elapsed=$((elapsed + 2))
      continue
    fi

    printf '%s\n' "$output" >&2
    return 1
  done

  log "ERROR: CLN did not become ready to fund a channel within ${max_wait}s"
  return 1
}

wait_for_sync() {
  local svc="$1" max_wait="${2:-90}"
  log "Waiting for $svc to sync..."
  local elapsed=0
  while [ $elapsed -lt "$max_wait" ]; do
    case "$svc" in
      lnd-alice)
        local synced
        synced=$(alice getinfo 2>/dev/null | jq -r '.synced_to_chain // false') || true
        [ "$synced" = "true" ] && return 0 ;;
      lnd-bob)
        local synced
        synced=$(bob getinfo 2>/dev/null | jq -r '.synced_to_chain // false') || true
        [ "$synced" = "true" ] && return 0 ;;
      cln)
        local info target_height
        target_height=$(btc getblockcount)
        info=$(cln getinfo 2>/dev/null) || info='{}'
        if printf '%s' "$info" | jq -e --argjson height "$target_height" \
          '.blockheight >= $height and (.warning_bitcoind_sync == null)' > /dev/null; then
          return 0
        fi ;;
    esac
    sleep 2
    elapsed=$((elapsed + 2))
  done
  log "ERROR: $svc did not sync after ${max_wait}s"
  return 1
}

# ─── Step 1: Mine initial blocks ─────────────────────────────────────

BLOCK_COUNT=$(btc getblockcount)
log "Current block height: $BLOCK_COUNT"

if [ "$BLOCK_COUNT" -lt 101 ]; then
  log "=== Step 1: Mining initial blocks ==="
  btc createwallet "regtest" 2>/dev/null || true
  btc loadwallet "regtest" 2>/dev/null || true
  MINER_ADDR=$(btc getnewaddress)
  btc generatetoaddress 101 "$MINER_ADDR" > /dev/null
  log "Mined 101 blocks."
else
  log "=== Step 1: Blocks already mined (height=$BLOCK_COUNT), skipping ==="
  btc loadwallet "regtest" 2>/dev/null || true
  MINER_ADDR=$(btc getnewaddress)
fi

sleep 3
wait_for_sync lnd-alice
wait_for_sync lnd-bob
wait_for_sync cln

# ─── Step 2: Fund all wallets ─────────────────────────────────────────

log "=== Step 2: Funding wallets ==="

ALICE_ADDR=$(alice newaddress p2tr | jq -r '.address')
BOB_ADDR=$(bob newaddress p2tr | jq -r '.address')
CLN_ADDR=$(cln newaddr | jq -r '.bech32 // .["p2tr-unannounced"]')

log "Alice address: $ALICE_ADDR"
log "Bob address:   $BOB_ADDR"
log "CLN address:   $CLN_ADDR"

btc sendtoaddress "$ALICE_ADDR" 2.0
btc sendtoaddress "$BOB_ADDR" 1.0
btc sendtoaddress "$CLN_ADDR" 2.0

btc generatetoaddress 6 "$MINER_ADDR" > /dev/null
log "Funded all wallets (confirmed)."

sleep 3
wait_for_sync lnd-alice
wait_for_sync lnd-bob
wait_for_sync cln

# ─── Step 3: Connect nodes ───────────────────────────────────────────

log "=== Step 3: Connecting nodes ==="

ALICE_PUBKEY=$(alice getinfo | jq -r '.identity_pubkey')
BOB_PUBKEY=$(bob getinfo | jq -r '.identity_pubkey')
CLN_PUBKEY=$(cln getinfo | jq -r '.id')

log "Alice pubkey: $ALICE_PUBKEY"
log "Bob pubkey:   $BOB_PUBKEY"
log "CLN pubkey:   $CLN_PUBKEY"

# Alice connects to Bob
alice connect "${BOB_PUBKEY}@lnd-bob:9735" 2>/dev/null || log "Alice already connected to Bob"

# CLN connects to Bob
cln connect "$BOB_PUBKEY" lnd-bob 9735 2>/dev/null || log "CLN already connected to Bob"

sleep 2

# ─── Step 4: Open channels ───────────────────────────────────────────

log "=== Step 4: Opening channels ==="

# Alice → Bob (5M sats, push 1M for some inbound liquidity)
EXISTING=$(alice listchannels 2>/dev/null | jq -r ".channels[] | select(.remote_pubkey==\"$BOB_PUBKEY\") | .chan_id" | head -1)
if [ -z "$EXISTING" ] || [ "$EXISTING" = "null" ]; then
  log "Opening Alice → Bob channel (5M sats, push 1M)..."
  alice openchannel --node_key="$BOB_PUBKEY" --local_amt=5000000 --push_amt=1000000
else
  log "Alice → Bob channel already exists: $EXISTING"
fi

# CLN → Bob (5M sats, push 1M)
EXISTING_CLN=$(cln listpeerchannels | jq -r ".channels[] | select(.peer_id==\"$BOB_PUBKEY\" and .opener==\"local\") | .short_channel_id" | head -1)
if [ -z "$EXISTING_CLN" ] || [ "$EXISTING_CLN" = "null" ]; then
  log "Opening CLN → Bob channel (5M sats, push 1M)..."
  open_cln_channel "$BOB_PUBKEY" 5000000 1000000000
else
  log "CLN → Bob channel already exists: $EXISTING_CLN"
fi

# Mine blocks to confirm channels
btc generatetoaddress 6 "$MINER_ADDR" > /dev/null
log "Channels opening, mining confirmation blocks."

# Wait for channels to be active
log "Waiting for channels to become active..."
for i in $(seq 1 60); do
  ALICE_ACTIVE=$(alice listchannels 2>/dev/null | jq '[.channels[] | select(.active==true)] | length')
  CLN_ACTIVE=$(cln listpeerchannels 2>/dev/null | jq '[.channels[] | select(.state=="CHANNELD_NORMAL")] | length')
  if [ "${ALICE_ACTIVE:-0}" -ge 1 ] && [ "${CLN_ACTIVE:-0}" -ge 1 ]; then
    log "Channels active! Alice: $ALICE_ACTIVE, CLN: $CLN_ACTIVE"
    break
  fi
  if [ "$i" -eq 60 ]; then
    log "ERROR: Channels did not become active"
    exit 1
  fi
  sleep 2
done

# Extra blocks for gossip propagation
btc generatetoaddress 6 "$MINER_ADDR" > /dev/null

# ─── Step 5: Extract credentials ─────────────────────────────────────

log "=== Step 5: Extracting credentials ==="

# LND Alice macaroon (hex) and TLS CA cert (base64)
ALICE_MACAROON_HEX=$(docker compose -f "$COMPOSE_FILE" exec -T lnd-alice \
  xxd -p -c 10000 /root/.lnd/data/chain/bitcoin/regtest/admin.macaroon)
ALICE_TLS_CERT_B64=$(base64 < "$PROJECT_DIR/lnd/tls/ca.pem" | tr -d '\n')

# CLN gRPC mTLS certs (base64-encoded)
CLN_CA_CERT_B64=$(docker compose -f "$COMPOSE_FILE" exec -T cln \
  base64 -w 0 /root/.lightning/regtest/ca.pem 2>/dev/null || echo "")
CLN_CLIENT_CERT_B64=$(docker compose -f "$COMPOSE_FILE" exec -T cln \
  base64 -w 0 /root/.lightning/regtest/client.pem 2>/dev/null || echo "")
CLN_CLIENT_KEY_B64=$(docker compose -f "$COMPOSE_FILE" exec -T cln \
  base64 -w 0 /root/.lightning/regtest/client-key.pem 2>/dev/null || echo "")
CLN_REST_RUNE=$(cln createrune 2>/dev/null | jq -r '.rune // empty')

# ─── Step 6: Setup SwissKnife ─────────────────────────────────────────

log "=== Step 6: Setting up SwissKnife ==="

# Export CLN rune to .env so docker-compose can pass it to swissknife
echo "CLN_RUNE=${CLN_REST_RUNE}" > "${PROJECT_DIR}/.env"

# Force-recreate swissknife so it picks up the new .env with CLN_RUNE
docker compose -f "$COMPOSE_FILE" up -d --force-recreate swissknife
log "SwissKnife container (re)started with CLN rune."

# Wait for SwissKnife to be ready
SWISSKNIFE_API_URL="http://localhost:3000"
for i in $(seq 1 60); do
  if curl -fsS -o /dev/null "$SWISSKNIFE_API_URL/v1/system/health" 2>/dev/null; then
    break
  fi
  if [ "$i" -eq 60 ]; then
    log "ERROR: SwissKnife did not become healthy"
    exit 1
  fi
  sleep 2
done

# Use the actual API-key path; a JWT is not an API key.
AUTH_BODY='{"password":"L402sdktestpass"}'
JWT_TOKEN=$(curl -fsS -X POST -H "Content-Type: application/json" \
  -d "$AUTH_BODY" "$SWISSKNIFE_API_URL/v1/auth/sign-up" | jq -er '.token') || \
JWT_TOKEN=$(curl -fsS -X POST -H "Content-Type: application/json" \
  -d "$AUTH_BODY" "$SWISSKNIFE_API_URL/v1/auth/sign-in" | jq -er '.token')
SWISSKNIFE_API_KEY=$(curl -fsS -X POST \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -d '{"name":"l402-regtest","permissions":["read:account","read:wallet","write:wallet","read:transaction","write:transaction"]}' \
  "$SWISSKNIFE_API_URL/v1/me/api-keys" | jq -er '.key')
unset JWT_TOKEN
log "SwissKnife API key created."

AUTH_HEADER="Api-Key: $SWISSKNIFE_API_KEY"
SWISSKNIFE_WALLET_ID=$(curl -fsS -H "$AUTH_HEADER" "$SWISSKNIFE_API_URL/v1/me/wallets" \
  | jq -er '.[0].id')
WALLET_URL="$SWISSKNIFE_API_URL/v1/me/wallets/$SWISSKNIFE_WALLET_ID"
BALANCE_MSAT=$(curl -fsS -H "$AUTH_HEADER" "$WALLET_URL/balance" \
  | jq -er '.available_msat')
if [ "$BALANCE_MSAT" -lt 100000000 ]; then
  log "Funding SwissKnife wallet (500k sats)..."
  FUNDING_BOLT11=$(curl -fsS -X POST \
    -H "Content-Type: application/json" -H "$AUTH_HEADER" \
    -d '{"amount_msat":500000000,"description":"regtest funding"}' \
    "$WALLET_URL/invoices" | jq -er '.ln_invoice.bolt11')
  # Channel activation does not imply that the two-hop route is in Alice's
  # graph yet. Wait for gossip and use an explicit funding-only fee allowance.
  for i in $(seq 1 60); do
    if alice queryroutes --dest="$CLN_PUBKEY" --amt=500000 --fee_limit=1000 2>/dev/null \
      | jq -e '.routes | length > 0' > /dev/null; then
      break
    fi
    if [ "$i" -eq 60 ]; then
      log "ERROR: Alice has no route to fund SwissKnife through CLN"
      exit 1
    fi
    sleep 2
  done
  if ! FUNDING_RESULT=$(alice payinvoice --force --json --fee_limit=1000 "$FUNDING_BOLT11"); then
    FUNDING_REASON=$(printf '%s' "$FUNDING_RESULT" | jq -r '.failure_reason // "unknown"')
    log "ERROR: SwissKnife funding payment failed: $FUNDING_REASON"
    exit 1
  fi
  for i in $(seq 1 30); do
    BALANCE_MSAT=$(curl -fsS -H "$AUTH_HEADER" "$WALLET_URL/balance" \
      | jq -er '.available_msat')
    [ "$BALANCE_MSAT" -ge 100000000 ] && break
    if [ "$i" -eq 30 ]; then
      log "ERROR: SwissKnife funding was not credited"
      exit 1
    fi
    sleep 2
  done
fi
log "SwissKnife wallet funded."

# ─── Step 7: Write environment file ──────────────────────────────────

log "=== Step 7: Writing environment file ==="

cat > "$ENV_FILE" <<EOF
# Auto-generated by init-regtest.sh — do not edit
# Generated at: $(date -u +%Y-%m-%dT%H:%M:%SZ)

# LND Alice (payer) — L402sdk LND client connects here
LND_GRPC_HOST=https://localhost:10009
LND_REST_HOST=https://localhost:8080
LND_MACAROON_HEX=${ALICE_MACAROON_HEX}
LND_TLS_CERT_BASE64=${ALICE_TLS_CERT_B64}
LND_PUBKEY=${ALICE_PUBKEY}

# CLN (payer) — L402sdk CLN client connects here
CLN_GRPC_HOST=https://localhost:9736
CLN_CA_CERT_BASE64=${CLN_CA_CERT_B64}
CLN_CLIENT_CERT_BASE64=${CLN_CLIENT_CERT_B64}
CLN_CLIENT_KEY_BASE64=${CLN_CLIENT_KEY_B64}
CLN_PUBKEY=${CLN_PUBKEY}
CLN_REST_URL=https://localhost:3010
CLN_RUNE=${CLN_REST_RUNE}

# LND Bob (receiver) — used by Aperture
BOB_PUBKEY=${BOB_PUBKEY}

# L402 Server (Aperture proxy)
L402_SERVER_URL=http://localhost:8081

# SwissKnife (custodial wallet connected to lnd-bob)
SWISSKNIFE_API_URL=${SWISSKNIFE_API_URL}
SWISSKNIFE_API_KEY=${SWISSKNIFE_API_KEY}
SWISSKNIFE_WALLET_ID=${SWISSKNIFE_WALLET_ID}

# Bitcoin
BITCOIND_RPC_URL=http://localhost:18443
EOF

log "=== Environment written to $ENV_FILE ==="
log ""
log "Summary:"
log "  Alice (payer, LND):   $ALICE_PUBKEY"
log "  Bob (receiver, LND):  $BOB_PUBKEY"
log "  CLN (payer):          $CLN_PUBKEY"
log "  L402 server:          http://localhost:8081 (Aperture → backend)"
log "  Alice gRPC:           https://localhost:10009"
log "  Alice REST:           https://localhost:8080"
log "  CLN gRPC:             https://localhost:9736"
log "  CLN REST:             https://localhost:3010"
log "  SwissKnife:           http://localhost:3000"
log ""
log "=== Regtest environment ready! ==="
