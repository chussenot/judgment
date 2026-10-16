//! What the OpenTelemetry tests share: a meter provider that keeps what it
//! exports in memory, and the mock-server helpers around it.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use judgment::observer::otel::OtelObserver;
use judgment::{Client, Questions, RetryPolicy};
use opentelemetry::metrics::MeterProvider;
use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData};
use opentelemetry_sdk::metrics::{InMemoryMetricExporter, PeriodicReader, SdkMeterProvider};
use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// One data point: its attributes as strings, how many values it holds and
/// their sum (a counter's value is both).
#[derive(Debug)]
pub struct Point {
    pub attrs: BTreeMap<String, String>,
    pub count: u64,
    pub sum: f64,
}

/// One instrument as exported.
#[derive(Debug)]
pub struct Instrument {
    pub unit: String,
    pub points: Vec<Point>,
}

/// A meter provider that keeps what it exports in memory.
pub struct Rig {
    provider: SdkMeterProvider,
    exporter: InMemoryMetricExporter,
}

impl Rig {
    pub fn new() -> Self {
        let exporter = InMemoryMetricExporter::default();
        let reader = PeriodicReader::builder(exporter.clone()).build();
        let provider = SdkMeterProvider::builder().with_reader(reader).build();
        Self { provider, exporter }
    }

    pub fn observer(&self) -> OtelObserver {
        OtelObserver::new(self.provider.meter("judgment-test"))
    }

    /// Everything recorded so far, by instrument name.
    pub fn read(&self) -> BTreeMap<String, Instrument> {
        self.provider.force_flush().unwrap();
        let exports = self.exporter.get_finished_metrics().unwrap();
        let mut out = BTreeMap::new();
        for scope in exports.last().unwrap().scope_metrics() {
            for metric in scope.metrics() {
                let points = match metric.data() {
                    AggregatedMetrics::F64(MetricData::Histogram(h)) => h
                        .data_points()
                        .map(|p| Point {
                            attrs: attrs(p.attributes()),
                            count: p.count(),
                            sum: p.sum(),
                        })
                        .collect(),
                    #[allow(clippy::cast_precision_loss)]
                    AggregatedMetrics::U64(MetricData::Histogram(h)) => h
                        .data_points()
                        .map(|p| Point {
                            attrs: attrs(p.attributes()),
                            count: p.count(),
                            sum: p.sum() as f64,
                        })
                        .collect(),
                    #[allow(clippy::cast_precision_loss)]
                    AggregatedMetrics::U64(MetricData::Sum(s)) => s
                        .data_points()
                        .map(|p| Point {
                            attrs: attrs(p.attributes()),
                            count: p.value(),
                            sum: p.value() as f64,
                        })
                        .collect(),
                    other => panic!("unexpected aggregation: {other:?}"),
                };
                out.insert(
                    metric.name().to_owned(),
                    Instrument {
                        unit: metric.unit().to_owned(),
                        points,
                    },
                );
            }
        }
        out
    }
}

pub fn attrs<'a>(
    list: impl Iterator<Item = &'a opentelemetry::KeyValue>,
) -> BTreeMap<String, String> {
    list.map(|kv| (kv.key.to_string(), kv.value.as_str().into_owned()))
        .collect()
}

pub fn attr_set(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

pub fn client(server: &MockServer, observer: OtelObserver) -> Client {
    Client::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .retry(RetryPolicy::none())
        .timeout(Duration::from_secs(2))
        .observer(Arc::new(observer))
        .build()
        .unwrap()
}

pub fn one_noul() -> Questions {
    let mut q = Questions::new();
    q.noul("x", "?", None).unwrap();
    q
}

pub fn noul_response(usage: &Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "model": "jev-1.13.0",
        "answers": { "x": { "type": "noul", "noul": 0.5 } },
        "usage": usage,
    }))
}

pub async fn serve(server: &MockServer, template: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(template)
        .mount(server)
        .await;
}
