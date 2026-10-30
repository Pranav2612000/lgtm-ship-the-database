//! Telemetry shared by the gateway and the worker: logs, traces and metrics
//! over OTLP, plus host metrics and a panic hook. Metric names match prod.

use std::time::Duration;

use opentelemetry::{KeyValue, global, trace::TracerProvider as _};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, MetricExporter, SpanExporter};
use opentelemetry_sdk::{
    Resource,
    logs::SdkLoggerProvider,
    metrics::{PeriodicReader, SdkMeterProvider},
    propagation::TraceContextPropagator,
    trace::SdkTracerProvider,
};
use sysinfo::{Disks, ProcessesToUpdate, System};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

/// Same as the Mimir datasource `timeInterval` in deploy/.
const EXPORT_INTERVAL: Duration = Duration::from_secs(15);

/// Flushes all telemetry on drop.
pub struct Telemetry {
    tracer: SdkTracerProvider,
    logger: SdkLoggerProvider,
    meter: SdkMeterProvider,
}

impl Drop for Telemetry {
    fn drop(&mut self) {
        let _ = self.tracer.shutdown();
        let _ = self.logger.shutdown();
        let _ = self.meter.shutdown();
    }
}

/// Call before starting the tokio runtime: the OTLP exporters use a blocking
/// HTTP client, which panics when created inside the runtime.
/// The endpoint comes from `OTEL_EXPORTER_OTLP_ENDPOINT`.
pub fn init(service: &'static str) -> Telemetry {
    let instance = std::env::var("INSTANCE_ID").unwrap_or_else(|_| service.to_string());
    let resource = Resource::builder()
        .with_service_name(service)
        .with_attribute(KeyValue::new("service.instance.id", instance))
        .build();

    let tracer = SdkTracerProvider::builder()
        .with_batch_exporter(SpanExporter::builder().with_http().build().unwrap())
        .with_resource(resource.clone())
        .build();
    let logger = SdkLoggerProvider::builder()
        .with_batch_exporter(LogExporter::builder().with_http().build().unwrap())
        .with_resource(resource.clone())
        .build();
    let reader = PeriodicReader::builder(MetricExporter::builder().with_http().build().unwrap())
        .with_interval(EXPORT_INTERVAL)
        .build();
    let meter = SdkMeterProvider::builder()
        .with_reader(reader)
        .with_resource(resource)
        .build();

    global::set_meter_provider(meter.clone());
    global::set_text_map_propagator(TraceContextPropagator::new());

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().with_ansi(false))
        .with(tracing_opentelemetry::layer().with_tracer(tracer.tracer(service)))
        .with(OpenTelemetryTracingBridge::new(&logger))
        .init();

    // Log panics as `panicked at <location>: <message>`, like prod. The
    // "Process panicked" alert searches for this text.
    std::panic::set_hook(Box::new(|info| {
        let message = info
            .payload_as_str()
            .unwrap_or("<non-string panic payload>");
        let location = info.location().map(ToString::to_string).unwrap_or_default();
        tracing::error!("panicked at {location}: {message}");
    }));

    report_host_metrics();

    Telemetry {
        tracer,
        logger,
        meter,
    }
}

/// Host and process metrics, on a plain thread every export interval.
fn report_host_metrics() {
    let meter = global::meter("infino");
    let cpu = meter.u64_gauge("infino.host.cpu_percent").build();
    let ram = meter.u64_gauge("infino.host.ram_percent").build();
    let disk = meter.u64_gauge("infino.host.disk_percent").build();
    let rss = meter.u64_gauge("infino.process.rss_bytes").build();
    let cpu_ns = meter.u64_counter("infino.process.cpu_ns").build();

    std::thread::spawn(move || {
        let pid = sysinfo::get_current_pid().unwrap();
        let mut sys = System::new();
        let mut disks = Disks::new_with_refreshed_list();
        let mut last_cpu_ms = 0;
        loop {
            sys.refresh_cpu_usage();
            sys.refresh_memory();
            sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
            disks.refresh(true);

            cpu.record(sys.global_cpu_usage() as u64, &[]);
            ram.record(sys.used_memory() * 100 / sys.total_memory().max(1), &[]);
            let (total, free) = disks.list().iter().fold((0, 0), |(t, f), d| {
                (t + d.total_space(), f + d.available_space())
            });
            disk.record((total - free) * 100 / total.max(1), &[]);

            if let Some(process) = sys.process(pid) {
                rss.record(process.memory(), &[]);
                let cpu_ms = process.accumulated_cpu_time();
                cpu_ns.add((cpu_ms - last_cpu_ms) * 1_000_000, &[]);
                last_cpu_ms = cpu_ms;
            }
            std::thread::sleep(EXPORT_INTERVAL);
        }
    });
}
