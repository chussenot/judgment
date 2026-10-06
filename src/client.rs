//! HTTP client for `POST /v1/systemone` and `GET /v1/models`.
//!
//! The defaults are the official SDKs': the key from `TYPESAFE_API_KEY`,
//! base URL `https://api.typesafe.ai`, model `jev-latest`, a 10 s timeout
//! per attempt, two retries with backoff and jitter ([`RetryPolicy`] says
//! where the crate deliberately differs). The user agent is
//! `judgment/<crate version>`, so the API's logs can tell this client from
//! the SDKs and from the application embedding it.
//!
//! The client follows no redirect, like the Python SDK: neither path ever
//! redirects, so a 3xx is a wrong base URL, and following it would send a
//! gateway header and, on a 307 or 308, the caller's state to wherever
//! `Location` named. A 3xx is [`Error::Http`], not retried.
//!
//! The key is checked when the client is built, with the Python SDK's
//! (0.7.1) rules ([`ClientBuilder::build`]). A key passed to the builder is
//! never replaced by the environment's, so a blank one is an error rather
//! than a silent switch of account. The key is marked sensitive, redacted
//! from every `Debug` output and quoted by no error.
//!
//! # Retries and errors
//!
//! Every HTTP call goes through [`crate::http::send_with_retries`] under the
//! client's [`RetryPolicy`] or the call's. The final response is classified
//! by status into [`Error`], grouped by what fixes it: 400, 413 and 422 are
//! [`Error::InvalidRequest`], with the server's message, the fields a
//! validation body names as [`ValidationIssue`]s and the code a hosted-API
//! 400 carries as `detail.error_type`; 401 is [`Error::Unauthorized`] and
//! 403 [`Error::PermissionDenied`], apart because a new key does not fix
//! it; 429 is [`Error::RateLimited`] and 529 [`Error::Overloaded`] once the
//! policy stopped; anything else is [`Error::Http`], a 404 included, since
//! neither path carries a resource id and a 404 is always a base URL that
//! is not the API or a server without the path.
//!
//! A 2xx that does not decode is [`Error::Decode`]; one that does not
//! answer the questions sent is what [`Response::verify`] names
//! ([`Error::is_unfit`]). Neither is retried: the call was billed, and a
//! second attempt is billed again for an answer no likelier to fit. Both
//! are reported to the process-wide [`Observer`] as a failed attempt
//! (`decode`, `unfit`), because the retry loop counted the 2xx as a
//! success; an unfit response's usage is reported too, since the tokens
//! were spent.
//!
//! What the API may add is tolerated (`answer` module docs, `# Decoding is
//! tolerant, reading is strict`). An answer of a kind this release does not
//! know is logged once at `warn` inside the `typesafe.evaluate` span; under
//! an id that was not asked it is kept as [`Answer::Unknown`], under an
//! asked id it is [`Error::AnswerTypeMismatch`]. The Python SDK skips the
//! answer and returns the rest. [`Replay`](crate::Replay), [`Fake`](crate::Fake)
//! and [`crate::eval::read_recording`] do not warn: a recorded answer was
//! warned about when the client received it.
//!
//! # Request id
//!
//! TypeSafe identifies a call by [`REQUEST_ID_HEADER`], the one link from a
//! failed call to its own logs. It is read here, not in the shared loop,
//! which stays free of any vendor's header names, and lands on
//! [`Response::request_id`], on every HTTP-derived [`Error::request_id`]
//! (`error` module docs, `# Request id`) and on the `request_id` field of
//! the `typesafe.evaluate` and `typesafe.list_models` spans. It is the last
//! attempt's id, absent after a transport failure, and optional everywhere,
//! like the JS SDK's `requestId`, because the OpenAPI document lists no
//! response headers. A value that is empty, not printable ASCII or longer
//! than 256 bytes is ignored, so what reaches a span stays bounded.
//!
//! # Per-call options
//!
//! [`Client::evaluate_with`] takes a [`CallOptions`] for one call (a
//! per-attempt timeout, a retry policy, headers, extra top-level body
//! fields), so one client serves a batch job and an interactive request
//! alike; [`ClientBuilder::default_header`] sets a header on every call.
//! The model is not an option: it is on the [`Request`], and a second place
//! to set it would let the span's `model` field name a model that was not
//! sent. No option's value is a span field either, since a header can be a
//! credential and an extra field a piece of the state.
//!
//! Precedence, lowest first: the builder's settings and default headers,
//! the call's options, then what the client owns, which is refused before
//! anything is sent rather than overwritten ([`Error::ReservedHeader`],
//! [`Error::ReservedField`]; the names are on [`CallOptions::header`] and
//! [`CallOptions::extra`]). The SDKs silently keep their own headers, and
//! the Python SDK's `extra_body` merges last-write-wins, so an extra
//! `questions` replaces the questions; here that would send a request the
//! response is not verified against, and a per-call `authorization` would
//! authenticate as another account. The headers HTTP owns are refused
//! because the stack keeps a caller's value over its own (a short
//! `content-length` truncates the body; a `host` sends the key to another
//! virtual host). Refusing a name now and allowing it later breaks nobody.
//!
//! The options stop at the [`SystemOne`](crate::SystemOne) trait: a
//! recording is filed under [`crate::eval::request_hash`] of the state and
//! the questions, and an extra field can change the answer, so extras could
//! cross the trait only by entering that hash.
//!
//! # `GET /v1/models`
//!
//! [`Client::list_models`] lists the models and aliases the account may
//! send. The endpoint is in the OpenAPI document and both SDKs, but a
//! compatible server may not serve it, and then the call is [`Error::Http`]
//! with status 404. Nothing else in the crate depends on it.

use std::env::VarError;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{
    AUTHORIZATION, CONNECTION, CONTENT_LENGTH, CONTENT_TYPE, HOST, HeaderMap, TE,
    TRANSFER_ENCODING, UPGRADE, USER_AGENT,
};
use reqwest::{RequestBuilder, StatusCode, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::answer::{Answer, Response, sanitize_server_str};
use crate::error::{Error, Result, ValidationIssue};
use crate::http::{self, Completed, Exhausted};
use crate::observer::Observer;
use crate::question::Questions;

/// Environment variable holding the API key, the one secret the client reads
/// itself.
pub const API_KEY_ENV: &str = "TYPESAFE_API_KEY";
/// Production API base URL.
pub const DEFAULT_BASE_URL: &str = "https://api.typesafe.ai";
/// Alias for the current stable release.
pub const DEFAULT_MODEL: &str = "jev-latest";
/// Default per-attempt timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
/// Response header carrying TypeSafe's id for the request (module docs,
/// `# Request id`).
pub const REQUEST_ID_HEADER: &str = "x-typesafe-request-id";
/// Longest request id kept: generous for an identifier (a UUID is 36 bytes),
/// short enough that a broken header cannot flood every span and message.
const REQUEST_ID_MAX_LEN: usize = 256;

pub use crate::http::{RetryPolicy, TransportRetry};
/// The header types [`CallOptions::header`] and
/// [`ClientBuilder::default_header`] take, re-exported so a caller needs no
/// direct dependency on `reqwest`.
pub use reqwest::header::{HeaderName, HeaderValue};

/// Sent and stripped from a caller's headers by both official SDKs; reserved
/// before this crate sends it, so that sending it later breaks no caller.
const RETRY_COUNT_HEADER: HeaderName = HeaderName::from_static("x-typesafe-retry-count");
/// Headers the client sets itself, refused as a per-call or default header.
const RESERVED_HEADERS: [HeaderName; 4] =
    [AUTHORIZATION, CONTENT_TYPE, USER_AGENT, RETRY_COUNT_HEADER];
/// Headers HTTP itself owns, refused because the stack would keep a caller's
/// value over the one it computes.
const TRANSPORT_HEADERS: [HeaderName; 6] = [
    CONTENT_LENGTH,
    TRANSFER_ENCODING,
    HOST,
    CONNECTION,
    TE,
    UPGRADE,
];
/// Body fields set from the [`Request`], refused as extra fields.
const RESERVED_FIELDS: [&str; 3] = ["state", "model", "questions"];

/// The body of `POST /v1/systemone`.
#[derive(Debug, Clone, Serialize)]
pub struct Request<'a, S: Serialize> {
    /// Content to evaluate: string, object or array.
    pub state: &'a S,
    /// Model name or alias.
    pub model: &'a str,
    /// The questions.
    pub questions: &'a Questions,
}

/// One entry from `GET /v1/models`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Name accepted by the `model` field.
    pub name: String,
    /// What the model is for.
    pub description: String,
    /// Release date, `YYYY-MM-DD` per the OpenAPI document, kept as the
    /// string sent because the hosted API sends an RFC 3339 timestamp
    /// instead (`docs/verification/hosted-typesafe.md`).
    pub release_date: String,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    models: Vec<ModelInfo>,
}

/// What one call to [`Client::evaluate_with`] changes from the client's own
/// settings (module docs, `# Per-call options`). Empty by default; anything
/// the client or HTTP sets itself is refused when it is added, so a set that
/// was built can always be sent as it reads.
///
/// ```
/// use std::time::Duration;
/// use judgment::client::{CallOptions, HeaderName, HeaderValue};
/// use judgment::RetryPolicy;
///
/// # fn main() -> judgment::Result<()> {
/// let options = CallOptions::new()
///     .timeout(Duration::from_secs(30))
///     .retry(RetryPolicy::conservative())
///     .header(
///         HeaderName::from_static("x-team"),
///         HeaderValue::from_static("billing"),
///     )?
///     .extra("beam_width", 4)?;
/// assert!(CallOptions::new().extra("model", "other").is_err());
/// # let _ = options;
/// # Ok(()) }
/// ```
///
/// `Debug` prints the names of the headers and extra fields, never their
/// values, which may be a credential or a piece of the state.
#[derive(Clone, Default)]
pub struct CallOptions {
    timeout: Option<Duration>,
    retry: Option<RetryPolicy>,
    headers: HeaderMap,
    extra: Map<String, Value>,
}

impl fmt::Debug for CallOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallOptions")
            .field("timeout", &self.timeout)
            .field("retry", &self.retry)
            .field("headers", &header_names(&self.headers))
            .field("extra", &self.extra.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl CallOptions {
    /// No options: what [`Client::evaluate`] sends.
    pub fn new() -> Self {
        Self::default()
    }

    /// The per-attempt timeout of this call, in place of the client's (not
    /// the shorter of the two, so a call may be given longer). A
    /// [`RetryPolicy::budget`] never cuts an attempt in flight; wrap the call
    /// in `tokio::time::timeout` for a hard deadline.
    #[must_use]
    pub fn timeout(mut self, per_attempt: Duration) -> Self {
        self.timeout = Some(per_attempt);
        self
    }

    /// The retry policy of this call, in place of the client's whole policy,
    /// as in the Python SDK. The JS SDK merges a partial policy field by
    /// field; the Rust spelling of that is struct update syntax over the
    /// client's own policy:
    ///
    /// ```
    /// # use judgment::{Client, RetryPolicy, client::CallOptions};
    /// # fn run(client: &Client) {
    /// let once_more = CallOptions::new().retry(RetryPolicy {
    ///     max_retries: 1,
    ///     ..client.retry().clone()
    /// });
    /// # let _ = once_more; }
    /// ```
    #[must_use]
    pub fn retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = Some(policy);
        self
    }

    /// A header sent on every attempt of this call, replacing a
    /// [`ClientBuilder::default_header`] of the same (case-insensitive) name.
    ///
    /// `authorization`, `content-type`, `user-agent`,
    /// `x-typesafe-retry-count`, `content-length`, `transfer-encoding`,
    /// `host`, `connection`, `te` and `upgrade` are [`Error::ReservedHeader`]
    /// (module docs, `# Per-call options`). Mark a secret value with
    /// [`HeaderValue::set_sensitive`] so `reqwest` redacts it.
    pub fn header(mut self, name: HeaderName, value: HeaderValue) -> Result<Self> {
        refuse_reserved_header(&name)?;
        self.headers.insert(name, value);
        Ok(self)
    }

    /// A top-level body field for a server parameter this crate does not
    /// model; the server may use, ignore or refuse it. `null` is sent as
    /// `null`. `state`, `model` and `questions` are [`Error::ReservedField`],
    /// matched exactly.
    pub fn extra(mut self, name: impl Into<String>, value: impl Into<Value>) -> Result<Self> {
        let name = name.into();
        if RESERVED_FIELDS.contains(&name.as_str()) {
            return Err(Error::ReservedField(name));
        }
        self.extra.insert(name, value.into());
        Ok(self)
    }

    /// Called inside the retry loop's `make`, so every attempt carries them.
    fn apply(&self, mut rb: RequestBuilder) -> RequestBuilder {
        if !self.headers.is_empty() {
            rb = rb.headers(self.headers.clone());
        }
        if let Some(timeout) = self.timeout {
            rb = rb.timeout(timeout);
        }
        rb
    }
}

fn refuse_reserved_header(name: &HeaderName) -> Result<()> {
    if RESERVED_HEADERS.contains(name) || TRANSPORT_HEADERS.contains(name) {
        return Err(Error::ReservedHeader(name.as_str().to_owned()));
    }
    Ok(())
}

/// For a `Debug` that must not print values.
fn header_names(headers: &HeaderMap) -> Vec<&str> {
    headers.keys().map(HeaderName::as_str).collect()
}

/// Without extras this serialises to the same bytes as the [`Request`] alone
/// (pinned by a unit test), so a call without options is unchanged on the wire.
#[derive(Serialize)]
struct Body<'r, 'a, S: Serialize> {
    #[serde(flatten)]
    request: &'r Request<'a, S>,
    #[serde(flatten)]
    extra: &'r Map<String, Value>,
}

/// Builder for [`Client`].
#[derive(Clone)]
pub struct ClientBuilder {
    api_key: Option<String>,
    base_url: String,
    model: String,
    timeout: Duration,
    retry: RetryPolicy,
    observer: Option<Arc<dyn Observer>>,
    headers: HeaderMap,
}

impl fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("timeout", &self.timeout)
            .field("retry", &self.retry)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("observer", &self.observer.as_ref().map(|_| "set"))
            .field("default_headers", &header_names(&self.headers))
            .finish()
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: DEFAULT_BASE_URL.to_owned(),
            model: DEFAULT_MODEL.to_owned(),
            timeout: DEFAULT_TIMEOUT,
            retry: RetryPolicy::default(),
            observer: None,
            headers: HeaderMap::new(),
        }
    }
}

impl ClientBuilder {
    /// Set the API key explicitly (otherwise read from `TYPESAFE_API_KEY`).
    ///
    /// [`build`](Self::build) trims it and refuses a malformed or blank one
    /// rather than falling back to the environment: a caller that passed a
    /// key meant that one.
    #[must_use]
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Override the base URL (for tests or a proxy).
    #[must_use]
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Default model for [`Client::system_one`].
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Per-attempt timeout.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Retry policy.
    #[must_use]
    pub fn retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }

    /// Where this client reports token usage; without one it reports to
    /// [`crate::observer::global`]. Failed attempts always go to the global
    /// observer.
    #[must_use]
    pub fn observer(mut self, observer: Arc<dyn Observer>) -> Self {
        self.observer = Some(observer);
        self
    }

    /// A header sent on every request this client makes, the models list
    /// included (a gateway's routing or tenant header, say). A
    /// [`CallOptions::header`] of the same name replaces it for one call.
    ///
    /// A reserved header ([`CallOptions::header`]) is refused by
    /// [`build`](Self::build), so this method stays infallible and the
    /// refusal still comes before anything is sent. Mark a secret value with
    /// [`HeaderValue::set_sensitive`].
    #[must_use]
    pub fn default_header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.headers.insert(name, value);
        self
    }

    /// Build the client. Reads `TYPESAFE_API_KEY` if no key was set.
    ///
    /// The key is checked first, with the Python SDK's rules: trimmed of what
    /// Python's `str.isspace` accepts, [`Error::MissingApiKey`] when blank,
    /// [`Error::InvalidApiKey`] when it has inner whitespace, a control or a
    /// non-ASCII character, or is not valid UTF-8; no message quotes it. A
    /// base URL that does not parse is [`Error::Url`]; a reserved
    /// [`default_header`](Self::default_header) is [`Error::ReservedHeader`].
    pub fn build(self) -> Result<Client> {
        let api_key = resolve_api_key(self.api_key, || std::env::var(API_KEY_ENV))?;
        let base_url = Url::parse(&self.base_url).map_err(|e| Error::Url(e.to_string()))?;
        for name in self.headers.keys() {
            refuse_reserved_header(name)?;
        }
        // Unreachable after `resolve_api_key`; kept so a change to that check
        // cannot turn a bad key into a panic.
        let mut auth = HeaderValue::from_str(&format!("Bearer {api_key}")).map_err(|_| {
            Error::InvalidApiKey {
                reason: "the key cannot be sent in an HTTP header".into(),
            }
        })?;
        auth.set_sensitive(true);
        let mut headers = self.headers;
        headers.insert(AUTHORIZATION, auth);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(self.timeout)
            .user_agent(concat!("judgment/", env!("CARGO_PKG_VERSION")))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| Error::Transport {
                attempts: 0,
                source: e,
            })?;
        Ok(Client {
            http,
            base_url,
            model: self.model,
            retry: self.retry,
            observer: self.observer,
        })
    }
}

/// A configured TypeSafe API client. Cheap to clone.
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base_url: Url,
    model: String,
    retry: RetryPolicy,
    observer: Option<Arc<dyn Observer>>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.base_url.as_str())
            .field("model", &self.model)
            .field("retry", &self.retry)
            .field("api_key", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Start building a client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Production defaults with the API key from `TYPESAFE_API_KEY`.
    pub fn from_env() -> Result<Self> {
        ClientBuilder::default().build()
    }

    /// The default model, what [`Client::system_one`] sends.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// The client's retry policy.
    pub fn retry(&self) -> &RetryPolicy {
        &self.retry
    }

    /// Evaluate `state` against `questions` with the default model.
    pub async fn system_one<S: Serialize + Sync>(
        &self,
        state: &S,
        questions: &Questions,
    ) -> Result<Response> {
        self.evaluate(&Request {
            state,
            model: &self.model,
            questions,
        })
        .await
    }

    /// Evaluate a fully specified request with the client's own settings:
    /// [`Client::evaluate_with`] with no [`CallOptions`].
    pub async fn evaluate<S: Serialize + Sync>(
        &self,
        request: &Request<'_, S>,
    ) -> Result<Response> {
        self.evaluate_with(request, &CallOptions::default()).await
    }

    /// Evaluate a fully specified request with per-call options (module
    /// docs, `# Per-call options`). The `typesafe.evaluate` span is this
    /// method's, so there is one per call whichever of the two was called.
    /// The response is verified against `request.questions` before it is
    /// returned (module docs, `# Retries and errors`).
    #[tracing::instrument(
        name = "typesafe.evaluate",
        skip_all,
        fields(
            model = request.model,
            input_tokens = tracing::field::Empty,
            request_id = tracing::field::Empty
        )
    )]
    pub async fn evaluate_with<S: Serialize + Sync>(
        &self,
        request: &Request<'_, S>,
        options: &CallOptions,
    ) -> Result<Response> {
        let url = self.url("v1/systemone")?;
        let body = serde_json::to_vec(&Body {
            request,
            extra: &options.extra,
        })?;
        let policy = options.retry.as_ref().unwrap_or(&self.retry);
        let reply = self
            .send(policy, || {
                options.apply(self.http.post(url.clone()).body(body.clone()))
            })
            .await
            .inspect_err(|e| record_request_id(e.request_id()))?;
        record_request_id(reply.request_id.as_deref());
        let mut response: Response = reply.decode()?;
        // The field means the header: a body key of the same name is
        // overwritten, with `None` when the header was absent.
        response.request_id = reply.request_id;
        // Key and kind are the server's strings (`verify` walks only the asked
        // ids), hence escaped and cut.
        for (id, answer) in &response.answers {
            if matches!(answer, Answer::Unknown(_)) {
                tracing::warn!(
                    question = %sanitize_server_str(id),
                    kind = %sanitize_server_str(answer.kind()),
                    "answer of a kind this client does not know; kept as Answer::Unknown"
                );
            }
        }
        tracing::Span::current().record("input_tokens", response.usage.input_tokens);
        // Billed whether or not it fits, so reported before the check.
        self.observer().on_usage(&response.model, &response.usage);
        if let Err(e) = response.verify(request.questions) {
            crate::observer::global().on_failed_attempt("typesafe", "unfit");
            return Err(e);
        }
        Ok(response)
    }

    /// List the models and aliases this account may send (module docs,
    /// `` # `GET /v1/models` ``). A server that does not serve it answers
    /// [`Error::Http`] with status 404. The list carries no request id; the
    /// `typesafe.list_models` span and every error do.
    #[tracing::instrument(
        name = "typesafe.list_models",
        skip_all,
        fields(request_id = tracing::field::Empty)
    )]
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let url = self.url("v1/models")?;
        let reply = self
            .send(&self.retry, || self.http.get(url.clone()))
            .await
            .inspect_err(|e| record_request_id(e.request_id()))?;
        record_request_id(reply.request_id.as_deref());
        let parsed: ModelsResponse = reply.decode()?;
        Ok(parsed.models)
    }

    fn observer(&self) -> Arc<dyn Observer> {
        self.observer
            .clone()
            .unwrap_or_else(crate::observer::global)
    }

    fn url(&self, path: &str) -> Result<Url> {
        self.base_url
            .join(path)
            .map_err(|e| Error::Url(e.to_string()))
    }

    /// The shared retry loop, with the last response's request id read here.
    /// It does not touch the span: the instrumented caller records the id,
    /// so the write cannot land on some other span a future caller has open.
    async fn send(
        &self,
        policy: &RetryPolicy,
        make: impl Fn() -> reqwest::RequestBuilder,
    ) -> Result<Reply> {
        match http::send_with_retries(policy, "typesafe", make).await {
            Ok(Completed {
                status,
                body,
                attempts,
                retry_after,
                headers,
                ..
            }) => {
                let request_id = read_request_id(&headers);
                if status.is_success() {
                    Ok(Reply { body, request_id })
                } else {
                    Err(classify(status, body, attempts, retry_after, request_id))
                }
            }
            Err(Exhausted::Transport { attempts, source }) => {
                Err(Error::Transport { attempts, source })
            }
            Err(Exhausted::TooLarge { limit, .. }) => Err(Error::ResponseTooLarge { limit }),
        }
    }
}

/// A 2xx body and the request id its response carried, not yet decoded.
struct Reply {
    body: String,
    request_id: Option<String>,
}

impl Reply {
    /// A failure keeps the request id, since a 2xx that does not decode is
    /// still a call TypeSafe can look up, and counts as a failed attempt
    /// because the retry loop counted the 2xx as a success.
    fn decode<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_str(&self.body).map_err(|source| {
            crate::observer::global().on_failed_attempt("typesafe", "decode");
            Error::Decode {
                source,
                request_id: self.request_id.clone(),
            }
        })
    }
}

/// Module docs, `# Retries and errors`.
fn classify(
    status: StatusCode,
    body: String,
    attempts: u32,
    retry_after: Option<Duration>,
    request_id: Option<String>,
) -> Error {
    match status.as_u16() {
        code @ (400 | 413 | 422) => {
            let body = error_detail(&body);
            Error::InvalidRequest {
                status: code,
                detail: body.detail,
                issues: body.issues,
                kind: body.kind,
                request_id,
            }
        }
        401 => Error::Unauthorized { request_id },
        403 => Error::PermissionDenied {
            detail: error_detail(&body).detail,
            request_id,
        },
        429 => Error::RateLimited {
            attempts,
            retry_after,
            request_id,
        },
        529 => Error::Overloaded {
            attempts,
            request_id,
        },
        code => Error::Http {
            status: code,
            attempts,
            body: http::truncate(body),
            request_id,
        },
    }
}

struct ErrorBody {
    detail: String,
    issues: Vec<ValidationIssue>,
    kind: Option<String>,
}

impl ErrorBody {
    fn plain(detail: String) -> Self {
        Self {
            detail,
            issues: Vec::new(),
            kind: None,
        }
    }
}

/// The message, the validation issues and the code of a 400, 403 or 422
/// body, for [`Error::InvalidRequest`] and [`Error::PermissionDenied`].
///
/// The message's field order is the Python SDK's (`extract_message` in its
/// `errors.py`); the issues are parsed whatever supplies the message, so
/// code gets them even when the server also sent prose. The code comes
/// first from `detail.error_type`, the hosted API's 400 shape, which may
/// come with no message at all. Deliberate differences from the SDK: an
/// empty string does not win over the next field; only a leading `body`
/// location segment is dropped ([`ValidationIssue::path`]), since dropping
/// every one hides a question whose id is `body`; a raw body is truncated
/// like every body this crate quotes. A validation entry's `input` and
/// `ctx` are dropped, since `input` can be a piece of the state, but only
/// best-effort: a body this function does not recognise is quoted as it
/// came.
fn error_detail(body: &str) -> ErrorBody {
    if body.trim().is_empty() {
        return ErrorBody::plain("no body".to_owned());
    }
    let object = match serde_json::from_str::<Value>(body) {
        Ok(Value::Object(object)) => object,
        Ok(Value::String(text)) if !text.is_empty() => {
            return ErrorBody::plain(http::truncate(text));
        }
        _ => return ErrorBody::plain(http::truncate(body.to_owned())),
    };
    let issues: Vec<ValidationIssue> = object
        .get("detail")
        .and_then(Value::as_array)
        .map(|entries| entries.iter().filter_map(validation_issue).collect())
        .unwrap_or_default();
    let nested = |key: &str, field: &str| object.get(key).and_then(|v| v.get(field));
    let kind = non_empty_str(nested("detail", "error_type"))
        .or_else(|| non_empty_str(nested("error", "type")))
        .or_else(|| non_empty_str(object.get("error_type")))
        .or_else(|| non_empty_str(object.get("type")))
        .map(str::to_owned);
    let message = non_empty_str(object.get("error"))
        .or_else(|| non_empty_str(nested("error", "message")))
        .or_else(|| non_empty_str(object.get("message")))
        .or_else(|| non_empty_str(object.get("detail")))
        .or_else(|| non_empty_str(nested("detail", "message")))
        .map(str::to_owned)
        .or_else(|| {
            (!issues.is_empty()).then(|| {
                issues
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            })
        })
        // As in the SDK (`"; ".join(parts) or None`).
        .filter(|message| !message.is_empty())
        // A code with no message says more than the raw body does.
        .or_else(|| kind.clone())
        .unwrap_or_else(|| body.to_owned());
    ErrorBody {
        detail: http::truncate(message),
        issues,
        kind,
    }
}

/// An empty message does not win over the next field.
fn non_empty_str(value: Option<&Value>) -> Option<&str> {
    value.and_then(Value::as_str).filter(|s| !s.is_empty())
}

/// `None` without a string `msg`; `input` and `ctx` are not read.
fn validation_issue(entry: &Value) -> Option<ValidationIssue> {
    let msg = entry.get("msg")?.as_str()?.to_owned();
    let loc = entry
        .get("loc")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    item.as_str()
                        .map_or_else(|| item.to_string(), str::to_owned)
                })
                .collect()
        })
        .unwrap_or_default();
    let kind = entry
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    Some(ValidationIssue { loc, msg, kind })
}

/// Python's `str.isspace`, which the SDK trims from a key: Rust's
/// `char::is_whitespace` plus the information separators U+001C to U+001F.
fn is_sdk_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// The environment is a parameter so tests can supply one: setting a process
/// variable is `unsafe` in edition 2024, and this workspace forbids `unsafe`.
/// It is not called when a key was passed explicitly, blank or not.
fn resolve_api_key(
    explicit: Option<String>,
    env: impl FnOnce() -> Result<String, VarError>,
) -> Result<String> {
    let (raw, origin) = match explicit {
        Some(key) => (key, "the key passed to the client builder"),
        None => match env() {
            Ok(key) => (key, API_KEY_ENV),
            Err(VarError::NotPresent) => return Err(Error::MissingApiKey),
            Err(VarError::NotUnicode(_)) => {
                return Err(Error::InvalidApiKey {
                    reason: format!("{API_KEY_ENV} is not valid UTF-8"),
                });
            }
        },
    };
    let key = raw.trim_matches(is_sdk_space);
    if key.is_empty() {
        return Err(Error::MissingApiKey);
    }
    if let Some(c) = key.chars().find(|c| !c.is_ascii_graphic()) {
        let what = if is_sdk_space(c) {
            "has whitespace inside it"
        } else if c.is_control() {
            "contains a control character"
        } else {
            // A byte-order mark lands here too: not whitespace to Python either.
            "contains a non-ASCII character"
        };
        return Err(Error::InvalidApiKey {
            reason: format!("{origin} {what}"),
        });
    }
    Ok(key.to_owned())
}

/// `to_str` refuses non-ASCII, CR, LF and DEL but lets a tab through, so one
/// left inside the value after trimming is refused here.
fn read_request_id(headers: &HeaderMap) -> Option<String> {
    let v = headers.get(REQUEST_ID_HEADER)?.to_str().ok()?.trim();
    if v.is_empty() || v.bytes().any(|b| b.is_ascii_control()) {
        return None;
    }
    if v.len() > REQUEST_ID_MAX_LEN {
        tracing::debug!(len = v.len(), "x-typesafe-request-id ignored: too long");
        return None;
    }
    Some(v.to_owned())
}

/// Called only from a method whose `#[tracing::instrument]` declares
/// `request_id`; an absent field is how a trace says no id was sent.
fn record_request_id(id: Option<&str>) {
    if let Some(id) = id {
        tracing::Span::current().record("request_id", id);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn request_id_is_read_only_when_usable() {
        let read = |value: HeaderValue| {
            let mut h = HeaderMap::new();
            h.insert(REQUEST_ID_HEADER, value);
            read_request_id(&h)
        };
        assert_eq!(
            read(HeaderValue::from_static("req_1")).as_deref(),
            Some("req_1")
        );
        assert_eq!(read_request_id(&HeaderMap::new()), None, "absent");
        assert_eq!(read(HeaderValue::from_static("")), None, "empty");
        assert_eq!(read(HeaderValue::from_static("   ")), None, "whitespace");
        assert_eq!(
            read(HeaderValue::from_static(" req_2 ")).as_deref(),
            Some("req_2"),
            "trimmed"
        );
        let longest = "r".repeat(REQUEST_ID_MAX_LEN);
        assert_eq!(
            read(HeaderValue::from_str(&longest).unwrap_or_else(|e| panic!("{e}"))),
            Some(longest.clone()),
            "256 bytes is kept"
        );
        let too_long = "r".repeat(REQUEST_ID_MAX_LEN + 1);
        assert_eq!(
            read(HeaderValue::from_str(&too_long).unwrap_or_else(|e| panic!("{e}"))),
            None,
            "257 bytes is dropped"
        );
        let opaque = HeaderValue::from_bytes(b"req\x80").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(read(opaque), None, "not printable ASCII");
        assert_eq!(read(HeaderValue::from_static("req\t1")), None, "inner tab");
        assert_eq!(
            read(HeaderValue::from_static("\treq_3\t")).as_deref(),
            Some("req_3"),
            "outer tabs are trimmed"
        );
        assert_eq!(
            read(HeaderValue::from_static("req 1")).as_deref(),
            Some("req 1"),
            "inner space kept"
        );
    }

    /// An environment that must not be read.
    fn no_env() -> std::result::Result<String, VarError> {
        panic!("the environment was read although a key was passed")
    }

    fn resolved(explicit: &str) -> Result<String> {
        resolve_api_key(Some(explicit.to_owned()), no_env)
    }

    #[test]
    fn a_key_is_trimmed_like_the_sdk() {
        assert_eq!(resolved("  sk-abc\r\n").ok().as_deref(), Some("sk-abc"));
        // The information separators are whitespace to Python, not to Rust.
        assert_eq!(
            resolved("\u{1f}sk-abc\u{1c}").ok().as_deref(),
            Some("sk-abc")
        );
        assert_eq!(
            resolved("\u{a0}sk-abc\u{3000}").ok().as_deref(),
            Some("sk-abc")
        );
    }

    #[test]
    fn a_blank_explicit_key_is_missing_and_never_reads_the_environment() {
        for blank in ["", "   ", "\n", "\t\u{1d}\u{a0}"] {
            let err = resolved(blank).unwrap_err();
            assert!(matches!(err, Error::MissingApiKey), "{blank:?}: {err:?}");
        }
    }

    #[test]
    fn an_unset_or_blank_environment_is_missing() {
        let from_env = |value: std::result::Result<&str, VarError>| {
            resolve_api_key(None, || value.map(str::to_owned))
        };
        assert!(matches!(
            from_env(Err(VarError::NotPresent)),
            Err(Error::MissingApiKey)
        ));
        assert!(matches!(from_env(Ok("  ")), Err(Error::MissingApiKey)));
        assert_eq!(from_env(Ok("sk-env\n")).ok().as_deref(), Some("sk-env"));
    }

    #[test]
    fn a_malformed_key_is_invalid_and_never_quoted() {
        let cases = [
            ("SECRET XYZ", "has whitespace inside it"),
            ("SECRET\tXYZ", "has whitespace inside it"),
            ("SECRET\u{a0}XYZ", "has whitespace inside it"),
            ("SECRET\u{7f}XYZ", "contains a control character"),
            ("SECRETXYZ\u{0}", "contains a control character"),
            ("SECRETXYZé", "contains a non-ASCII character"),
            ("SECRET\u{200b}XYZ", "contains a non-ASCII character"),
            ("\u{feff}SECRETXYZ", "contains a non-ASCII character"),
        ];
        for (key, what) in cases {
            let err = resolved(key).unwrap_err();
            let Error::InvalidApiKey { reason } = &err else {
                panic!("{key:?}: {err:?}");
            };
            assert_eq!(
                reason,
                &format!("the key passed to the client builder {what}"),
                "{key:?}"
            );
            for shown in [err.to_string(), format!("{err:?}")] {
                assert!(!shown.contains("SECRET"), "{key:?}: {shown}");
                assert!(!shown.contains("XYZ"), "{key:?}: {shown}");
            }
        }
        let err = resolve_api_key(None, || Ok("SECRET XYZ".to_owned())).unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid API key: TYPESAFE_API_KEY has whitespace inside it"
        );
    }

    #[test]
    fn a_non_utf8_environment_key_is_invalid() {
        let err = resolve_api_key(None, || {
            Err(VarError::NotUnicode(std::ffi::OsString::from("sk")))
        })
        .unwrap_err();
        assert!(
            matches!(&err, Error::InvalidApiKey { reason } if reason == "TYPESAFE_API_KEY is not valid UTF-8"),
            "{err:?}"
        );
    }

    #[test]
    fn build_refuses_an_invalid_key_before_any_request() {
        // Nothing listens on the base URL: a request would be a transport
        // error, not a key error.
        for key in ["sk\rabc", "sk abc"] {
            let err = Client::builder()
                .api_key(key)
                .base_url("http://127.0.0.1:1")
                .build()
                .unwrap_err();
            assert!(
                matches!(err, Error::InvalidApiKey { .. }),
                "{key:?}: {err:?}"
            );
        }
        let err = Client::builder()
            .api_key("sk\u{e9}")
            .base_url("not a url")
            .build()
            .unwrap_err();
        assert!(matches!(err, Error::InvalidApiKey { .. }), "{err:?}");
        let err = Client::builder().api_key(" \n").build().unwrap_err();
        assert!(matches!(err, Error::MissingApiKey), "{err:?}");
    }

    fn detail(body: &str) -> (String, Vec<ValidationIssue>) {
        let read = error_detail(body);
        (read.detail, read.issues)
    }

    fn only_detail(body: &str) -> String {
        let (message, issues) = detail(body);
        assert!(issues.is_empty(), "{body}: {issues:?}");
        message
    }

    fn kind_of(body: &str) -> Option<String> {
        error_detail(body).kind
    }

    #[test]
    fn error_detail_reads_the_hosted_api_s_three_400_shapes() {
        // Bodies as the hosted API sent them (docs/verification/hosted-typesafe.md),
        // none in the OpenAPI document, which describes the 422 list only.

        let body = r#"{"detail":"Too many choices. Must have at most 255 choices."}"#;
        assert_eq!(
            only_detail(body),
            "Too many choices. Must have at most 255 choices."
        );
        assert_eq!(kind_of(body), None);

        let body =
            r#"{"detail":{"error_type":"api_usage_error","message":"Unknown model: jev-0.0.0"}}"#;
        assert_eq!(only_detail(body), "Unknown model: jev-0.0.0");
        assert_eq!(kind_of(body).as_deref(), Some("api_usage_error"));

        let body = r#"{"detail":{"error_type":"max_tokens_exceeded"}}"#;
        assert_eq!(only_detail(body), "max_tokens_exceeded");
        assert_eq!(kind_of(body).as_deref(), Some("max_tokens_exceeded"));

        assert_eq!(
            kind_of(r#"{"error":{"message":"bad question","type":"invalid_request"}}"#).as_deref(),
            Some("invalid_request")
        );
        let body = r#"{"detail":{"error_type":"authentication_error","message":"Must supply an API key! Check your request and try again."}}"#;
        assert_eq!(
            only_detail(body),
            "Must supply an API key! Check your request and try again."
        );
        assert_eq!(kind_of(body).as_deref(), Some("authentication_error"));

        // A 422 for one value of a union-typed field: one issue per
        // alternative, each with a trailing type segment, and no code.
        let body = r#"{"detail":[{"type":"string_type","loc":["body","questions","s","score","criteria",1,"str"],"msg":"Input should be a valid string","input":null},{"type":"dict_type","loc":["body","questions","s","score","criteria",1,"dict[any,any]"],"msg":"Input should be a valid dictionary","input":null},{"type":"list_type","loc":["body","questions","s","score","criteria",1,"list[any]"],"msg":"Input should be a valid list","input":null}]}"#;
        let (message, issues) = detail(body);
        assert_eq!(issues.len(), 3);
        assert_eq!(issues[0].path(), "questions.s.score.criteria.1.str");
        assert_eq!(issues[1].kind, "dict_type");
        assert!(
            message
                .starts_with("questions.s.score.criteria.1.str: Input should be a valid string; ")
        );
        assert_eq!(kind_of(body), None);
    }

    #[test]
    fn error_detail_reads_every_body_shape() {
        // The OpenAPI document's example, at
        // /components/schemas/HTTPValidationError/properties/detail/examples/0.
        let (message, issues) = detail(
            r#"{"detail":[{"loc":["body","state"],"msg":"Field required","type":"missing"}]}"#,
        );
        assert_eq!(message, "state: Field required");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].path(), "state");
        assert_eq!(issues[0].kind, "missing");
        assert_eq!(issues[0].loc, ["body", "state"]);

        // The ValidationError.loc example: a question error names the id
        // second and the question type third.
        let (message, issues) = detail(
            r#"{"detail":[{"loc":["body","questions","urgency","score","criteria"],"msg":"List should have at least 2 items","type":"too_short"}]}"#,
        );
        assert_eq!(issues[0].path(), "questions.urgency.score.criteria");
        assert_eq!(
            message,
            "questions.urgency.score.criteria: List should have at least 2 items"
        );

        let (message, issues) = detail(
            r#"{"detail":[{"loc":["body","state"],"msg":"Field required","type":"missing"},{"loc":["body","model"],"msg":"Input should be a valid string","type":"string_type"}]}"#,
        );
        assert_eq!(issues.len(), 2);
        assert_eq!(
            message,
            "state: Field required; model: Input should be a valid string"
        );

        assert_eq!(
            only_detail(r#"{"detail":"questions.x.criteria: invalid"}"#),
            "questions.x.criteria: invalid"
        );
        assert_eq!(
            only_detail(r#"{"error":"model mismatch"}"#),
            "model mismatch"
        );
        assert_eq!(
            only_detail(r#"{"error":{"message":"bad question","type":"invalid_request"}}"#),
            "bad question"
        );
        assert_eq!(only_detail(r#"{"message":"slow down"}"#), "slow down");
        assert_eq!(only_detail(r#"{"detail":{"message":"nested"}}"#), "nested");
        assert_eq!(
            only_detail(r#"{"error":"first","message":"second"}"#),
            "first"
        );
        let (message, issues) = detail(
            r#"{"message":"invalid body","detail":[{"loc":["body","state"],"msg":"Field required","type":"missing"}]}"#,
        );
        assert_eq!(message, "invalid body");
        assert_eq!(issues.len(), 1);
        // Pinned deviation from the SDK: an empty string does not win.
        assert_eq!(only_detail(r#"{"error":"","message":"x"}"#), "x");

        let html = "<html><body>Bad Gateway</body></html>";
        assert_eq!(only_detail(html), html);
        let long = format!("<html>{}</html>", "x".repeat(5_000));
        let quoted = only_detail(&long);
        assert!(quoted.len() <= 2_000 + '…'.len_utf8(), "{}", quoted.len());
        assert!(quoted.ends_with('…'));
        assert_eq!(only_detail(""), "no body");
        assert_eq!(only_detail(" \n"), "no body");
        assert_eq!(only_detail(r#"{"detail":[]}"#), r#"{"detail":[]}"#);
        assert_eq!(only_detail(r#"{"other":1}"#), r#"{"other":1}"#);
        assert_eq!(only_detail("[1,2]"), "[1,2]");
        assert_eq!(only_detail("42"), "42");
        // A JSON string body is its own message, blank included, as in the
        // SDK (`body or None`); only an empty one is quoted.
        assert_eq!(only_detail(r#""model not found""#), "model not found");
        assert_eq!(only_detail(r#"" ""#), " ");
        assert_eq!(only_detail(r#""""#), r#""""#);

        let (message, issues) = detail(
            r#"{"detail":[{"loc":["body","a"]},{"loc":["body","b"],"msg":7},"text",{"loc":["body","c"],"msg":"kept","type":"x"}]}"#,
        );
        assert_eq!(issues.len(), 1);
        assert_eq!(message, "c: kept");
        let body = r#"{"detail":[{"loc":["body","a"]}]}"#;
        assert_eq!(only_detail(body), body);
        // Issues that join to nothing are no message, as in the SDK.
        let body = r#"{"detail":[{"loc":["body"],"msg":"","type":"x"}]}"#;
        let (message, issues) = detail(body);
        assert_eq!(message, body);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].msg, "");
        // Two of them join to "; ", which is kept, as the SDK keeps it.
        let (message, _) =
            detail(r#"{"detail":[{"loc":["body"],"msg":""},{"loc":["body"],"msg":""}]}"#);
        assert_eq!(message, "; ");

        let (message, issues) = detail(r#"{"detail":[{"msg":"Field required","type":"missing"}]}"#);
        assert!(issues[0].loc.is_empty());
        assert_eq!(message, "Field required");
        let (message, issues) =
            detail(r#"{"detail":[{"loc":"body.state","msg":"Field required"}]}"#);
        assert!(issues[0].loc.is_empty());
        assert_eq!(issues[0].kind, "", "a missing type is an empty kind");
        assert_eq!(message, "Field required");

        let (message, issues) = detail(
            r#"{"detail":[{"loc":["body","questions","risk","score","criteria",0],"msg":"Input should be a valid string","type":"string_type"}]}"#,
        );
        assert_eq!(issues[0].loc.last().map(String::as_str), Some("0"));
        assert_eq!(
            message,
            "questions.risk.score.criteria.0: Input should be a valid string"
        );
    }

    #[test]
    fn validation_input_never_reaches_the_message() {
        let (message, issues) = detail(
            r#"{"detail":[{"loc":["body","state"],"msg":"Input should be a valid dictionary","type":"dict_type","input":"SECRET-STATE","ctx":{"hint":"SECRET-CTX"}}]}"#,
        );
        assert_eq!(message, "state: Input should be a valid dictionary");
        let shown = format!("{message} {issues:?}");
        assert!(!shown.contains("SECRET"), "{shown}");

        // Best-effort only: a body with no usable entry is still quoted.
        let body = r#"{"detail":[{"loc":["body","state"],"input":"SECRET-STATE"}]}"#;
        assert!(only_detail(body).contains("SECRET-STATE"));
    }

    fn questions() -> Questions {
        let mut q = Questions::new();
        q.noul("urgent", "Does `message` convey urgency?", None)
            .unwrap();
        q.score("severity", "How bad is `message`?", ["minor", "major"])
            .unwrap();
        q
    }

    fn body<S: Serialize>(request: &Request<'_, S>, options: &CallOptions) -> Vec<u8> {
        serde_json::to_vec(&Body {
            request,
            extra: &options.extra,
        })
        .unwrap()
    }

    #[test]
    fn a_call_without_extras_is_the_request_byte_for_byte() {
        let q = questions();
        let object = serde_json::json!({ "message": "help", "nested": { "b": 1, "a": [2, 3] } });
        let text = "Help! My payouts have been failing.";
        let none = CallOptions::new();
        let no_extras = CallOptions::new()
            .timeout(Duration::from_secs(1))
            .retry(RetryPolicy::none())
            .header(
                HeaderName::from_static("x-team"),
                HeaderValue::from_static("a"),
            )
            .unwrap();
        let object_request = Request {
            state: &object,
            model: "jev-latest",
            questions: &q,
        };
        let text_request = Request {
            state: &text,
            model: "jev-latest",
            questions: &q,
        };
        for options in [&none, &no_extras] {
            assert_eq!(
                body(&object_request, options),
                serde_json::to_vec(&object_request).unwrap()
            );
            assert_eq!(
                body(&text_request, options),
                serde_json::to_vec(&text_request).unwrap()
            );
        }
        let with = CallOptions::new()
            .extra("beam_width", 4)
            .unwrap()
            .extra("tag", Value::Null)
            .unwrap();
        let sent = String::from_utf8(body(&text_request, &with)).unwrap();
        let bare = String::from_utf8(serde_json::to_vec(&text_request).unwrap()).unwrap();
        assert_eq!(
            sent,
            format!(
                "{},\"beam_width\":4,\"tag\":null}}",
                bare.strip_suffix('}').unwrap()
            )
        );
    }

    #[test]
    fn an_extra_field_may_not_replace_state_model_or_questions() {
        // The names themselves, not the constant: a name dropped from it
        // must fail here.
        assert_eq!(RESERVED_FIELDS.len(), 3);
        for name in ["state", "model", "questions"] {
            let err = CallOptions::new().extra(name, "x").unwrap_err();
            assert!(
                matches!(&err, Error::ReservedField(n) if n == name),
                "{name}: {err:?}"
            );
            assert_eq!(
                err.to_string(),
                format!("body field {name:?} is set by the client and cannot be an extra field")
            );
            assert_eq!(err.request_id(), None);
        }
        let options = CallOptions::new()
            .extra("Model", "x")
            .unwrap()
            .extra("STATE", 1)
            .unwrap();
        assert_eq!(options.extra.len(), 2);
        let options = CallOptions::new()
            .extra("tag", 1)
            .unwrap()
            .extra("tag", 2)
            .unwrap();
        assert_eq!(options.extra.get("tag"), Some(&Value::from(2)));
    }

    #[test]
    fn the_client_owned_headers_are_refused_whatever_their_case() {
        let value = || HeaderValue::from_static("x");
        for spelled in [
            "Authorization",
            "AUTHORIZATION",
            "Content-Type",
            "content-type",
            "User-Agent",
            "X-TypeSafe-Retry-Count",
            "x-typesafe-retry-count",
            "Content-Length",
            "content-length",
            "Transfer-Encoding",
            "Host",
            "HOST",
            "Connection",
            "TE",
            "Upgrade",
        ] {
            let name = HeaderName::from_bytes(spelled.as_bytes()).unwrap();
            let lower = spelled.to_ascii_lowercase();
            let expected =
                format!("header {lower:?} is set by the client and cannot be overridden");

            let err = CallOptions::new()
                .header(name.clone(), value())
                .unwrap_err();
            assert!(
                matches!(&err, Error::ReservedHeader(n) if *n == lower),
                "{spelled}: {err:?}"
            );
            assert_eq!(err.to_string(), expected);
            assert_eq!(err.request_id(), None);

            // As a default header, refused at build(), before any request:
            // nothing listens on the base URL.
            let err = Client::builder()
                .api_key("sk-test")
                .base_url("http://127.0.0.1:1")
                .default_header(HeaderName::from_static("x-team"), value())
                .default_header(name, value())
                .build()
                .unwrap_err();
            assert!(
                matches!(&err, Error::ReservedHeader(n) if *n == lower),
                "{spelled}: {err:?}"
            );
        }
        let err = Client::builder()
            .api_key(" ")
            .default_header(AUTHORIZATION, value())
            .build()
            .unwrap_err();
        assert!(matches!(err, Error::MissingApiKey), "{err:?}");
        CallOptions::new()
            .header(HeaderName::from_static("x-team"), value())
            .unwrap();
        Client::builder()
            .api_key("sk-test")
            .default_header(HeaderName::from_static("x-team"), value())
            .build()
            .unwrap();
    }

    #[test]
    fn debug_output_names_headers_and_extras_but_never_their_values() {
        let mut secret = HeaderValue::from_static("SECRET-HEADER");
        secret.set_sensitive(true);
        let options = CallOptions::new()
            .timeout(Duration::from_millis(250))
            .header(HeaderName::from_static("x-gateway-token"), secret.clone())
            .unwrap()
            .header(
                HeaderName::from_static("x-team"),
                HeaderValue::from_static("PLAIN-HEADER"),
            )
            .unwrap()
            .extra("beam_width", "SECRET-EXTRA")
            .unwrap();
        let dbg = format!("{options:?}");
        for name in ["x-gateway-token", "x-team", "beam_width", "250ms"] {
            assert!(dbg.contains(name), "{name}: {dbg}");
        }
        for value in ["SECRET-HEADER", "PLAIN-HEADER", "SECRET-EXTRA"] {
            assert!(!dbg.contains(value), "{value}: {dbg}");
        }

        let builder = Client::builder()
            .api_key("sk-secret")
            .default_header(HeaderName::from_static("x-gateway-token"), secret)
            .default_header(
                HeaderName::from_static("x-team"),
                HeaderValue::from_static("PLAIN-HEADER"),
            );
        let dbg = format!("{builder:?}");
        assert!(dbg.contains("x-gateway-token"), "{dbg}");
        assert!(dbg.contains("x-team"), "{dbg}");
        for value in ["SECRET-HEADER", "PLAIN-HEADER", "sk-secret"] {
            assert!(!dbg.contains(value), "{value}: {dbg}");
        }
        let dbg = format!("{:?}", builder.build().unwrap());
        for value in ["SECRET-HEADER", "PLAIN-HEADER", "sk-secret"] {
            assert!(!dbg.contains(value), "{value}: {dbg}");
        }
    }

    #[test]
    fn debug_output_redacts_the_key() {
        let c = Client::builder()
            .api_key("sk-secret")
            .build()
            .unwrap_or_else(|e| panic!("{e}"));
        let dbg = format!("{c:?}");
        assert!(!dbg.contains("sk-secret"));
        assert!(dbg.contains("<redacted>"));
    }
}
