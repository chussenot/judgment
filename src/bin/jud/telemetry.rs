//! OTLP export for the `jud` binary (feature `cli-otel`): the library's spans
//! go to a trace pipeline and its `OtelObserver` metrics to a metric one, both
//! over OTLP/HTTP with protobuf bodies, to the collector named by
//! `OTEL_EXPORTER_OTLP_ENDPOINT`. Without that variable nothing is started and
//! a run costs what it always did.
//!
//! The SDK and the exporter live here and nowhere in the library, which names
//! the OpenTelemetry API only. The standard `OTEL_*` variables (headers,
//! protocol-specific endpoints, resource attributes, timeout) are read by the
//! exporters themselves.
//!
//! A telemetry failure never changes what `jud` does: it is said once on
//! stderr, stdout is untouched, and the exit status is the command's own. The
//! returned [`Guard`] flushes both pipelines when it drops, which `main` makes
//! the last thing on every path, status 0, 1, 2 and 3 alike.

use std::sync::Arc;
use std::time::Duration;

use judgment::observer::otel::OtelObserver;
use opentelemetry::metrics::MeterProvider as _;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::{MetricExporter, Protocol, SpanExporter, WithExportConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::layer::SubscriberExt;

use crate::out;

const ENDPOINT: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";
const TIMEOUT: &str = "OTEL_EXPORTER_OTLP_TIMEOUT";
/// A short default, so a collector that is down delays the exit by seconds,
/// not by the exporters' ten; the standard variable overrides it.
const EXPORT_TIMEOUT: Duration = Duration::from_secs(3);

/// The two pipelines; dropping it flushes and shuts both down.
pub(crate) struct Guard {
    traces: SdkTracerProvider,
    metrics: SdkMeterProvider,
}

/// Start export when the endpoint variable is set and non-empty. A failure to
/// start is reported and the run goes on without telemetry.
pub(crate) fn init() -> Option<Guard> {
    if std::env::var_os(ENDPOINT).is_none_or(|v| v.is_empty()) {
        return None;
    }
    match start() {
        Ok(guard) => Some(guard),
        Err(e) => {
            out::note!("jud: telemetry disabled: {e}");
            None
        }
    }
}

fn start() -> Result<Guard, Box<dyn std::error::Error>> {
    let resource = Resource::builder().with_service_name("jud").build();
    let timeout = std::env::var_os(TIMEOUT).is_none();

    let mut spans = SpanExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary);
    let mut metrics = MetricExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary);
    if timeout {
        spans = spans.with_timeout(EXPORT_TIMEOUT);
        metrics = metrics.with_timeout(EXPORT_TIMEOUT);
    }

    let traces = SdkTracerProvider::builder()
        .with_resource(resource.clone())
        .with_batch_exporter(spans.build()?)
        .build();
    let meters = SdkMeterProvider::builder()
        .with_resource(resource)
        .with_periodic_exporter(metrics.build()?)
        .build();

    let layer = tracing_opentelemetry::layer().with_tracer(traces.tracer("judgment"));
    tracing::subscriber::set_global_default(tracing_subscriber::registry().with(layer))?;
    // First install wins, and nothing else in the binary installs one.
    judgment::observer::set_global(Arc::new(OtelObserver::new(meters.meter("judgment"))));
    Ok(Guard {
        traces,
        metrics: meters,
    })
}

impl Drop for Guard {
    fn drop(&mut self) {
        let mut failures = Vec::new();
        if let Err(e) = self.traces.shutdown() {
            failures.push(format!("traces: {e}"));
        }
        if let Err(e) = self.metrics.shutdown() {
            failures.push(format!("metrics: {e}"));
        }
        if !failures.is_empty() {
            out::note!("jud: telemetry not exported ({})", failures.join("; "));
        }
    }
}
