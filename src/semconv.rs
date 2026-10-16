//! The OpenTelemetry names this crate writes, in one place: the attribute
//! names of the `gen_ai.*` semantic conventions
//! (<https://opentelemetry.io/docs/specs/semconv/gen-ai/>) and of the
//! general `server.*` and `error.type` ones, and the names of the metrics
//! [`crate::observer::otel::OtelObserver`] records.
//!
//! The `GenAI` conventions are at Development status in OpenTelemetry, so a
//! name here can still change upstream; this module is the one edit point
//! when it does. The `tracing` macros take a field name as a literal, not a
//! constant, so the client's span spells the same strings out, and a test
//! (`tests/spans.rs`) holds the two together.
//!
//! `system_one` is not one of the conventions' operation names (`chat`,
//! `embeddings`, ...): a System One call evaluates typed questions and
//! generates no text, so it has an operation of its own.

/// `gen_ai.operation.name` on every span and metric.
pub const GEN_AI_OPERATION_NAME: &str = "gen_ai.operation.name";
/// The one value this crate gives it: see the module docs.
pub const OPERATION_SYSTEM_ONE: &str = "system_one";
/// `gen_ai.provider.name`: `typesafe`, `openrouter`, `ollama` or `other`,
/// from the server's address. A fixed set, so it is safe as a metric label.
pub const GEN_AI_PROVIDER_NAME: &str = "gen_ai.provider.name";
/// `gen_ai.request.model`: the model the request named, alias included.
pub const GEN_AI_REQUEST_MODEL: &str = "gen_ai.request.model";
/// `gen_ai.response.model`: the versioned model that answered.
pub const GEN_AI_RESPONSE_MODEL: &str = "gen_ai.response.model";
/// `gen_ai.usage.input_tokens`.
pub const GEN_AI_USAGE_INPUT_TOKENS: &str = "gen_ai.usage.input_tokens";
/// `gen_ai.usage.output_tokens`.
pub const GEN_AI_USAGE_OUTPUT_TOKENS: &str = "gen_ai.usage.output_tokens";
/// `gen_ai.token.type`, a metric attribute.
pub const GEN_AI_TOKEN_TYPE: &str = "gen_ai.token.type";
/// The `gen_ai.token.type` of a prompt's tokens.
pub const TOKEN_TYPE_INPUT: &str = "input";
/// The `gen_ai.token.type` of an answer's tokens.
pub const TOKEN_TYPE_OUTPUT: &str = "output";
/// `server.address`: the host the client talks to.
pub const SERVER_ADDRESS: &str = "server.address";
/// `error.type`: an HTTP status as a number (`429`), or one of this crate's
/// own short names (`transport`, `decode`, `unfit`, `too_large`, and an
/// error variant's name in snake case). Bounded, so safe as a metric label.
pub const ERROR_TYPE: &str = "error.type";

/// The histogram of tokens per call, by `gen_ai.token.type`.
pub const METRIC_TOKEN_USAGE: &str = "gen_ai.client.token.usage";
/// The histogram of a call's duration in seconds.
pub const METRIC_OPERATION_DURATION: &str = "gen_ai.client.operation.duration";
/// The counter of failed attempts, retried or not, by `error.type`.
pub const METRIC_RETRY_ATTEMPTS: &str = "judgment.retry.attempts";
/// The histogram of what a call cost in US dollars, when the server says.
pub const METRIC_USAGE_COST: &str = "judgment.usage.cost";
/// The histogram of the confidence of a Choice or Score answer, by question.
pub const METRIC_ANSWER_CONFIDENCE: &str = "judgment.answer.confidence";
/// The question id on [`METRIC_ANSWER_CONFIDENCE`].
pub const QUESTION_ID: &str = "judgment.question.id";
