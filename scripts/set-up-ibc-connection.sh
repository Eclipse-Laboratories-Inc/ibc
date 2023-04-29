#!/usr/bin/env bash
set -euo pipefail

CLI=$(dirname "$0")/../target/debug/eclipse-ibc-cli
ENDPOINT_A=http://127.0.0.1:8111
ENDPOINT_B=http://127.0.0.1:9111
CHAIN_A=ibc-a
CHAIN_B=ibc-b
CLIENT_ID=xx-eclipse-0
CONNECTION_ID=connection-0
PORT_ID=test-msgs
CHANNEL_ID=channel-0
IBC_ACCOUNT=A7NJxtiKpEFL4TSTygkKSkf5b2g719DJbvQPRr4moUHD
export RUST_LOG=info

: <<'END_COMMENT'
END_COMMENT

"$CLI" tx --endpoint "$ENDPOINT_A" admin init-storage-account
"$CLI" tx --endpoint "$ENDPOINT_B" admin init-storage-account

"$CLI" generate --endpoint "$ENDPOINT_B" client create "$CHAIN_B" \
  | "$CLI" tx --endpoint "$ENDPOINT_A" client create
"$CLI" generate --endpoint "$ENDPOINT_A" client create "$CHAIN_A" \
  | "$CLI" tx --endpoint "$ENDPOINT_B" client create

"$CLI" generate --endpoint "$ENDPOINT_B" client update "$CLIENT_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_A" client update
"$CLI" generate --endpoint "$ENDPOINT_A" client update "$CLIENT_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_B" client update

"$CLI" generate --endpoint "$ENDPOINT_B" --cpty-endpoint "$ENDPOINT_A" connection open-init "$CLIENT_ID" "$CLIENT_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_A" connection open-init
"$CLI" generate --endpoint "$ENDPOINT_A" client update "$CLIENT_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_B" client update

"$CLI" generate --endpoint "$ENDPOINT_A" --cpty-endpoint "$ENDPOINT_B" connection open-try "$CLIENT_ID" "$CLIENT_ID" "$CONNECTION_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_B" connection open-try
"$CLI" generate --endpoint "$ENDPOINT_B" client update "$CLIENT_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_A" client update

"$CLI" generate --endpoint "$ENDPOINT_B" --cpty-endpoint "$ENDPOINT_A" connection open-ack "$CLIENT_ID" "$CONNECTION_ID" "$CLIENT_ID" "$CONNECTION_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_A" connection open-ack
"$CLI" generate --endpoint "$ENDPOINT_A" client update "$CLIENT_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_B" client update

"$CLI" generate --endpoint "$ENDPOINT_A" --cpty-endpoint "$ENDPOINT_B" connection open-confirm "$CLIENT_ID" "$CONNECTION_ID" "$CONNECTION_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_B" connection open-confirm
"$CLI" generate --endpoint "$ENDPOINT_B" client update "$CLIENT_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_A" client update

solana airdrop 100 "$IBC_ACCOUNT" --url "$ENDPOINT_A"
solana airdrop 100 "$IBC_ACCOUNT" --url "$ENDPOINT_B"

"$CLI" tx --endpoint "$ENDPOINT_A" port bind "$PORT_ID"
"$CLI" tx --endpoint "$ENDPOINT_B" port bind "$PORT_ID"

"$CLI" generate --endpoint "$ENDPOINT_A" client update "$CLIENT_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_B" client update

"$CLI" generate --endpoint "$ENDPOINT_B" client update "$CLIENT_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_A" client update

"$CLI" generate --endpoint "$ENDPOINT_B" --cpty-endpoint "$ENDPOINT_A" channel open-init "$CONNECTION_ID" "$PORT_ID" "$PORT_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_A" channel open-init
"$CLI" generate --endpoint "$ENDPOINT_A" client update "$CLIENT_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_B" client update

"$CLI" generate --endpoint "$ENDPOINT_A" --cpty-endpoint "$ENDPOINT_B" channel open-try "$CLIENT_ID" "$CONNECTION_ID" "$PORT_ID" "$PORT_ID" "$CHANNEL_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_B" channel open-try
"$CLI" generate --endpoint "$ENDPOINT_B" client update "$CLIENT_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_A" client update

"$CLI" generate --endpoint "$ENDPOINT_B" --cpty-endpoint "$ENDPOINT_A" channel open-ack "$CLIENT_ID" "$PORT_ID" "$CHANNEL_ID" "$PORT_ID" "$CHANNEL_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_A" channel open-ack

"$CLI" generate --endpoint "$ENDPOINT_A" client update "$CLIENT_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_B" client update

"$CLI" generate --endpoint "$ENDPOINT_A" --cpty-endpoint "$ENDPOINT_B" channel open-confirm "$CLIENT_ID" "$PORT_ID" "$CHANNEL_ID" "$PORT_ID" "$CHANNEL_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_B" channel open-confirm
"$CLI" generate --endpoint "$ENDPOINT_B" client update "$CLIENT_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_A" client update

solana airdrop 100 "$IBC_ACCOUNT" --url "$ENDPOINT_A"
solana airdrop 100 "$IBC_ACCOUNT" --url "$ENDPOINT_B"
