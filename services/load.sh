#!/usr/bin/env bash
# Sends traffic through the gateway: an append, an update and every query op
# each round, a delete every 20 rounds, and an optimize every OPTIMIZE_EVERY
# rounds (0 = never, the default, so parts pile up for demo 3).
set -euo pipefail

GATEWAY="${GATEWAY:-http://localhost:8080}"
DB="${DB:-demo}"
OPTIMIZE_EVERY="${OPTIMIZE_EVERY:-0}"

call() {
  curl -s -o /dev/null -w "%{http_code} $1\n" -X POST "$GATEWAY/v1/$1/$DB" \
    -H 'Content-Type: application/json' -d "$2"
}

# A random 16-dim vector, the width of the demo schema.
vector() {
  local v="0.$RANDOM"
  for _ in $(seq 15); do v="$v,0.$RANDOM"; done
  echo "[$v]"
}

row() {
  echo "{\"id\":$1,\"text\":\"hello grafana $1\",\"vector\":$(vector)}"
}

call create_table '{"table":"docs"}'

start=$(date +%s)
round=0
while true; do
  round=$((round + 1))
  id=$((start * 1000 + round))
  call append "{\"table\":\"docs\",\"rows\":[$(row $id)]}"
  call update "{\"table\":\"docs\",\"field\":\"id\",\"value\":$id,\"rows\":[$(row $id)]}"
  call query_sql '{"sql":"SELECT id, text FROM docs LIMIT 5"}'
  call count '{"table":"docs","field":"text","query":"grafana"}'
  call exact_match "{\"table\":\"docs\",\"field\":\"text\",\"query\":\"hello grafana $id\"}"
  call token_match '{"table":"docs","field":"text","query":"grafana"}'
  call vector_search "{\"table\":\"docs\",\"vector\":$(vector),\"k\":3}"
  call hybrid_search "{\"table\":\"docs\",\"field\":\"text\",\"query\":\"hello\",\"vector\":$(vector),\"k\":3}"
  if (( round % 20 == 0 )); then
    call delete "{\"table\":\"docs\",\"field\":\"id\",\"value\":$id}"
  fi
  if (( OPTIMIZE_EVERY > 0 )) && (( round % OPTIMIZE_EVERY == 0 )); then
    call optimize '{"table":"docs"}'
  fi
  sleep 0.5
done
