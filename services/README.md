# Demo gateway and worker

A very small copy of the Infino data path. The gateway forwards each request to
the worker, and the worker runs it on a store on the local file system. Both
send logs, traces and metrics to the LGTM stack in `../deploy`.

```sh
cd ../deploy && docker compose up -d        # LGTM stack first
cd ../services && ./build-versions.sh       # builds images v1 and v2 (see demo 3)
docker compose up -d
./load.sh                                   # traffic (Ctrl-C to stop)
```

`docker compose down -v` also deletes the worker's data.

## API

`POST http://localhost:8080/v1/{op}/{database}` with a JSON body.

| Op              | Body                                                    |
|-----------------|---------------------------------------------------------|
| `create_table`  | `{"table":"docs"}`                                      |
| `drop_table`    | `{"table":"docs"}`                                      |
| `append`        | `{"table":"docs","rows":[{...}]}`                       |
| `update`        | `{"table":"docs","field":"id","value":1,"set":{...}}`   |
| `delete`        | `{"table":"docs","field":"id","value":1}`               |
| `optimize`      | `{"table":"docs"}`                                      |
| `query_sql`     | `{"sql":"SELECT * FROM docs LIMIT 5"}`                  |
| `count`         | `{"table":"docs"}`                                      |
| `exact_match`   | `{"table":"docs","field":"id","value":1}`               |
| `token_match`   | `{"table":"docs","field":"text","token":"hello"}`       |
| `vector_search` | `{"table":"docs","vector":[0.1,0.2,0.3],"k":3}`         |
| `hybrid_search` | `token_match` + `vector_search` fields                  |

Vectors are read from each row's `"vector"` field. `query_sql` supports only
`SELECT * FROM <table> [LIMIT <n>]`.

## How the worker works

A table is a folder on the `worker-data` volume: `/data/{database}/{table}/`.
Each `append` writes a new segment file (`000000.json`, `000001.json`, ...).
Every query reads all segment files one after another, and each read also waits
`SEGMENT_READ_MS` (default 5ms) to act like an object store read. `optimize`
merges all segments into one file.

So without `optimize` (`OPTIMIZE_EVERY=0 ./load.sh`), queries get slower with
every append. When a request takes more than `SLOW_QUERY_MS` (default 1000),
the gateway logs `Slow query detected`.

## Demo 1: random timeouts in the gateway

Before forwarding, the gateway checks the body in `parse_request`. For
`STALL_PERCENT` of requests (5 in `docker-compose.yaml`, 0 by default) it first
sleeps 0.5-3s. Requests longer than `REQUEST_TIMEOUT_MS` (2000) return `504`
with an ERROR log `request timed out`. The others longer than 1s log
`Slow query detected`.

Where it shows in Grafana:

- Gateway Overview: p95 latency and Error rate (5xx) go up.
- Alerts: `Slow query detected` and `High error rate` fire.
- Tempo: `{ name = "parse_request" && duration > 500ms }` finds the slow traces.
  They stop at `parse_request` and never reach the worker.

To "fix" it, set `STALL_PERCENT: "0"` and run `docker compose up -d`.

## Demo 3: segments read one after another

The worker reads segments one after another (`read_segments` in
`src/bin/worker.rs`), so a query gets slower with every segment. The fix is
`demo3-parallel-reads.patch`: it reads all segments at the same time.

`./build-versions.sh` builds both versions:

- `infino-demo:v1`: the current code, with the serial reads.
- `infino-demo:v2`: the code with the patch applied.

The source code always stays on v1.

On stage:

```sh
OPTIMIZE_EVERY=0 ./load.sh                          # segments pile up
# Grafana > Explore > Tempo: { name = "execute" && span.op = "count" }
../deploy/trace-to-perfetto.py <trace_id>           # open the file in ui.perfetto.dev
cat demo3-parallel-reads.patch                      # the fix
WORKER_VERSION=v2 docker compose up -d infino-worker
../deploy/trace-to-perfetto.py <new trace_id>       # reads now run side by side
```

The data is on disk, so deploying v2 keeps it. A plain `docker compose up -d`
goes back to v1.

## Telemetry

- Code: `src/lib.rs`. Exported every 15s to Alloy (`OTEL_EXPORTER_OTLP_ENDPOINT`).
- Traces: gateway `request` → `forward_to_worker` → worker `execute` →
  `read_segment` (one per segment). The trace context is sent in the
  `traceparent` header.
- Logs carry the trace id, so a log line in Loki opens its trace in Tempo.
- Metrics: see `../deploy/README.md`.
