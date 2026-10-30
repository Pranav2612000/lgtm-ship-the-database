#!/usr/bin/env bash
# Sends traffic through the gateway: appends, every query op, and an optimize
# every OPTIMIZE_EVERY rounds (0 = never, so segments pile up and queries slow down).
set -euo pipefail

GATEWAY="${GATEWAY:-http://localhost:8080}"
DB="${DB:-demo}"
OPTIMIZE_EVERY="${OPTIMIZE_EVERY:-50}"

call() {
  curl -s -o /dev/null -w "%{http_code} $1\n" -X POST "$GATEWAY/v1/$1/$DB" \
    -H 'Content-Type: application/json' -d "$2"
}

call create_table '{"table":"docs"}'

round=0
while true; do
  round=$((round + 1))
  id=$RANDOM
  call append "{\"table\":\"docs\",\"rows\":[{\"id\":$id,\"text\":\"hello grafana round $round\",\"vector\":[0.$RANDOM,0.$RANDOM,0.$RANDOM]}]}"
  call update "{\"table\":\"docs\",\"field\":\"id\",\"value\":$id,\"set\":{\"seen\":true}}"
  call query_sql '{"sql":"SELECT * FROM docs LIMIT 5"}'
  call count '{"table":"docs"}'
  call exact_match "{\"table\":\"docs\",\"field\":\"id\",\"value\":$id}"
  call token_match '{"table":"docs","field":"text","token":"grafana"}'
  call vector_search '{"table":"docs","vector":[0.1,0.2,0.3],"k":3}'
  call hybrid_search '{"table":"docs","field":"text","token":"hello","vector":[0.1,0.2,0.3],"k":3}'
  if (( round % 20 == 0 )); then
    call delete "{\"table\":\"docs\",\"field\":\"id\",\"value\":$id}"
  fi
  if (( OPTIMIZE_EVERY > 0 )) && (( round % OPTIMIZE_EVERY == 0 )); then
    call optimize '{"table":"docs"}'
  fi
  sleep 0.5
done
