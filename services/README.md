# Demo gateway and worker

A very small copy of the Infino data path. The gateway forwards each request to
the worker, and the worker runs it on an in-memory store. Both send logs,
traces and metrics to the LGTM stack in `../deploy`.

```sh
cd ../deploy && docker compose up -d        # LGTM stack first
cd ../services && docker compose up -d --build
./load.sh                                   # traffic (Ctrl-C to stop)
```

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

Each `append` adds a new segment. Every query reads all segments one after
another, and each read sleeps `SEGMENT_READ_MS` (default 5ms) in place of an
object store read. `optimize` merges all segments into one.

So without `optimize` (`OPTIMIZE_EVERY=0 ./load.sh`), queries get slower with
every append. When a request takes more than `SLOW_QUERY_MS` (default 1000),
the gateway logs `Slow query detected`.

## Telemetry

- Code: `src/lib.rs`. Exported every 15s to Alloy (`OTEL_EXPORTER_OTLP_ENDPOINT`).
- Traces: gateway `request` → `forward_to_worker` → worker `execute` →
  `read_segment` (one per segment). The trace context is sent in the
  `traceparent` header.
- Logs carry the trace id, so a log line in Loki opens its trace in Tempo.
- Metrics: see `../deploy/README.md`.
