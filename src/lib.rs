//! # judgment
//!
//! Typed, calibrated judgments from [TypeSafe](https://docs.typesafe.ai)
//! System One models (Jev) and from any backend that speaks the same wire.
//!
//! A System One model does not generate text. It evaluates a `state` (any
//! JSON, images included) against typed questions and returns calibrated
//! answers: a [`Noul`] is a probability of yes, a [`Choice`] a chosen
//! option with its distribution and a [`Confidence`], a [`Score`] a
//! weighted position on ordered levels, a [`BoundingBox`] where something
//! is in an image. The threshold that turns a number into an action stays in
//! code, and [`eval`] measures the probabilities against outcomes. The
//! README says why that matters and what the crate guarantees; `docs/` says
//! how.
//!
//! ## The shape of a call
//!
//! [`Questions`] builds a request, and each question returns a [`Handle`]
//! whose type parameter is its answer's type. A [`SystemOne`] backend
//! ([`Client`] over HTTP, [`Fake`] in a test, [`Replay`] and [`Recorder`]
//! over recordings) answers it with a [`Response`], verified against the
//! questions it was sent before it is returned ([`Response::verify`]), and
//! [`Response::get`] reads an answer through its handle. A mismatch anywhere
//! is an [`Error`], grouped by what fixes it, never a misread number.
//!
//! ```no_run
//! # // The client needs the `http` feature; without it this block compiles to nothing.
//! # #[cfg(feature = "http")]
//! # mod example {
//! use judgment::{Client, Questions, options};
//!
//! options! {
//!     enum Department {
//!         Billing = "billing" => "Payments, invoicing, refunds",
//!         Technical = "technical" => "Bugs, outages, integrations",
//!         Sales = "sales" => "Pricing, upgrades, new accounts",
//!     }
//! }
//!
//! # async fn run() -> judgment::Result<()> {
//! let mut questions = Questions::new();
//! let dept = questions.choice::<Department>("department", "Which team should handle `message`?")?;
//! let urgent = questions.noul("is_urgent", "Does `message` convey urgency?", None)?;
//!
//! let client = Client::from_env()?;
//! let state = serde_json::json!({ "message": "Help! My payouts have been failing for 3 days." });
//! let response = client.system_one(&state, &questions).await?;
//!
//! let dept = response.get(&dept)?;          // Choice<Department>
//! let urgent = response.get(&urgent)?;      // Noul
//! if dept.chosen == Department::Billing && dept.confidence.at_least(0.7) && urgent.is_yes(0.6) {
//!     // page billing on-call
//! }
//! # Ok(()) }
//! # }
//! ```
//!
//! ## Features and modules
//!
//! * `http` (default): [`client`] and the [`http`] retry loop, with the
//!   official SDKs' defaults and retries ([`RetryPolicy`] states where they
//!   differ); the loop is public so another client over `reqwest` can share
//!   it. Without the feature the crate is [`question`], [`answer`], the
//!   other [`backend`]s, [`error`], [`eval`] (recordings, graded judgments,
//!   accuracy, Brier score and calibration error) and [`observer`] (the seam
//!   for an application's own metrics), for a project that brings its own
//!   transport.
//! * `openapi`: `contract`, the vendored TypeSafe OpenAPI document as text.
//! * `jud`: `jud`, the `.jud` format for rubrics, cases and recordings
//!   (`docs/reference/jud-format.md`).
//! * `pacer` (implied by `http`): [`Pacer`] and [`Paced`], a bound on calls
//!   in flight and one pause every call on it honours, which a client given
//!   the pacer ([`client::ClientBuilder::pacer`]) sets from a 429's named wait
//!   (`docs/guides/bound-a-stream.md`).
//! * `otel`: [`observer::otel::OtelObserver`], an [`Observer`] that records
//!   OpenTelemetry metrics through the OpenTelemetry API
//!   (`docs/guides/export-to-opentelemetry.md`). The names it and the
//!   client's spans use are in [`semconv`].
//!
//! The runnable examples under `examples/` cover the four patterns TypeSafe
//! documents (<https://docs.typesafe.ai/patterns>); `docs/guides/patterns.md` says
//! how they are run and re-recorded.

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod answer;
pub mod asset;
pub mod backend;
#[cfg(feature = "http")]
#[cfg_attr(docsrs, doc(cfg(feature = "http")))]
pub mod client;
#[cfg(feature = "openapi")]
#[cfg_attr(docsrs, doc(cfg(feature = "openapi")))]
pub mod contract;
pub mod error;
pub mod eval;
#[cfg(feature = "http")]
#[cfg_attr(docsrs, doc(cfg(feature = "http")))]
pub mod http;
#[cfg(feature = "jud")]
#[cfg_attr(docsrs, doc(cfg(feature = "jud")))]
pub mod jud;
pub mod observer;
pub mod question;
pub mod semconv;

pub use answer::{
    Answer, BoundingBox, BoxEdges, Choice, Confidence, FromAnswer, Noul, PixelBox, Probability,
    Response, Score, Usage,
};
pub use asset::{Assets, Image, ImageFormat};
pub use backend::{Fake, Recorder, Replay, SystemOne};
#[cfg(feature = "pacer")]
#[cfg_attr(docsrs, doc(cfg(feature = "pacer")))]
pub use backend::{Paced, Pacer};
#[cfg(feature = "http")]
#[cfg_attr(docsrs, doc(cfg(feature = "http")))]
pub use client::{
    CallOptions, Client, ClientBuilder, ModelInfo, Request, RetryPolicy, TransportRetry,
};
pub use error::{Error, Result, ValidationIssue};
pub use observer::Observer;
pub use question::{Handle, NoulCriteria, Options, Question, Questions};

// The guides' Rust fragments compile as documentation tests, so a page cannot
// show code the crate does not accept (docs/project/contributing.md). A
// tutorial's whole programs are `examples/` files held by
// `tests/docs_examples.rs` instead.
#[cfg(all(doctest, feature = "http"))]
#[doc = include_str!("../docs/guides/record-replay-and-test.md")]
pub struct DocsRecordReplayAndTest;
#[cfg(all(doctest, feature = "http"))]
#[doc = include_str!("../docs/guides/configure-a-backend.md")]
pub struct DocsConfigureABackend;
#[cfg(all(doctest, feature = "http"))]
#[doc = include_str!("../docs/guides/ask-about-an-image.md")]
pub struct DocsAskAboutAnImage;
#[cfg(all(doctest, feature = "http"))]
#[doc = include_str!("../docs/guides/locate-with-a-box.md")]
pub struct DocsLocateWithABox;
#[cfg(all(doctest, feature = "http"))]
#[doc = include_str!("../docs/guides/use-long-context.md")]
pub struct DocsUseLongContext;
#[cfg(all(doctest, feature = "http", feature = "otel"))]
#[doc = include_str!("../docs/guides/export-to-opentelemetry.md")]
pub struct DocsExportToOpenTelemetry;
#[cfg(all(doctest, feature = "http"))]
#[doc = include_str!("../docs/guides/bound-a-stream.md")]
pub struct DocsBoundAStream;
