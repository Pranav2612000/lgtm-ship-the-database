# Local LGTM stack (talk demo)

A copy of our prod Grafana stack (Grafana, Mimir, Loki, Tempo + Alloy), on
docker-compose instead of a GCP VM. Same image versions, datasources,
dashboards and alert rules as prod, trimmed to the demo services.

```sh
docker compose up -d        # start
docker compose down         # stop, keep data
docker compose down -v      # stop, delete all data
```

| What    | Where                                         |
|---------|-----------------------------------------------|
| Grafana | http://localhost:3000 (admin / admin)         |
| OTLP    | localhost:4317 (gRPC), localhost:4318 (HTTP)  |
| Alloy   | http://localhost:12345                        |
| MCP     | http://localhost:8000/mcp (see `.mcp.json`)   |

All ports bind to 127.0.0.1 only. Set `GRAFANA_ADMIN_PASSWORD` to change the
password.

## How data flows

```
infino-gateway ─┐                    ┌─> Mimir  (metrics)
                ├─ OTLP ─> Alloy ────┼─> Loki   (logs)
infino-worker  ─┘                    └─> Tempo  (traces) ─ span metrics ─> Mimir
container stdout/stderr ─> Alloy ─> Loki   (panics, crash output)
```

## What the demo services send

The services in `../services` send these. Dashboards and alerts expect the same names as prod:

- `service.name`: `infino-gateway` or `infino-worker`. It becomes the `job` label.
- `service.instance.id`: for example `gateway-1`. It becomes the `instance` label.
- Export metrics every **15s** (prod uses 60s, which is too slow for a live demo).
- Compose service names must start with `infino-` so Alloy ships their container logs.

| Metric                            | Type             | Sent by | Labels   |
|-----------------------------------|------------------|---------|----------|
| `infino_queries`                  | counter          | gateway |          |
| `infino_worker_request_duration`  | histogram, ms    | gateway | `status`, `op` |
| `infino_host_cpu_percent`         | gauge            | both    |          |
| `infino_host_ram_percent`         | gauge            | both    |          |
| `infino_host_disk_percent`        | gauge            | both    |          |
| `infino_process_rss_bytes`        | gauge            | both    |          |
| `infino_process_cpu_ns`           | counter          | both    |          |

Log lines that alerts look for: ``Slow query detected``, ``panicked at ``, and
any ERROR-level log (High error rate).

Traces: the gateway should propagate the trace context to the worker, so one
request is one trace. The service map and span latency panels come from this.

## Alerts

Rules from prod: High disk usage, high memory usage, High error rate, Slow query
detected, Process panicked, Loki not answering. They have no contact point yet,
so they show up only in Grafana (Alerting > Alert rules).

## Traces in Perfetto

`./trace-to-perfetto.py <trace_id | exported.json>` downloads a trace from Tempo
(or reads a JSON file exported from Grafana) and writes a file to open in
https://ui.perfetto.dev. Each service is a process, and spans that run at the
same time go on separate rows.
