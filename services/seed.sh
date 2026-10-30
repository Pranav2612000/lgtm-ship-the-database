#!/usr/bin/env bash
# Appends N rows one at a time (default 1000) to the table `docs`. Each append
# is one commit, so one superfile and, with the demo engine, one manifest part
# (demo 3).
#
#   ./seed.sh [N]
set -euo pipefail

GATEWAY="${GATEWAY:-http://localhost:8080}"
DB="${DB:-demo}"
N="${1:-1000}"

call() {
  curl -s -o /dev/null -w "%{http_code}\n" -X POST "$GATEWAY/v1/$1/$DB" \
    -H 'Content-Type: application/json' -d "$2"
}

call create_table '{"table":"docs"}' >/dev/null

start=$(date +%s)
for i in $(seq "$N"); do
  id=$((start * 1000 + i))
  v="0.$RANDOM"
  for _ in $(seq 15); do v="$v,0.$RANDOM"; done
  call append "{\"table\":\"docs\",\"rows\":[{\"id\":$id,\"text\":\"hello grafana $id\",\"vector\":[$v]}]}"
done | sort | uniq -c
