# Demo gateway and worker

A very small copy of the Infino data path. The gateway forwards each request to
the worker, and the worker runs it on the real infino engine
(github.com/infino-ai/infino), which stores its data in MinIO. All of them send
logs, traces and metrics to the LGTM stack in `../deploy`.

```sh
cd ../deploy && docker compose up -d        # LGTM stack first
cd ../services && ./build-versions.sh       # images v1 and v2, ~8 min the first time
docker compose up -d
./load.sh                                   # traffic (Ctrl-C to stop)
```

`docker compose down -v` also deletes the data in MinIO. Steps for each demo:
`../DEMO.md`.

## API

`POST http://localhost:8080/v1/{op}/{database}` with a JSON body. The response
header `x-trace-id` is the request's trace id in Tempo.

Every table has the same schema: `id` (int), `text` (full-text index) and
`vector` (16 floats, cosine).

| Op              | Body                                                                   |
|-----------------|------------------------------------------------------------------------|
| `create_table`  | `{"table":"docs"}`                                                     |
| `drop_table`    | `{"table":"docs"}`                                                     |
| `append`        | `{"table":"docs","rows":[{"id":1,"text":"...","vector":[16 floats]}]}` |
| `update`        | `{"table":"docs","field":"id","value":1,"rows":[new row]}`             |
| `delete`        | `{"table":"docs","field":"id","value":1}`                              |
| `optimize`      | `{"table":"docs"}`                                                     |
| `query_sql`     | `{"sql":"SELECT id, text FROM docs LIMIT 5"}`                          |
| `count`         | `{"table":"docs","field":"text","query":"grafana"}`                    |
| `exact_match`   | `{"table":"docs","field":"text","query":"hello grafana 1"}`            |
| `token_match`   | `{"table":"docs","field":"text","query":"grafana"}`                    |
| `vector_search` | `{"table":"docs","vector":[16 floats],"k":3}`                          |
| `hybrid_search` | `{"table":"docs","field":"text","query":"hello","vector":[16 floats],"k":3}` |

`update` replaces each matching row with a row from `rows` (same count).

## Scripts

- `load.sh`: traffic for the dashboards (an append, an update and every query
  each round). `OPTIMIZE_EVERY=N` also runs optimize every N rounds.
- `seed.sh [N]`: N appends, one at a time (default 1000), about 2 minutes.
- `build-versions.sh`: builds `infino-demo:v1` and `infino-demo:v2` (demo 3).
- `get-engine.sh`: gets the engine at a commit, used by the Dockerfile.

## Demo 1: random timeouts in the gateway

Before forwarding, the gateway checks the body in `parse_request`. For
`STALL_PERCENT` of requests (5 in `docker-compose.yaml`, 0 by default) it first
sleeps 0.5-3s. Requests longer than `REQUEST_TIMEOUT_MS` (2000; optimize gets
60s) return `504` with an ERROR log `request timed out`. The others longer than
1s log `Slow query detected`.

## Demo 3: manifest parts loaded one after another (infino #1033)

A table's manifest is split into parts. When `optimize` compacts superfiles,
it removes them from every part, and before #1033 it loaded those parts from
storage one after another (a loop of awaits). #1033 loads them 32 at a time.

The two images differ only in the engine commit:

- `v1`: `589ed923`, the commit before #1033.
- `v2`: `9ec8f405`, #1033 itself.

Both builds have one demo-only change (`get-engine.sh`): a manifest part holds
1 superfile instead of 10,000, so each append makes a new part and 1000
appends make ~1000 parts. Both build with `detailed-tracing`, which turns on
the engine's own spans, like prod.

With ~1000 parts on local MinIO:

|                      | v1                       | v2                       |
|----------------------|--------------------------|--------------------------|
| `manifest.part_load` | ~980, one after another  | ~980, up to 32 at a time |
| `manifest.update`    | ~1s                      | ~0.2s                    |

The parts must not already be in memory: opening a table loads all of them,
but every commit keeps only the newest 4. So `seed.sh` (many commits) right
before `optimize` gives the cold loads, like a worker that keeps ingesting.

## Telemetry

- Code: `src/lib.rs`. Exported every 15s to Alloy (`OTEL_EXPORTER_OTLP_ENDPOINT`).
- Traces: gateway `request` → `forward_to_worker` → worker `execute` → the
  engine's spans. The trace context is sent in the `traceparent` header.
- Logs carry the trace id, so a log line in Loki opens its trace in Tempo.
- Metrics: see `../deploy/README.md`.
