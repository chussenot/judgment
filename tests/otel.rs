//! What `OtelObserver` records for a client's calls, read back from the
//! SDK's in-memory metric exporter: the instruments' names and units, the
//! attribute sets of each data point, and what is deliberately absent (a
//! cost the server did not report, the confidence histogram until asked
//! for, an error type on a success).
//!
//! Every client here has its own observer. The retry counter is fed by the
//! process-wide observer instead, which is set once per process and would
//! also see these tests' failed attempts, so it has a target of its own
//! (`tests/otel_global.rs`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]

mod otel_rig;

use judgment::Questions;
use judgment::semconv::{
    ERROR_TYPE, GEN_AI_OPERATION_NAME, GEN_AI_PROVIDER_NAME, GEN_AI_REQUEST_MODEL,
    GEN_AI_RESPONSE_MODEL, GEN_AI_TOKEN_TYPE, METRIC_ANSWER_CONFIDENCE, METRIC_OPERATION_DURATION,
    METRIC_RETRY_ATTEMPTS, METRIC_TOKEN_USAGE, METRIC_USAGE_COST, QUESTION_ID, SERVER_ADDRESS,
};
use otel_rig::{Rig, attr_set, client, noul_response, one_noul, serve};
use serde_json::json;
use wiremock::{MockServer, ResponseTemplate};

/// The attributes every call's instruments share.
fn call_attrs() -> [(&'static str, &'static str); 3] {
    [
        (GEN_AI_OPERATION_NAME, "system_one"),
        (GEN_AI_PROVIDER_NAME, "other"),
        (GEN_AI_REQUEST_MODEL, "jev-latest"),
    ]
}

#[tokio::test]
async fn a_call_records_tokens_duration_and_cost() {
    let rig = Rig::new();
    let server = MockServer::start().await;
    serve(
        &server,
        noul_response(&json!({ "input_tokens": 3, "output_tokens": 1, "cost": 0.0025 })),
    )
    .await;
    client(&server, rig.observer())
        .system_one(&"s", &one_noul())
        .await
        .unwrap();
    let metrics = rig.read();

    let tokens = &metrics[METRIC_TOKEN_USAGE];
    assert_eq!(tokens.unit, "{token}");
    assert_eq!(tokens.points.len(), 2, "{tokens:?}");
    for (kind, total) in [("input", 3.0), ("output", 1.0)] {
        let mut want = attr_set(&call_attrs());
        want.insert(GEN_AI_RESPONSE_MODEL.into(), "jev-1.13.0".into());
        want.insert(GEN_AI_TOKEN_TYPE.into(), kind.into());
        let point = tokens.points.iter().find(|p| p.attrs == want).unwrap();
        assert_eq!((point.count, point.sum), (1, total), "{kind}");
    }

    let duration = &metrics[METRIC_OPERATION_DURATION];
    assert_eq!(duration.unit, "s");
    assert_eq!(duration.points.len(), 1);
    let mut want = attr_set(&call_attrs());
    want.insert(SERVER_ADDRESS.into(), "127.0.0.1".into());
    assert_eq!(duration.points[0].attrs, want, "no error.type on a success");
    assert_eq!(duration.points[0].count, 1);
    assert!(duration.points[0].sum > 0.0);

    let cost = &metrics[METRIC_USAGE_COST];
    assert_eq!(cost.unit, "USD");
    assert_eq!(cost.points.len(), 1);
    assert_eq!(cost.points[0].attrs, attr_set(&call_attrs()));
    assert!((cost.points[0].sum - 0.0025).abs() < 1e-12);

    // Nothing switched on or fed by a failure.
    for absent in [METRIC_ANSWER_CONFIDENCE, METRIC_RETRY_ATTEMPTS] {
        assert!(
            !metrics.contains_key(absent),
            "{absent} recorded: {metrics:?}"
        );
    }
}

#[tokio::test]
async fn a_cost_the_server_does_not_report_is_not_recorded() {
    let rig = Rig::new();
    let server = MockServer::start().await;
    serve(
        &server,
        noul_response(&json!({ "input_tokens": 3, "output_tokens": 1 })),
    )
    .await;
    client(&server, rig.observer())
        .system_one(&"s", &one_noul())
        .await
        .unwrap();
    let metrics = rig.read();
    assert!(metrics.contains_key(METRIC_TOKEN_USAGE));
    assert!(!metrics.contains_key(METRIC_USAGE_COST), "{metrics:?}");
}

#[tokio::test]
async fn a_failed_call_records_its_duration_with_the_error_type() {
    let rig = Rig::new();
    let server = MockServer::start().await;
    serve(&server, ResponseTemplate::new(429)).await;
    client(&server, rig.observer())
        .system_one(&"s", &one_noul())
        .await
        .unwrap_err();
    let metrics = rig.read();

    let duration = &metrics[METRIC_OPERATION_DURATION];
    assert_eq!(duration.points.len(), 1);
    let mut want = attr_set(&call_attrs());
    want.insert(SERVER_ADDRESS.into(), "127.0.0.1".into());
    want.insert(ERROR_TYPE.into(), "rate_limited".into());
    assert_eq!(duration.points[0].attrs, want);
    // No response came back, so no tokens and no cost.
    assert!(!metrics.contains_key(METRIC_TOKEN_USAGE), "{metrics:?}");
    assert!(!metrics.contains_key(METRIC_USAGE_COST), "{metrics:?}");
}

#[tokio::test]
async fn a_response_that_does_not_fit_is_billed_and_fails() {
    let rig = Rig::new();
    let server = MockServer::start().await;
    // Decodes, but answers no asked question: the tokens were spent.
    serve(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({
            "model": "jev-1.13.0",
            "answers": {},
            "usage": { "input_tokens": 7, "output_tokens": 2 },
        })),
    )
    .await;
    let error = client(&server, rig.observer())
        .system_one(&"s", &one_noul())
        .await
        .unwrap_err();
    assert!(error.is_unfit(), "{error}");
    let metrics = rig.read();

    let tokens = &metrics[METRIC_TOKEN_USAGE];
    let total: f64 = tokens.points.iter().map(|p| p.sum).sum();
    assert!((total - 9.0).abs() < f64::EPSILON, "{tokens:?}");
    let duration = &metrics[METRIC_OPERATION_DURATION];
    assert_eq!(duration.points[0].attrs[ERROR_TYPE], "unfit");
}

#[tokio::test]
async fn confidence_is_recorded_per_question_only_when_asked_for() {
    let mut questions = Questions::new();
    questions
        .dynamic_choice(
            "desk",
            "?",
            [("billing".to_owned(), None), ("sales".to_owned(), None)],
        )
        .unwrap();
    let body = json!({
        "model": "jev-1.13.0",
        "answers": { "desk": {
            "type": "choice",
            "choice": "billing",
            "probabilities": { "billing": 0.9, "sales": 0.1 },
            "confidence": 0.8,
        } },
        "usage": { "input_tokens": 3, "output_tokens": 1 },
    });

    for (asked, recorded) in [(false, false), (true, true)] {
        let rig = Rig::new();
        let server = MockServer::start().await;
        serve(&server, ResponseTemplate::new(200).set_body_json(&body)).await;
        client(&server, rig.observer().answer_confidence(asked))
            .system_one(&"s", &questions)
            .await
            .unwrap();
        let metrics = rig.read();
        assert_eq!(
            metrics.contains_key(METRIC_ANSWER_CONFIDENCE),
            recorded,
            "asked: {asked}"
        );
        if recorded {
            let confidence = &metrics[METRIC_ANSWER_CONFIDENCE];
            assert_eq!(confidence.unit, "1");
            assert_eq!(confidence.points.len(), 1);
            assert_eq!(
                confidence.points[0].attrs,
                attr_set(&[(GEN_AI_REQUEST_MODEL, "jev-latest"), (QUESTION_ID, "desk")])
            );
            assert!((confidence.points[0].sum - 0.8).abs() < f64::EPSILON);
        }
    }
}
