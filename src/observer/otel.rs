//! An [`Observer`] that records OpenTelemetry metrics, through the
//! OpenTelemetry API only: the application owns the SDK, the readers and the
//! exporter, and hands this a [`Meter`] from its own provider.
//!
//! | Instrument | Kind, unit | Attributes |
//! |---|---|---|
//! | `gen_ai.client.token.usage` | histogram `{token}` | operation, provider, request model, response model, `gen_ai.token.type` |
//! | `gen_ai.client.operation.duration` | histogram `s` | operation, provider, request model, `server.address`, `error.type` on failure |
//! | `judgment.retry.attempts` | counter `{attempt}` | `error.type` |
//! | `judgment.usage.cost` | histogram `USD`, only when the server reports a cost | operation, provider, request model |
//! | `judgment.answer.confidence` | histogram `1`, only when switched on | request model, question id |
//!
//! The names and attributes are in [`crate::semconv`]. The `GenAI` conventions
//! are at Development status in OpenTelemetry, so a name can still change
//! upstream. Every attribute has a bounded set of values (a provider is one
//! of four, an error type a status or a short name, a model is what the
//! caller names) except the question id on the confidence histogram, which
//! is the caller's own string and so off until asked for
//! ([`OtelObserver::answer_confidence`]). A request id and the state's
//! content are never an attribute.
//!
//! A client reports its failed attempts to its own observer when it has one
//! and to the process-wide one ([`crate::observer::set_global`]) otherwise,
//! so `judgment.retry.attempts` counts a client's attempts either way. A
//! loop run directly through [`crate::http::send_with_retries`] has no
//! client and reports to the global observer.

use opentelemetry::KeyValue;
use opentelemetry::metrics::{Counter, Histogram, Meter};

use super::{CallReport, Observer};
use crate::answer::Answer;
use crate::semconv::{
    ERROR_TYPE, GEN_AI_OPERATION_NAME, GEN_AI_PROVIDER_NAME, GEN_AI_REQUEST_MODEL,
    GEN_AI_RESPONSE_MODEL, GEN_AI_TOKEN_TYPE, METRIC_ANSWER_CONFIDENCE, METRIC_OPERATION_DURATION,
    METRIC_RETRY_ATTEMPTS, METRIC_TOKEN_USAGE, METRIC_USAGE_COST, OPERATION_SYSTEM_ONE,
    QUESTION_ID, SERVER_ADDRESS, TOKEN_TYPE_INPUT, TOKEN_TYPE_OUTPUT,
};

/// The explicit bucket bounds the `GenAI` conventions recommend: seconds for a
/// duration, powers of four for a token count.
const DURATION_BOUNDS: [f64; 14] = [
    0.01, 0.02, 0.04, 0.08, 0.16, 0.32, 0.64, 1.28, 2.56, 5.12, 10.24, 20.48, 40.96, 81.92,
];
const TOKEN_BOUNDS: [f64; 14] = [
    1.0,
    4.0,
    16.0,
    64.0,
    256.0,
    1024.0,
    4096.0,
    16384.0,
    65536.0,
    262_144.0,
    1_048_576.0,
    4_194_304.0,
    16_777_216.0,
    67_108_864.0,
];

/// Records a client's calls as OpenTelemetry metrics (module docs).
#[derive(Debug, Clone)]
pub struct OtelObserver {
    meter: Meter,
    token_usage: Histogram<u64>,
    duration: Histogram<f64>,
    retries: Counter<u64>,
    cost: Histogram<f64>,
    confidence: Option<Histogram<f64>>,
}

impl OtelObserver {
    /// An observer recording to instruments created on `meter`, which comes
    /// from the application's own `MeterProvider`.
    #[must_use]
    pub fn new(meter: Meter) -> Self {
        Self {
            token_usage: meter
                .u64_histogram(METRIC_TOKEN_USAGE)
                .with_unit("{token}")
                .with_description("Tokens in a call's state and questions, and in its answers.")
                .with_boundaries(TOKEN_BOUNDS.to_vec())
                .build(),
            duration: meter
                .f64_histogram(METRIC_OPERATION_DURATION)
                .with_unit("s")
                .with_description("Wall time of a call, retries and waits included.")
                .with_boundaries(DURATION_BOUNDS.to_vec())
                .build(),
            retries: meter
                .u64_counter(METRIC_RETRY_ATTEMPTS)
                .with_unit("{attempt}")
                .with_description("Attempts that failed, whether or not a later one succeeded.")
                .build(),
            cost: meter
                .f64_histogram(METRIC_USAGE_COST)
                .with_unit("USD")
                .with_description("What a call cost, when the server reports it.")
                .build(),
            confidence: None,
            meter,
        }
    }

    /// Also record the confidence of every Choice and Score answer of a
    /// successful call, labelled by question id. Off by default: an id is
    /// the caller's own string, so it is a metric label only when the caller
    /// knows its ids are few and fixed.
    #[must_use]
    pub fn answer_confidence(mut self, on: bool) -> Self {
        self.confidence = on.then(|| {
            self.meter
                .f64_histogram(METRIC_ANSWER_CONFIDENCE)
                .with_unit("1")
                .with_description("Concentration of a Choice or Score answer's distribution.")
                .build()
        });
        self
    }
}

impl Observer for OtelObserver {
    fn on_failed_attempt(&self, _service: &'static str, status: &str) {
        self.retries
            .add(1, &[KeyValue::new(ERROR_TYPE, status.to_owned())]);
    }

    fn on_call(&self, call: &CallReport<'_>) {
        let mut attrs = vec![
            KeyValue::new(GEN_AI_OPERATION_NAME, OPERATION_SYSTEM_ONE),
            KeyValue::new(GEN_AI_PROVIDER_NAME, call.provider),
            KeyValue::new(GEN_AI_REQUEST_MODEL, call.request_model.to_owned()),
        ];

        let mut timed = attrs.clone();
        timed.push(KeyValue::new(
            SERVER_ADDRESS,
            call.server_address.to_owned(),
        ));
        if let Some(kind) = call.error_type {
            timed.push(KeyValue::new(ERROR_TYPE, kind.to_owned()));
        }
        self.duration.record(call.duration.as_secs_f64(), &timed);

        if let Some(usage) = call.usage {
            if let Some(cost) = usage.cost {
                self.cost.record(cost, &attrs);
            }
            if let Some(model) = call.response_model {
                attrs.push(KeyValue::new(GEN_AI_RESPONSE_MODEL, model.to_owned()));
            }
            for (kind, tokens) in [
                (TOKEN_TYPE_INPUT, usage.input_tokens),
                (TOKEN_TYPE_OUTPUT, usage.output_tokens),
            ] {
                attrs.push(KeyValue::new(GEN_AI_TOKEN_TYPE, kind));
                self.token_usage.record(tokens, &attrs);
                attrs.pop();
            }
        }

        if let (Some(histogram), Some(answers)) = (&self.confidence, call.answers) {
            for (id, answer) in answers {
                if let Answer::Choice { confidence, .. } | Answer::Score { confidence, .. } = answer
                {
                    histogram.record(
                        confidence.value(),
                        &[
                            KeyValue::new(GEN_AI_REQUEST_MODEL, call.request_model.to_owned()),
                            KeyValue::new(QUESTION_ID, id.clone()),
                        ],
                    );
                }
            }
        }
    }
}
