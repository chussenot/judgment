//! `judgment.retry.attempts`, fed by the process-wide observer: a failed
//! attempt is reported there and never to a client's own observer.
//!
//! A target of its own, with one test: `observer::set_global` succeeds once
//! per process, and any other test in the binary would add its own failed
//! attempts to the counter (`tests/otel.rs` has the rest).
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod otel_rig;

use std::sync::Arc;
use std::time::Duration;

use judgment::semconv::{ERROR_TYPE, METRIC_RETRY_ATTEMPTS};
use judgment::{Client, RetryPolicy, observer};
use otel_rig::{Rig, attr_set, noul_response, one_noul, serve};
use serde_json::json;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn failed_attempts_reach_the_global_observer_as_retry_attempts() {
    let rig = Rig::new();
    assert!(observer::set_global(Arc::new(rig.observer())));
    let server = MockServer::start().await;
    // One 429, then a 200: the failed attempt counts though the call succeeds.
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    serve(
        &server,
        noul_response(&json!({ "input_tokens": 3, "output_tokens": 1 })),
    )
    .await;
    let retrying = RetryPolicy {
        max_retries: 1,
        backoff_initial: Duration::from_millis(1),
        backoff_max: Duration::from_millis(1),
        backoff_jitter: 0.0,
        ..RetryPolicy::default()
    };
    Client::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .retry(retrying)
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap()
        .system_one(&"s", &one_noul())
        .await
        .unwrap();
    let metrics = rig.read();

    let retries = &metrics[METRIC_RETRY_ATTEMPTS];
    assert_eq!(retries.unit, "{attempt}");
    assert_eq!(retries.points.len(), 1, "{retries:?}");
    assert_eq!(retries.points[0].attrs, attr_set(&[(ERROR_TYPE, "429")]));
    assert_eq!(retries.points[0].count, 1);
}
