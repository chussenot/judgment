//! The seam between this crate and an application's metrics.
//!
//! A library must not pick a metrics backend for the application that embeds
//! it, so the crate emits `tracing` spans and hands the two numbers an
//! application always wants, tokens per response and failed attempts per
//! upstream, to an [`Observer`]; the default is a no-op.
//!
//! Set one per client with [`crate::client::ClientBuilder::observer`], or
//! once per process with [`set_global`]. Token usage and a finished call go
//! to the client's own observer when it has one and to the global one
//! otherwise, the way `tracing` falls back to its global subscriber, and so
//! do the failed attempts the client's retry loop sees
//! ([`crate::http::send_with_retries_paced`] takes the observer to report
//! to; [`crate::http::send_with_retries`], with none, reports to the global
//! one).
//!
//! With the `otel` feature, [`otel::OtelObserver`] is an observer that
//! records OpenTelemetry metrics (`docs/guides/export-to-opentelemetry.md`).

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use crate::answer::{Answer, Usage};

#[cfg(feature = "otel")]
#[cfg_attr(docsrs, doc(cfg(feature = "otel")))]
pub mod otel;

/// One finished call to a server, as [`Observer::on_call`] reports it: what
/// was asked of whom, how long it took, what it cost and how it ended.
///
/// Everything is bounded or already the caller's: the provider is one of a
/// fixed set, the error type a status or a short name, the models are the
/// ones the call named and the server reported. Nothing from the state, the
/// questions' text or a request id is in it.
///
/// Non-exhaustive: a later release may add a field.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct CallReport<'a> {
    /// `typesafe`, `openrouter`, `ollama` or `other`, from the base URL's
    /// host. A Laya or autojev server on a local address reads as `other`.
    pub provider: &'static str,
    /// The base URL's host.
    pub server_address: &'a str,
    /// The model the request named, alias included.
    pub request_model: &'a str,
    /// The versioned model that answered, when a response came back, even
    /// one that did not fit the questions.
    pub response_model: Option<&'a str>,
    /// Tokens and cost of the response, when one came back: billed whether
    /// or not it fit, so present for an [`Error::is_unfit`](crate::Error::is_unfit)
    /// failure too.
    pub usage: Option<&'a Usage>,
    /// The verified answers, only when the call succeeded.
    pub answers: Option<&'a BTreeMap<String, Answer>>,
    /// Wall time from the call's start to its end, retries and waits
    /// included.
    pub duration: Duration,
    /// How the call failed, `None` when it did: an HTTP status as a number,
    /// or `transport`, `decode`, `unfit`, `too_large`, or an error's name in
    /// snake case ([`crate::semconv::ERROR_TYPE`]).
    pub error_type: Option<&'a str>,
}

/// What the client and the retry loop report.
pub trait Observer: Send + Sync + 'static {
    /// One response arrived: its model name and token usage.
    fn on_usage(&self, model: &str, usage: &Usage) {
        let _ = (model, usage);
    }

    /// One request with images is about to be sent to `model`: how many
    /// images and how many bytes of them together, before any base64.
    /// Reported once per call, whether or not the call then succeeds, so
    /// an application can count what it uploads the way it counts tokens
    /// (an image is billed as input tokens, which [`Observer::on_usage`]
    /// then reports among the rest). Nothing for a request without images.
    fn on_assets(&self, model: &str, images: usize, bytes: u64) {
        let _ = (model, images, bytes);
    }

    /// One attempt against `service` failed with `status`: an HTTP status
    /// code, `transport` (no response), `too_large` (body over the policy's
    /// cap), or, from the TypeSafe client, `decode` (a 2xx whose body did not
    /// decode) or `unfit` (a 2xx that did not fit the questions sent).
    /// Retried attempts are reported too: they are load whether or not a
    /// later attempt succeeds. An `unfit` response's usage is also reported
    /// through [`Observer::on_usage`], since it was billed.
    fn on_failed_attempt(&self, service: &'static str, status: &str) {
        let _ = (service, status);
    }

    /// One call finished, whichever way: after every retry, with its total
    /// duration, its outcome and, when a response came back, its tokens and
    /// cost. Reported once per call by the client, after
    /// [`Observer::on_usage`] and [`Observer::on_failed_attempt`] have seen
    /// the same call's parts, so a counter built from it counts calls and a
    /// counter built from those counts responses and attempts.
    fn on_call(&self, call: &CallReport<'_>) {
        let _ = call;
    }
}

/// Reports nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopObserver;

impl Observer for NoopObserver {}

static GLOBAL: OnceLock<Arc<dyn Observer>> = OnceLock::new();

/// Install the process-wide observer. The first call wins; later calls are
/// ignored and return `false`, so an application installs it once at start-up
/// and a library never overrides it.
pub fn set_global(observer: Arc<dyn Observer>) -> bool {
    GLOBAL.set(observer).is_ok()
}

/// The process-wide observer installed by [`set_global`], or a no-op.
pub fn global() -> Arc<dyn Observer> {
    static NOOP: OnceLock<Arc<dyn Observer>> = OnceLock::new();
    GLOBAL
        .get()
        .unwrap_or_else(|| NOOP.get_or_init(|| Arc::new(NoopObserver)))
        .clone()
}
