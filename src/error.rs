//! One error type for the client and the typed answer layer, grouped by what
//! fixes each variant rather than by HTTP status, because a caller picks a
//! remedy from the error alone: configuration ([`Error::MissingApiKey`],
//! [`Error::InvalidApiKey`], [`Error::Unauthorized`],
//! [`Error::PermissionDenied`]), request ([`Error::InvalidRequest`],
//! [`Error::InvalidQuestion`], [`Error::DuplicateQuestionId`],
//! [`Error::ReservedHeader`], [`Error::ReservedField`], [`Error::Url`]),
//! transient ([`Error::RateLimited`], [`Error::Overloaded`]), transport and
//! decode ([`Error::Transport`], [`Error::ResponseTooLarge`], [`Error::Http`],
//! [`Error::Decode`]), an answer that does not fit its question
//! ([`Error::is_unfit`]), the caller's own value ([`Error::NotAProbability`])
//! and recordings ([`Error::Io`], [`Error::NoRecording`],
//! [`Error::InvalidRecording`]). Each variant says what fixes it.
//!
//! [`crate::eval`] keeps its own smaller [`crate::eval::Error`] for recording
//! files by case id, because a harness handles a missing file differently
//! from a missing answer; the backends map it into this type.
//!
//! # Request id
//!
//! Every variant built from an HTTP response, and the four answer-fit
//! errors, carry TypeSafe's `x-typesafe-request-id` when the response had
//! one ([`Error::request_id`]). It is the one link from a failure to
//! TypeSafe's own logs, so it also ends the message (` [request_id …]`),
//! where a log line that keeps only the message still has it. The API does
//! not promise the header, so it is optional.

use std::time::Duration;

/// Everything that can go wrong talking to TypeSafe or reading its answers.
///
/// Non-exhaustive, so a `match` outside this crate needs a wildcard arm; the
/// variants themselves are not, so their fields can be matched and built.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// No usable key: none passed to the builder and `TYPESAFE_API_KEY`
    /// unset or blank, or a blank key passed to the builder. A blank
    /// explicit key never falls back to the environment, as in the Python
    /// SDK: another key would authenticate as someone else.
    #[error("no API key: pass a non-blank key to the client builder, or set TYPESAFE_API_KEY")]
    MissingApiKey,
    /// The key cannot be a TypeSafe key (whitespace inside it, a control or
    /// non-ASCII character, or `TYPESAFE_API_KEY` not valid UTF-8); refused
    /// when the client is built, with the Python SDK's rule, so a pasted
    /// non-breaking space fails at start-up with a named cause instead of a
    /// 401 on the first call. `reason` never contains any part of the key.
    #[error("invalid API key: {reason}")]
    InvalidApiKey {
        /// Where the key came from and what is wrong, without the key.
        reason: String,
    },
    /// The API rejected the key (HTTP 401). Not retried; check that the key
    /// is the right one and still valid.
    #[error("authentication failed (401): check the API key{}", request_id_suffix(.request_id.as_deref()))]
    Unauthorized {
        /// TypeSafe's `x-typesafe-request-id`, when the response had one.
        request_id: Option<String>,
    },
    /// The API accepted the key but refused the call (HTTP 403). Not
    /// retried, and kept apart from [`Error::Unauthorized`] because a new
    /// key does not help, the account's access does. The hosted API answers
    /// 403 to a request without a `Bearer` key too; this client always
    /// sends one, so that means something in between stripped the header.
    #[error("permission denied (403): {detail}{}", request_id_suffix(.request_id.as_deref()))]
    PermissionDenied {
        /// The server's message, or the body truncated when it has none.
        detail: String,
        /// TypeSafe's `x-typesafe-request-id`, when the response had one.
        request_id: Option<String>,
    },
    /// The API refused the request body (HTTP 400, 413 or 422). Not
    /// retried; fix the question or the state the issues point at, or send
    /// less of it ([`Error::is_request_too_large`]). The statuses share a
    /// variant because they mean the same to the caller: the API reference
    /// documents 422 for a body that fails validation, the official SDKs
    /// have a bad-request error for 400, and a compatible server may answer
    /// 413 for a body past its own limits. Each issue's echoed `input` is
    /// dropped because it can be a piece of the state.
    #[error("request rejected by the API ({status}): {detail}{}", request_id_suffix(.request_id.as_deref()))]
    InvalidRequest {
        /// HTTP status: 400, 413 or 422.
        status: u16,
        /// The server's message, the issues joined, the code, or the body
        /// truncated, in that order of preference.
        detail: String,
        /// The validation issues the body listed; empty when it listed none.
        issues: Vec<ValidationIssue>,
        /// The server's machine-readable code, when the body carried one;
        /// `None` for a 422, whose issues carry their own `kind` each.
        kind: Option<String>,
        /// TypeSafe's `x-typesafe-request-id`, when the response had one.
        request_id: Option<String>,
    },
    /// Rate limited (HTTP 429), returned once the retry policy stopped. Slow
    /// down: wait `retry_after` when the server gave one, and lower the
    /// request rate or raise the account's limit if it keeps happening.
    #[error(
        "rate limited (429) after {attempts} attempts{}{}",
        crate::error::retry_after_suffix(*.retry_after),
        request_id_suffix(.request_id.as_deref())
    )]
    RateLimited {
        /// Total attempts made, including the first.
        attempts: u32,
        /// The last response's wait (`retry-after-ms` or `Retry-After`),
        /// when it named one. It can be longer than any wait the policy
        /// took, when the policy's cap or budget refused it.
        retry_after: Option<Duration>,
        /// The last response's `x-typesafe-request-id`, when it had one.
        request_id: Option<String>,
    },
    /// TypeSafe overloaded (HTTP 529), returned once the retry policy
    /// stopped. Nothing on the caller's side is wrong; wait and try again.
    #[error("service overloaded (529) after {attempts} attempts{}", request_id_suffix(.request_id.as_deref()))]
    Overloaded {
        /// Total attempts made, including the first.
        attempts: u32,
        /// The last response's `x-typesafe-request-id`, when it had one.
        request_id: Option<String>,
    },
    /// Any other non-success status. Most often the API's own 408 or 5xx
    /// once the retry policy stopped (wait and try again); otherwise a
    /// proxy, a base URL that is not the API (a 3xx among them, since the
    /// client follows no redirect), or a status newer than this crate.
    #[error(
        "unexpected HTTP status {status} after {attempts} attempts: {}{}",
        body_or_none(.body),
        request_id_suffix(.request_id.as_deref())
    )]
    Http {
        /// Status code.
        status: u16,
        /// Total attempts made, including the first.
        attempts: u32,
        /// Response body, truncated, as it came (empty when the response
        /// had none).
        body: String,
        /// The last response's `x-typesafe-request-id`, when it had one.
        request_id: Option<String>,
    },
    /// Network failure, TLS failure, timeout or a body that could not be
    /// read, once the retry policy stopped. Check connectivity, the base URL
    /// and the per-attempt timeout. No request id: no response came back,
    /// or its headers were dropped with its unreadable body, as the SDKs do.
    #[cfg(feature = "http")]
    #[error("transport error after {attempts} attempts: {source}")]
    Transport {
        /// Total attempts made, including the first.
        attempts: u32,
        /// Underlying reqwest error.
        #[source]
        source: reqwest::Error,
    },
    /// The body was over the policy's
    /// [`max_body_bytes`](crate::RetryPolicy::max_body_bytes) and was not
    /// read: the API answered with far more than it documents, or something
    /// else answered in its place. Check the base URL before raising the
    /// cap. Never retried; no request id, the response was dropped unread.
    #[error("response body over {limit} bytes; not read")]
    ResponseTooLarge {
        /// The cap that was passed, in bytes.
        limit: usize,
    },
    /// The body (or a recording) was not the JSON shape the API documents:
    /// the API changed, the base URL points at something else, or the file
    /// is corrupt. Decoding tolerates what the API may add
    /// ([`crate::answer`]), so only a known shape broken lands here. A 2xx
    /// that does not decode keeps its request id, as the Python SDK's
    /// validation error does, and is not retried: a 2xx is final.
    #[error("could not decode API response: {source}{}", request_id_suffix(.request_id.as_deref()))]
    Decode {
        /// What serde could not read.
        #[source]
        source: serde_json::Error,
        /// The response's `x-typesafe-request-id`, when the body came from an
        /// HTTP response that had one.
        request_id: Option<String>,
    },
    /// A recording could not be read or written. Check the directory and its
    /// permissions; `context` names what was being accessed.
    #[error("cannot access {context}: {source}")]
    Io {
        /// What was being accessed.
        context: String,
        /// Cause.
        #[source]
        source: std::io::Error,
    },
    /// A replay had no recording for the request. Run it once through a
    /// [`crate::Recorder`] over the same directory, then replay.
    #[error("no recording for request {0}; record it first")]
    NoRecording(String),
    /// A file in a recordings directory that is not a recording: a `.jud`
    /// document of another kind, or a `.jud` or `.json` file that does not
    /// parse. Fix or move the file.
    #[error("not a recording, {path}: {reason}")]
    InvalidRecording {
        /// The file.
        path: String,
        /// What is wrong with it.
        reason: String,
    },
    /// A question id was added twice to one request. Rename one; ids are the
    /// keys answers come back under, so they must be unique.
    #[error("duplicate question id {0:?}")]
    DuplicateQuestionId(String),
    /// A question was built with criteria the API would reject (too few or
    /// too many options or levels). Caught before sending.
    #[error("invalid question {id:?}: {reason}")]
    InvalidQuestion {
        /// Question id.
        id: String,
        /// What is wrong.
        reason: String,
    },
    /// The response has no answer under a question's id: the server (or a
    /// [`crate::Fake`]) left a question unanswered, so report the request
    /// id; from [`crate::Response::get`] it can also mean the handle belongs
    /// to a different request.
    #[error("no answer for question {id:?}{}", request_id_suffix(.request_id.as_deref()))]
    MissingAnswer {
        /// Question id.
        id: String,
        /// The response's `x-typesafe-request-id`, when it had one.
        request_id: Option<String>,
    },
    /// The answer under this id is a different primitive than its question
    /// or handle: the server answered another kind (report the request id),
    /// or the handle or the recording was made for another question set
    /// (fix the code or record again). An answer of a kind this release does
    /// not know ([`crate::Answer::Unknown`]) reads this way too, with its
    /// `type` as `actual`, escaped and cut; the remedy is to upgrade.
    #[error(
        "answer {id:?} is a {actual} but a {expected} was requested{}",
        request_id_suffix(.request_id.as_deref())
    )]
    AnswerTypeMismatch {
        /// Question id.
        id: String,
        /// Primitive the question or handle expected.
        expected: &'static str,
        /// The kind the server returned: `noul`, `choice`, `score`, or an
        /// unknown kind's escaped `type`.
        actual: String,
        /// The response's `x-typesafe-request-id`, when it had one.
        request_id: Option<String>,
    },
    /// A Choice answer named an option its question did not offer, as the
    /// chosen option or in its distribution. An error rather than a guess,
    /// because reading it as some other option would decide on an answer
    /// the model did not give. From the client, report the request id; from
    /// a recording, the enum changed since it was recorded.
    #[error(
        "answer {id:?} names option \"{}\", which its question does not offer{}",
        crate::answer::sanitize_server_str(.option),
        request_id_suffix(.request_id.as_deref())
    )]
    UnknownOption {
        /// Question id.
        id: String,
        /// The option string the API returned, whole.
        option: String,
        /// The response's `x-typesafe-request-id`, when it had one.
        request_id: Option<String>,
    },
    /// The answer is of the right primitive but does not describe its
    /// question: a Score off the scale the question sent (another legend, a
    /// probability keyed by a non-level, a score off the ends). From the
    /// client, report the request id. An application may raise it for its
    /// own reading of an answer, so its errors share one type.
    #[error(
        "answer {id:?} does not fit its question: {reason}{}",
        request_id_suffix(.request_id.as_deref())
    )]
    InvalidAnswer {
        /// Question id.
        id: String,
        /// What does not fit. It never quotes a level's text, which is the
        /// caller's own question and can be long.
        reason: String,
        /// The response's `x-typesafe-request-id`, when it had one.
        request_id: Option<String>,
    },
    /// A value outside `[0, 1]` given to [`crate::Probability::new`] or
    /// [`crate::Confidence::new`], directly or through a [`crate::Fake`]. On
    /// the wire it is [`Error::Decode`], since serde folds this into its
    /// message.
    #[error("value {value} is not a probability in [0, 1]")]
    NotAProbability {
        /// The offending value.
        value: f64,
    },
    /// A header the client or HTTP sets itself was given as a per-call or
    /// default header; the value is its lowercase name. Refused before
    /// anything is sent. `authorization`, `content-type` and `user-agent`
    /// are the client's own (another key means another client);
    /// `x-typesafe-retry-count` is not sent yet, but both official SDKs own
    /// it, so reserving it now means sending it later breaks no caller; the
    /// framing headers (`content-length`, `transfer-encoding`, `host`,
    /// `connection`, `te`, `upgrade`) would truncate or reframe the body or
    /// send the key to another virtual host than the base URL names.
    #[error("header {0:?} is set by the client and cannot be overridden")]
    ReservedHeader(String),
    /// A body field the client sets itself (`state`, `model` or `questions`)
    /// was given as a per-call extra field; the value is the name. Refused
    /// before anything is sent: a replaced `questions` or `state` is a
    /// request the response is not read against and a recording is not
    /// keyed by, and a replaced `model` would mislabel the span.
    #[error("body field {0:?} is set by the client and cannot be an extra field")]
    ReservedField(String),
    /// The base URL does not parse or cannot be joined with a path. Fix the
    /// URL given to the builder.
    #[error("invalid URL: {0}")]
    Url(String),
}

impl From<serde_json::Error> for Error {
    /// A serde failure with no response behind it: no request id.
    fn from(source: serde_json::Error) -> Self {
        Self::Decode {
            source,
            request_id: None,
        }
    }
}

impl Error {
    /// TypeSafe's `x-typesafe-request-id` for the response this error came
    /// from: the id to quote to TypeSafe support. `None` before anything was
    /// sent, for a recording, or on a transport failure. The match names
    /// every variant, so a new one has to choose.
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Unauthorized { request_id }
            | Self::PermissionDenied { request_id, .. }
            | Self::InvalidRequest { request_id, .. }
            | Self::RateLimited { request_id, .. }
            | Self::Overloaded { request_id, .. }
            | Self::Http { request_id, .. }
            | Self::Decode { request_id, .. }
            | Self::MissingAnswer { request_id, .. }
            | Self::AnswerTypeMismatch { request_id, .. }
            | Self::UnknownOption { request_id, .. }
            | Self::InvalidAnswer { request_id, .. } => request_id.as_deref(),
            #[cfg(feature = "http")]
            Self::Transport { .. } => None,
            Self::ResponseTooLarge { .. }
            | Self::MissingApiKey
            | Self::InvalidApiKey { .. }
            | Self::Io { .. }
            | Self::NoRecording(_)
            | Self::InvalidRecording { .. }
            | Self::DuplicateQuestionId(_)
            | Self::InvalidQuestion { .. }
            | Self::ReservedHeader(_)
            | Self::ReservedField(_)
            | Self::NotAProbability { .. }
            | Self::Url(_) => None,
        }
    }

    /// Adds the response's id to an answer-fit error that has none yet. The
    /// typed views read an answer without its response, so
    /// [`crate::Response::get`] adds the id here.
    pub(crate) fn with_request_id(mut self, id: Option<&str>) -> Self {
        if let Self::MissingAnswer { request_id, .. }
        | Self::AnswerTypeMismatch { request_id, .. }
        | Self::UnknownOption { request_id, .. }
        | Self::InvalidAnswer { request_id, .. } = &mut self
            && request_id.is_none()
        {
            *request_id = id.map(str::to_owned);
        }
        self
    }

    /// True for the four errors of a response that did not fit its
    /// questions ([`Error::MissingAnswer`], [`Error::AnswerTypeMismatch`],
    /// [`Error::UnknownOption`], [`Error::InvalidAnswer`]): the call went
    /// through and its answer cannot be used, so an evaluation harness
    /// records the case as failed and carries on. False for [`Error::Decode`].
    pub fn is_unfit(&self) -> bool {
        matches!(
            self,
            Self::MissingAnswer { .. }
                | Self::AnswerTypeMismatch { .. }
                | Self::UnknownOption { .. }
                | Self::InvalidAnswer { .. }
        )
    }

    /// True when the server refused the request for its size: an
    /// [`Error::InvalidRequest`] with status 413 (a compatible server's own
    /// limits) or whose `kind` is `max_tokens_exceeded` (the hosted API's
    /// 400 for a state over its token budget). Named because it is the one
    /// refused body fixed by sending less, state first.
    pub fn is_request_too_large(&self) -> bool {
        match self {
            Self::InvalidRequest { status: 413, .. } => true,
            Self::InvalidRequest {
                kind: Some(kind), ..
            } => kind == "max_tokens_exceeded",
            _ => false,
        }
    }
}

/// One invalid value a 400 or 422 body named: an entry of the `detail` list
/// in the OpenAPI document's `HTTPValidationError`, kept as data so code can
/// point at the question at fault. The entry's `input` and `ctx` are not
/// kept: `input` can be a piece of the state, and neither locates the field.
/// For a question the path runs `questions.<id>.<type>.<field>`, because
/// `FastAPI` puts the tag of the question's discriminated union in the
/// location. Non-exhaustive, so a field can be added in a minor release;
/// only this crate builds one.
///
/// ```compile_fail
/// // Outside this crate a struct literal does not compile.
/// let issue = judgment::ValidationIssue {
///     loc: vec!["body".to_owned()],
///     msg: "Field required".to_owned(),
///     kind: "missing".to_owned(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ValidationIssue {
    /// Where the invalid value is, as the server sent it: the request
    /// location (`body`) then field names and array indices, an integer
    /// segment kept as its decimal string.
    pub loc: Vec<String>,
    /// The server's explanation, for example `Field required`.
    pub msg: String,
    /// The server's machine-readable code (its `type`), for example
    /// `missing`; empty when the entry had none.
    pub kind: String,
}

impl ValidationIssue {
    /// The location as a dotted path without the leading `body`:
    /// `questions.urgency.score.criteria`. Only a leading `body` is dropped;
    /// the Python SDK drops every one, which would also remove a question
    /// whose id is `body`.
    pub fn path(&self) -> String {
        let segments = match self.loc.split_first() {
            Some((first, rest)) if first == "body" => rest,
            _ => self.loc.as_slice(),
        };
        segments.join(".")
    }
}

impl std::fmt::Display for ValidationIssue {
    /// `path: msg`, or `msg` alone when the location is empty.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path = self.path();
        if path.is_empty() {
            f.write_str(&self.msg)
        } else {
            write!(f, "{path}: {}", self.msg)
        }
    }
}

/// Convenience alias.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// The ` [request_id <id>]` clause of a message, empty when there is no id.
fn request_id_suffix(id: Option<&str>) -> String {
    id.map(|id| format!(" [request_id {id}]"))
        .unwrap_or_default()
}

/// `no body` for an empty or blank [`Error::Http`] body, so the message does
/// not end on a dangling colon; the field keeps what the server sent.
fn body_or_none(body: &str) -> &str {
    if body.trim().is_empty() {
        "no body"
    } else {
        body
    }
}

/// The server's-wait clause of a rate-limit message, empty when the server
/// named none. Public for the error types of other clients built on
/// [`crate::http`].
pub fn retry_after_suffix(retry_after: Option<Duration>) -> String {
    retry_after
        .map(|d| format!("; server asked to retry after {}s", d.as_secs_f64()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    /// The variants built from an HTTP error response, with the message
    /// each reads without an id.
    fn http_variants(request_id: Option<&str>) -> [(Error, &'static str); 8] {
        let id = || request_id.map(str::to_owned);
        [
            (
                Error::Unauthorized { request_id: id() },
                "authentication failed (401): check the API key",
            ),
            (
                Error::PermissionDenied {
                    detail: "model not enabled".into(),
                    request_id: id(),
                },
                "permission denied (403): model not enabled",
            ),
            (
                Error::InvalidRequest {
                    status: 422,
                    detail: "bad".into(),
                    issues: Vec::new(),
                    kind: None,
                    request_id: id(),
                },
                "request rejected by the API (422): bad",
            ),
            (
                Error::InvalidRequest {
                    status: 400,
                    detail: "model mismatch".into(),
                    issues: Vec::new(),
                    kind: None,
                    request_id: id(),
                },
                "request rejected by the API (400): model mismatch",
            ),
            (
                Error::RateLimited {
                    attempts: 3,
                    retry_after: Some(Duration::from_secs(2)),
                    request_id: id(),
                },
                "rate limited (429) after 3 attempts; server asked to retry after 2s",
            ),
            (
                Error::Overloaded {
                    attempts: 3,
                    request_id: id(),
                },
                "service overloaded (529) after 3 attempts",
            ),
            (
                Error::Http {
                    status: 500,
                    attempts: 3,
                    body: "boom".into(),
                    request_id: id(),
                },
                "unexpected HTTP status 500 after 3 attempts: boom",
            ),
            (
                Error::Http {
                    status: 503,
                    attempts: 1,
                    body: " \n".into(),
                    request_id: id(),
                },
                "unexpected HTTP status 503 after 1 attempts: no body",
            ),
        ]
    }

    #[test]
    fn request_id_is_read_from_the_http_variants_and_ends_their_message() {
        // Without an id the messages read exactly as before ids existed.
        for (err, message) in http_variants(None) {
            assert_eq!(err.request_id(), None, "{err:?}");
            assert_eq!(err.to_string(), message);
        }
        for (err, message) in http_variants(Some("req_1")) {
            assert_eq!(err.request_id(), Some("req_1"), "{err:?}");
            assert_eq!(err.to_string(), format!("{message} [request_id req_1]"));
        }

        // `?` on a serde error: a Decode with no response behind it.
        let decode = Error::from(serde_json::from_str::<u8>("x").unwrap_err());
        assert!(matches!(
            decode,
            Error::Decode {
                request_id: None,
                ..
            }
        ));
        assert_eq!(decode.request_id(), None);
        assert!(!decode.to_string().contains("request_id"), "{decode}");
        let decode = Error::Decode {
            source: serde_json::from_str::<u8>("x").unwrap_err(),
            request_id: Some("req_2".into()),
        };
        assert_eq!(decode.request_id(), Some("req_2"));
        assert!(
            decode.to_string().ends_with(" [request_id req_2]"),
            "{decode}"
        );

        for err in [
            Error::MissingApiKey,
            Error::InvalidApiKey {
                reason: "has whitespace inside it".into(),
            },
            Error::Url("nope".into()),
            Error::NoRecording("abc".into()),
            Error::ReservedHeader("authorization".into()),
            Error::ReservedField("model".into()),
        ] {
            assert_eq!(err.request_id(), None, "{err:?}");
        }
    }

    /// The four errors of a response that does not fit its questions.
    fn unfit(request_id: Option<&str>) -> [Error; 4] {
        let id = || request_id.map(str::to_owned);
        [
            Error::MissingAnswer {
                id: "q".into(),
                request_id: id(),
            },
            Error::AnswerTypeMismatch {
                id: "q".into(),
                expected: "noul",
                actual: "score".into(),
                request_id: id(),
            },
            Error::UnknownOption {
                id: "q".into(),
                option: "sales".into(),
                request_id: id(),
            },
            Error::InvalidAnswer {
                id: "q".into(),
                reason: "score 4 is outside 0..=3, the scale the question sent".into(),
                request_id: id(),
            },
        ]
    }

    #[test]
    fn is_unfit_names_the_four_fit_errors() {
        for err in unfit(None) {
            assert!(err.is_unfit(), "{err:?}");
        }
        let (http, _) = http_variants(Some("req_1"))
            .into_iter()
            .next()
            .unwrap_or_else(|| unreachable!());
        for err in [
            http,
            Error::MissingApiKey,
            Error::NotAProbability { value: 1.5 },
            Error::NoRecording("abc".into()),
            Error::DuplicateQuestionId("q".into()),
            Error::from(serde_json::from_str::<u8>("x").unwrap_err()),
        ] {
            assert!(!err.is_unfit(), "{err:?}");
        }
    }

    #[test]
    fn an_unfit_answer_carries_its_responses_request_id() {
        let messages = [
            r#"no answer for question "q""#,
            r#"answer "q" is a score but a noul was requested"#,
            r#"answer "q" names option "sales", which its question does not offer"#,
            r#"answer "q" does not fit its question: score 4 is outside 0..=3, the scale the question sent"#,
        ];
        for (err, message) in unfit(None).into_iter().zip(messages) {
            assert_eq!(err.request_id(), None, "{err:?}");
            assert_eq!(err.to_string(), message);
        }
        for (err, message) in unfit(Some("req_9")).into_iter().zip(messages) {
            assert_eq!(err.request_id(), Some("req_9"), "{err:?}");
            assert_eq!(err.to_string(), format!("{message} [request_id req_9]"));
        }
    }

    fn issue(loc: &[&str], msg: &str) -> ValidationIssue {
        ValidationIssue {
            loc: loc.iter().map(|s| (*s).to_owned()).collect(),
            msg: msg.into(),
            kind: "missing".into(),
        }
    }

    #[test]
    fn validation_issue_path_drops_only_the_leading_body() {
        // A question whose id is `body` keeps its segment; the Python SDK
        // would drop it.
        let nested = issue(&["body", "questions", "body", "noul"], "bad");
        assert_eq!(nested.path(), "questions.body.noul");
        assert_eq!(nested.to_string(), "questions.body.noul: bad");

        let question = issue(
            &["body", "questions", "urgency", "score", "criteria"],
            "short",
        );
        assert_eq!(question.path(), "questions.urgency.score.criteria");

        assert_eq!(issue(&["query", "limit"], "x").path(), "query.limit");
        assert_eq!(issue(&["body"], "Field required").path(), "");
        assert_eq!(
            issue(&["body"], "Field required").to_string(),
            "Field required"
        );
        assert_eq!(issue(&[], "Field required").to_string(), "Field required");
    }
}
