# Running the demos from scratch

All commands run from `lgtm-ship-the-database/`. Needs Docker Desktop.

## 1. Before the talk (about 15 minutes)

```sh
# LGTM stack: Grafana, Mimir, Loki, Tempo, Alloy, MCP
cd deploy && docker compose down -v && docker compose up -d && cd ..

# Images v1 (engine before #1033) and v2 (#1033). ~8 min the first time.
cd services && ./build-versions.sh

# Start clean on v1
docker compose down -v
docker compose up -d
```

Grafana: http://localhost:3000 (admin / admin).

### Prepare the demo 3 traces

Run both versions now and keep the two trace ids, so on stage you only open
them. Each `seed.sh` takes about 2 minutes.

```sh
# v1: the bug
./seed.sh 1000
curl -si -X POST localhost:8080/v1/optimize/demo \
  -H 'Content-Type: application/json' -d '{"table":"docs"}' | grep -i x-trace-id
# -> write down the id: SLOW trace

# v2: the fix
WORKER_VERSION=v2 docker compose up -d infino-worker
./seed.sh 1000
curl -si -X POST localhost:8080/v1/optimize/demo \
  -H 'Content-Type: application/json' -d '{"table":"docs"}' | grep -i x-trace-id
# -> write down the id: FAST trace

# Back to v1 for the rest of the talk
docker compose up -d infino-worker
```

Check that each worker ran the right engine (prints `589ed923f503` for v1,
`9ec8f4054d4c` for v2):

```sh
docker compose exec infino-worker sh -c \
  "grep -aoE '(589ed923f503|9ec8f4054d4c)' /usr/local/bin/infino-worker | head -1"
```

Then start traffic for the dashboards and demo 1, and leave it running:

```sh
./load.sh > /dev/null &
```

## 2. Demo 3 on stage: the trace in Grafana Tempo

1. Grafana → **Explore** → data source **Tempo** → query type **TraceQL**.
2. Paste the SLOW trace id and run it. The trace opens:
   `request` → `forward_to_worker` → `execute` → `optimize` → `compact` →
   `compact_table` → `commit_compaction_batch` → `compaction_commit_attempt` →
   `manifest.update`.
3. Expand `manifest.update`: about 1000 `manifest.part_load` spans, each starting
   when the one before ends, a staircase of about 1s. Click one span: its
   attributes show `cache_hit = false` and a different `part_id`.
4. Ask the room what is wrong. Hints, one at a time: "How long is each span?
   How many are there? Do any of them overlap?"
5. Reveal #1033: the loop awaited each part load in turn; the fix loads 32 at
   a time. Its issue (#1025): 2,112 parts × ~146 ms ≈ 287 s in prod.
6. Open the FAST trace id the same way: the same ~1000 `manifest.part_load`
   spans now overlap, and `manifest.update` takes about 0.2s instead of 1s.

To find these traces without the ids:

```
{ resource.service.name = "infino-worker" && name = "manifest.update" && duration > 500ms }
```

Compare `manifest.update`, not the whole request: the rest of `optimize`
(vector drain, gc) varies by more than the fix saves on local MinIO.

### Optional: Perfetto

```sh
deploy/trace-to-perfetto.py <trace id>      # writes <trace id>.perfetto.json
```

Open the file in https://ui.perfetto.dev. The slow trace's loads sit on one
row; the fast trace's spread over many rows.

### Optional: run it live

The same steps as above, on stage: `./seed.sh 1000` (about 2 minutes), then
the `optimize` call. Deploy the fix with
`WORKER_VERSION=v2 docker compose up -d infino-worker`. The data is in MinIO,
so it survives the deploy, but run `./seed.sh` again before `optimize`: the
first optimize already compacted the old parts.

## 3. Demo 1 on stage: random gateway timeouts

`load.sh` must have run for a few minutes (`STALL_PERCENT` is 5).

1. Dashboards → **Infino - Gateway Overview**: p95 latency and Error rate (5xx).
2. Alerting → Alert rules: `Slow query detected`, `High error rate`.
3. Explore → Tempo: `{ name = "parse_request" && duration > 500ms }`. The slow
   traces stop in `parse_request` and never reach the worker.
4. The fix: `STALL_PERCENT: "0"` in `services/docker-compose.yaml`, then
   `docker compose up -d infino-gateway`.

## 4. Demo 4: Grafana MCP

The MCP server runs at http://localhost:8000/mcp, read-only.
`deploy/.mcp.json` registers it for Claude Code: start `claude` from
`deploy/` (or copy the file to where you run it).

## After the talk

```sh
cd services && docker compose down -v
cd ../deploy && docker compose down -v
```
