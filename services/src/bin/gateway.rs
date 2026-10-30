//! Demo gateway: forwards query, ingest and optimize requests to the worker.
//!
//!   POST /v1/{op}/{database}  ->  POST {WORKER_URL}/{op}/{database}

use std::time::Instant;

use axum::{
    Router,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
    routing::post,
};
use opentelemetry::{
    KeyValue, global,
    metrics::{Counter, Histogram},
};
use opentelemetry_http::HeaderInjector;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

const QUERY_OPS: &[&str] = &[
    "query_sql",
    "token_match",
    "exact_match",
    "count",
    "vector_search",
    "hybrid_search",
];
const OTHER_OPS: &[&str] = &[
    "append",
    "update",
    "delete",
    "create_table",
    "drop_table",
    "optimize",
];

#[derive(Clone)]
struct AppState {
    client: reqwest::Client,
    worker_url: String,
    slow_query_ms: u64,
    queries: Counter<u64>,
    request_duration: Histogram<u64>,
}

fn main() {
    let _telemetry = infino_demo::init("infino-gateway");
    tokio::runtime::Runtime::new().unwrap().block_on(run());
}

async fn run() {
    let meter = global::meter("infino");
    let state = AppState {
        client: reqwest::Client::new(),
        worker_url: std::env::var("WORKER_URL").unwrap_or_else(|_| "http://localhost:9000".into()),
        slow_query_ms: env_u64("SLOW_QUERY_MS", 1000),
        queries: meter.u64_counter("infino.queries").build(),
        // Milliseconds, with the same buckets as prod.
        request_duration: meter
            .u64_histogram("infino.worker.request_duration")
            .with_boundaries(vec![
                0.0, 100.0, 500.0, 1000.0, 5000.0, 10000.0, 60000.0, 120000.0,
            ])
            .build(),
    };

    let app = Router::new()
        .route("/v1/{op}/{database}", post(forward))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    tracing::info!("gateway listening on :8080");
    axum::serve(listener, app).await.unwrap();
}

#[tracing::instrument(name = "request", skip_all, fields(op = %op, database = %database, otel.kind = "server"))]
async fn forward(
    State(state): State<AppState>,
    Path((op, database)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    let is_query = QUERY_OPS.contains(&op.as_str());
    if !is_query && !OTHER_OPS.contains(&op.as_str()) {
        return (StatusCode::NOT_FOUND, format!("unknown op: {op}")).into_response();
    }

    let start = Instant::now();
    let (status, body) = match call_worker(&state, &op, &database, body).await {
        Ok(response) => response,
        Err(err) => {
            tracing::error!(%err, "worker request failed");
            (StatusCode::BAD_GATEWAY, Bytes::from(err.to_string()))
        }
    };
    let duration_ms = start.elapsed().as_millis() as u64;

    state.request_duration.record(
        duration_ms,
        &[
            KeyValue::new("status", status.as_u16().to_string()),
            KeyValue::new("op", op.clone()),
        ],
    );
    if is_query {
        state.queries.add(1, &[]);
    }
    if duration_ms > state.slow_query_ms {
        tracing::warn!(
            op,
            database,
            status = status.as_u16(),
            duration_ms,
            threshold_ms = state.slow_query_ms,
            "Slow query detected"
        );
    }

    (status, [(CONTENT_TYPE, "application/json")], body).into_response()
}

#[tracing::instrument(name = "forward_to_worker", skip_all, fields(otel.kind = "client"))]
async fn call_worker(
    state: &AppState,
    op: &str,
    database: &str,
    body: Bytes,
) -> Result<(StatusCode, Bytes), reqwest::Error> {
    // Send the trace context, so the worker's spans join this trace.
    let mut headers = HeaderMap::new();
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(
            &Span::current().context(),
            &mut HeaderInjector(&mut headers),
        )
    });
    headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());

    let response = state
        .client
        .post(format!("{}/{op}/{database}", state.worker_url))
        .headers(headers)
        .body(body)
        .send()
        .await?;
    Ok((response.status(), response.bytes().await?))
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
