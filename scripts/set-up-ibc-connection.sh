#!/usr/bin/env bash
set -euo pipefail

CLI=$(dirname "$0")/../target/debug/eclipse-ibc-cli
ENDPOINT_A=http://127.0.0.1:8111
ENDPOINT_B=http://127.0.0.1:9111
CHAIN_A=ibc-a
CHAIN_B=ibc-b
CLIENT_ID=xx-eclipse-0
CONNECTION_ID=connection-0
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

"$CLI" generate --endpoint "$ENDPOINT_B" connection open-init "$CLIENT_ID" "$CLIENT_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_A" connection open-init
"$CLI" generate --endpoint "$ENDPOINT_A" client update "$CLIENT_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_B" client update

"$CLI" generate --endpoint "$ENDPOINT_A" connection open-try "$CLIENT_ID" "$CLIENT_ID" "$CONNECTION_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_B" connection open-try
"$CLI" generate --endpoint "$ENDPOINT_B" client update "$CLIENT_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_A" client update

"$CLI" generate --endpoint "$ENDPOINT_B" connection open-ack "$CONNECTION_ID" "$CLIENT_ID" "$CONNECTION_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_A" connection open-ack
"$CLI" generate --endpoint "$ENDPOINT_A" client update "$CLIENT_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_B" client update

"$CLI" generate --endpoint "$ENDPOINT_A" connection open-confirm "$CONNECTION_ID" "$CONNECTION_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_B" connection open-confirm
"$CLI" generate --endpoint "$ENDPOINT_B" client update "$CLIENT_ID" \
  | "$CLI" tx --endpoint "$ENDPOINT_A" client update
