//! The retry loop every client over `reqwest` shares: [`RetryPolicy`] and
//! [`send_with_retries`].
//!
//! Public so that another client in the same application retries the same
//! way and reports to the same [`crate::Observer`]. The loop names no
//! vendor's header or status and returns the last response, headers
//! included, for each client to classify. `docs/concepts/how-judgment-works.md` ("Retries")
//! draws it.

use std::collections::BTreeSet;
use std::time::{Duration, SystemTime};

use reqwest::StatusCode;
use reqwest::header::{DATE, HeaderMap, HeaderName, RETRY_AFTER};

/// Not registered: a convention of several vendors' SDKs, read before
/// `Retry-After`.
const RETRY_AFTER_MS: HeaderName = HeaderName::from_static("retry-after-ms");

/// Which transport failures (no usable response) a [`RetryPolicy`] retries.
///
/// The levels split on whether the request left the process, because a
/// System One call is billed: a connection never made sent nothing, while a
/// timeout, a reset or a body cut short may have been processed. The SDKs'
/// two flags (connection error, timeout) do not answer that question.
/// [`BeforeSend`](Self::BeforeSend) relies on
/// [`reqwest::Error::is_connect`], so a misclassification only retries less.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum TransportRetry {
    /// Every transport failure, a body cut short included; the SDKs' default.
    #[default]
    Any,
    /// Only a failure to connect, when nothing was sent.
    BeforeSend,
    /// None.
    Never,
}

/// Which failures are retried, how many times, how long between attempts
/// and how far the server's own wait is trusted.
///
/// The defaults are the official SDKs': two retries; 0.5 s doubling to 5 s,
/// each wait `backoff × (1 − U·j)` for a uniform `U` in `[0, 1)` and jitter
/// `j` 0.25, so the nominal backoff is also the longest; 408, 429, every 5xx
/// (TypeSafe's 529 included) and every transport failure retried. A 2xx is
/// never retried, even when listed: a System One call is billed, and
/// re-sending a success would pay for it again.
///
/// # The server's wait
///
/// Read by [`parse_retry_after`] and honoured on any retried status, not
/// only 429. It replaces the backoff with no jitter: the server named a
/// time, and shortening it would retry before the server said it was ready.
/// Above `retry_after_max` the backoff applies instead, so a hostile or
/// misconfigured header cannot stall a caller for minutes.
///
/// # The budget
///
/// `budget` counts every attempt and every wait from the first send. A wait
/// that would end at or beyond it is not started and the last failure is
/// returned (tenacity's `stop_before_delay`, which the Python SDK uses), so
/// a 429 comes back as a rate limit error still carrying the server's wait.
/// An attempt in flight is never cut, so a call can run to just under the
/// budget plus one per-attempt timeout; `tokio::time::timeout` around the
/// call is the hard deadline. Off by default: the attempt count and the
/// per-attempt timeout already bound a call, and a default budget would
/// change the timing of every client that shares this loop.
///
/// ```
/// use std::time::Duration;
/// use judgment::{Client, RetryPolicy};
///
/// // Up to four retries, but none that would start 20 s or more after
/// // the first send.
/// let policy = RetryPolicy {
///     max_retries: 4,
///     budget: Some(Duration::from_secs(20)),
///     ..RetryPolicy::default()
/// };
/// let builder = Client::builder().retry(policy);
/// # let _ = builder;
/// ```
///
/// # Parity with the official SDKs (Python 0.7.1, JS 0.6.0)
///
/// Matches both: the count, backoff, jitter, status set and server's wait
/// above; a per-call policy that replaces the client's whole policy
/// ([`crate::client::CallOptions::retry`]), as in the Python SDK; a wait
/// above the cap falling back to the backoff (the JS SDK's
/// `maxRetryAfterMs`); the budget (the Python SDK's `timeout`); dropping
/// the future cancelling the call, a wait included.
///
/// Deliberately differs: the budget is off by default (Python 30 s, JS
/// none); the cap is 30 s (JS 60 s, Python none); an HTTP date is measured
/// against the response's `Date` header (the SDKs use the local clock); an
/// empty or unrepresentable header is ignored (the SDKs retry at once or
/// honour it); one three-level `transport` setting replaces
/// the flags `api_connection_error` and `api_timeout_error`
/// ([`TransportRetry`]); a jitter outside `[0, 1]` is clamped, not an
/// error; dates are the three RFC 9110 forms only (JS `Date.parse` also
/// takes ISO 8601, Python's `email.utils.parsedate_to_datetime` RFC 5322
/// forms); [`RetryPolicy::conservative`] and `max_body_bytes` have no SDK
/// equivalent, both SDKs buffering whatever the server sends.
///
/// Not implemented: `respect_retry_after` (set `retry_after_max` to zero,
/// which still takes a zero wait); exception and predicate hooks (a closure
/// field would cost the type `PartialEq` and `Debug`);
/// `X-TypeSafe-Retry-Count`, which both SDKs send on a retry, reserved
/// ([`crate::Error::ReservedHeader`]) so that sending it later breaks no
/// caller.
#[derive(Debug, Clone, PartialEq)]
pub struct RetryPolicy {
    /// Retries after the first attempt; 0 disables retries.
    pub max_retries: u32,
    /// First backoff delay; doubled each retry.
    pub backoff_initial: Duration,
    /// Cap on the computed backoff.
    pub backoff_max: Duration,
    /// Share of each backoff that may be taken off at random, so clients that
    /// failed together do not retry in lockstep. Clamped to `[0, 1]`.
    pub backoff_jitter: f64,
    /// Statuses retried, never a 2xx. Default 408, 429 and 500 to 599:
    /// transient by definition, where 400, 401, 403 and 422 are a request
    /// body, a key or an account's access that a retry cannot fix.
    pub http_statuses: BTreeSet<u16>,
    /// Longest server wait honoured; above it the backoff applies ("The
    /// server's wait" above). Default 30 s.
    pub retry_after_max: Duration,
    /// Which transport failures are retried; default [`TransportRetry::Any`].
    pub transport: TransportRetry,
    /// Time from the first send that retrying may use ("The budget" above);
    /// `None` (the default) for no budget, `Some(Duration::ZERO)` for none.
    pub budget: Option<Duration>,
    /// Most bytes of a response body the loop buffers, 8 MiB by default; over
    /// it the call is [`Exhausted::TooLarge`], never retried since the next
    /// attempt would carry the same body. Every body is decoded from that
    /// buffer, so an upstream, or a proxy answering in its place, cannot grow
    /// the process without bound.
    pub max_body_bytes: usize,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            backoff_initial: Duration::from_millis(500),
            backoff_max: Duration::from_secs(5),
            backoff_jitter: 0.25,
            http_statuses: [408, 429].into_iter().chain(500..=599).collect(),
            retry_after_max: Duration::from_secs(30),
            transport: TransportRetry::Any,
            budget: None,
            max_body_bytes: 8 * 1024 * 1024,
        }
    }
}

impl RetryPolicy {
    /// No retries at all.
    pub fn none() -> Self {
        Self {
            max_retries: 0,
            ..Self::default()
        }
    }

    /// Retry only what cannot have been charged twice: 408, 429, and a
    /// connection that was never made.
    ///
    /// A System One call is billed (the OpenAPI document calls
    /// `Usage.input_tokens` the "Number of billable input tokens") and the
    /// API reference does not say whether a 5xx or a timeout was charged. A
    /// 408 (RFC 9110 §15.5.9) and a 429 (RFC 6585 §4) say the server did not
    /// process the request; a 529 gets the same retry advice as a 429 but no
    /// such promise, so it is excluded. The cost is a call failed on the
    /// first 5xx or timeout a retry would have saved.
    pub fn conservative() -> Self {
        Self {
            http_statuses: BTreeSet::from([408, 429]),
            transport: TransportRetry::BeforeSend,
            ..Self::default()
        }
    }

    /// Whether `status` is retried: in [`http_statuses`](Self::http_statuses) and not a success.
    pub fn is_retryable(&self, status: StatusCode) -> bool {
        !status.is_success() && self.http_statuses.contains(&status.as_u16())
    }

    /// Whether the transport failure `e` is retried, by [`transport`](Self::transport).
    pub fn retries_transport(&self, e: &reqwest::Error) -> bool {
        match self.transport {
            TransportRetry::Any => true,
            TransportRetry::BeforeSend => e.is_connect(),
            TransportRetry::Never => false,
        }
    }

    /// The wait before retry number `retry` (1-based): the server's wait
    /// within `retry_after_max`, otherwise the backoff less the jitter.
    pub fn delay(&self, retry: u32, retry_after: Option<Duration>) -> Duration {
        self.delay_with(retry, retry_after, fastrand::f64())
    }

    /// [`delay`](Self::delay) with the random draw `unit` passed in, so the arithmetic is testable.
    fn delay_with(&self, retry: u32, retry_after: Option<Duration>, unit: f64) -> Duration {
        if let Some(ra) = retry_after
            && ra <= self.retry_after_max
        {
            return ra;
        }
        let exp = self
            .backoff_initial
            .saturating_mul(2u32.saturating_pow(retry.saturating_sub(1)))
            .min(self.backoff_max);
        // `NaN > 0.0` is false, so a NaN jitter reads as no jitter.
        let jitter = if self.backoff_jitter > 0.0 {
            self.backoff_jitter.min(1.0)
        } else {
            0.0
        };
        if jitter == 0.0 {
            return exp;
        }
        // Never `mul_f64`: `Duration::MAX.mul_f64(1.0)` panics, since the
        // product rounds above the largest representable duration.
        let cut = Duration::try_from_secs_f64(exp.as_secs_f64() * unit.clamp(0.0, 1.0) * jitter)
            .unwrap_or(exp);
        exp.saturating_sub(cut)
    }

    /// The wait before the retry that follows attempt `attempt`, or `None`
    /// when retries are used up or the wait would reach the budget.
    fn next_wait(
        &self,
        attempt: u32,
        retry_after: Option<Duration>,
        elapsed: Duration,
    ) -> Option<Duration> {
        if attempt > self.max_retries {
            return None;
        }
        let delay = self.delay(attempt, retry_after);
        if let Some(budget) = self.budget
            && elapsed.saturating_add(delay) >= budget
        {
            tracing::warn!(
                attempt,
                ?delay,
                ?elapsed,
                ?budget,
                "retry budget spent; returning the last failure"
            );
            return None;
        }
        Some(delay)
    }
}

/// The last response of a retry loop, for the caller to classify.
/// Non-exhaustive so more of the response can be handed back without a
/// breaking change.
#[derive(Debug)]
#[non_exhaustive]
pub struct Completed {
    /// HTTP status.
    pub status: StatusCode,
    /// Response body as text.
    pub body: String,
    /// Total attempts made, including the first.
    pub attempts: u32,
    /// The last response's wait, when it named one ([`parse_retry_after`]).
    pub retry_after: Option<Duration>,
    /// The last response's headers, for each client to read its own
    /// upstream's (a request id, say), so the loop names no vendor.
    pub headers: HeaderMap,
}

/// The retry loop gave up without a response the caller can classify.
/// Exhaustive on purpose: every client must map a new kind, as
/// [`crate::Error::request_id`] names every variant.
#[derive(Debug)]
pub enum Exhausted {
    /// A transport-level failure the policy stopped retrying.
    Transport {
        /// Total attempts made, including the first.
        attempts: u32,
        /// The last error.
        source: reqwest::Error,
    },
    /// The last response's body was over
    /// [`max_body_bytes`](RetryPolicy::max_body_bytes).
    TooLarge {
        /// Total attempts made, including the first.
        attempts: u32,
        /// The cap that was passed, in bytes.
        limit: usize,
    },
}

/// Send `make()` until it yields a response the policy does not retry, or
/// the policy stops (`docs/concepts/how-judgment-works.md`, "Retries", draws the loop).
///
/// A status the policy does not retry, or the last retried one when it
/// stops, is `Ok(Completed)`; the last transport failure, or a body over
/// [`max_body_bytes`](RetryPolicy::max_body_bytes), is `Err(Exhausted)`. The
/// body is decoded as UTF-8 with invalid sequences replaced, since every
/// upstream answers in JSON. Every failed attempt, retried or not, goes to
/// the global [`crate::Observer`] under `service` as the status, `transport`
/// or `too_large`: a retry that succeeds hides the failure from the caller,
/// but the attempt was still load on the upstream.
pub async fn send_with_retries(
    policy: &RetryPolicy,
    service: &'static str,
    make: impl Fn() -> reqwest::RequestBuilder,
) -> Result<Completed, Exhausted> {
    let started = tokio::time::Instant::now();
    let mut attempt: u32 = 0;
    loop {
        attempt += 1;
        match make().send().await {
            Ok(mut resp) => {
                let status = resp.status();
                let retry_after = parse_retry_after(resp.headers());
                if !status.is_success() {
                    crate::observer::global().on_failed_attempt(service, status.as_str());
                }
                let headers = std::mem::take(resp.headers_mut());
                let body = match read_body(resp, policy.max_body_bytes).await {
                    Ok(b) => b,
                    Err(BodyRead::TooLarge) => {
                        crate::observer::global().on_failed_attempt(service, "too_large");
                        tracing::warn!(
                            attempt,
                            status = status.as_u16(),
                            limit = policy.max_body_bytes,
                            "response body over the cap; not retried"
                        );
                        return Err(Exhausted::TooLarge {
                            attempts: attempt,
                            limit: policy.max_body_bytes,
                        });
                    }
                    Err(BodyRead::Transport(source)) => {
                        crate::observer::global().on_failed_attempt(service, "transport");
                        if policy.retries_transport(&source)
                            && let Some(delay) = policy.next_wait(attempt, None, started.elapsed())
                        {
                            tracing::warn!(attempt, ?delay, error = %source, "body read failed; retrying");
                            tokio::time::sleep(delay).await;
                            continue;
                        }
                        return Err(Exhausted::Transport {
                            attempts: attempt,
                            source,
                        });
                    }
                };
                if policy.is_retryable(status)
                    && let Some(delay) = policy.next_wait(attempt, retry_after, started.elapsed())
                {
                    tracing::warn!(
                        attempt,
                        status = status.as_u16(),
                        ?delay,
                        "retryable status; retrying"
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Ok(Completed {
                    status,
                    body,
                    attempts: attempt,
                    retry_after,
                    headers,
                });
            }
            Err(source) => {
                crate::observer::global().on_failed_attempt(service, "transport");
                if policy.retries_transport(&source)
                    && let Some(delay) = policy.next_wait(attempt, None, started.elapsed())
                {
                    tracing::warn!(attempt, ?delay, error = %source, "transport error; retrying");
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Err(Exhausted::Transport {
                    attempts: attempt,
                    source,
                });
            }
        }
    }
}

/// Why [`read_body`] stopped.
enum BodyRead {
    TooLarge,
    Transport(reqwest::Error),
}

/// The body, refusing more than `limit` bytes, before a read when `Content-Length` says so.
async fn read_body(mut resp: reqwest::Response, limit: usize) -> Result<String, BodyRead> {
    let declared = resp.content_length();
    if declared.is_some_and(|n| n > u64::try_from(limit).unwrap_or(u64::MAX)) {
        return Err(BodyRead::TooLarge);
    }
    let mut buf = Vec::with_capacity(declared.and_then(|n| usize::try_from(n).ok()).unwrap_or(0));
    while let Some(chunk) = resp.chunk().await.map_err(BodyRead::Transport)? {
        if buf.len().saturating_add(chunk.len()) > limit {
            return Err(BodyRead::TooLarge);
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8(buf)
        .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()))
}

/// The server's wait ("The server's wait" on [`RetryPolicy`]):
/// `retry-after-ms`, then `Retry-After` in seconds, then as an HTTP date in
/// the three RFC 9110 forms. A date is measured against the
/// response's own `Date` header when it parses, so a local clock that
/// disagrees with the server's neither stretches nor cuts the wait; a past
/// date means retry now. A value that is empty, negative, not finite or too
/// large for a [`Duration`] is ignored: a bad `retry-after-ms` leaves
/// `Retry-After` to decide, and a bad `Retry-After` leaves the backoff.
pub fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
    parse_retry_after_at(headers, SystemTime::now())
}

/// [`parse_retry_after`] with the local clock passed in.
fn parse_retry_after_at(headers: &HeaderMap, now: SystemTime) -> Option<Duration> {
    if let Some(ms) = header_str(headers, &RETRY_AFTER_MS)
        && let Ok(ms) = ms.parse::<f64>()
        && ms.is_finite()
        && ms >= 0.0
        && let Ok(wait) = Duration::try_from_secs_f64(ms / 1000.0)
    {
        return Some(wait);
    }
    let raw = header_str(headers, &RETRY_AFTER)?;
    if let Ok(secs) = raw.parse::<f64>() {
        // `try_from`, never `from_secs_f64`: `inf` or `1e20` would panic.
        return (secs.is_finite() && secs >= 0.0)
            .then(|| Duration::try_from_secs_f64(secs).ok())
            .flatten();
    }
    let at = httpdate::parse_http_date(raw).ok()?;
    let reference = header_str(headers, &DATE)
        .and_then(|d| httpdate::parse_http_date(d).ok())
        .unwrap_or(now);
    Some(at.duration_since(reference).unwrap_or(Duration::ZERO))
}

/// A header's value, trimmed; empty or not visible ASCII reads as absent.
fn header_str<'a>(headers: &'a HeaderMap, name: &HeaderName) -> Option<&'a str> {
    let value = headers.get(name)?.to_str().ok()?.trim();
    (!value.is_empty()).then_some(value)
}

pub use crate::error::retry_after_suffix;

/// Truncate a response body for inclusion in an error message.
pub fn truncate(s: String) -> String {
    truncate_to(s, 2_000)
}

/// `s` cut to at most `max` bytes on a character boundary, `…` appended when
/// cut. The cap is a parameter so the bounded proof reaches the cutting path.
fn truncate_to(mut s: String, max: usize) -> String {
    if s.len() > max {
        let cut = s.floor_char_boundary(max);
        s.truncate(cut);
        s.push('…');
    }
    s
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use reqwest::header::HeaderValue;

    fn no_jitter() -> RetryPolicy {
        RetryPolicy {
            backoff_jitter: 0.0,
            ..RetryPolicy::default()
        }
    }

    fn headers(pairs: &[(&HeaderName, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (name, value) in pairs {
            h.insert((*name).clone(), HeaderValue::from_str(value).unwrap());
        }
        h
    }

    /// A whole second, so a date formatted from it parses back exactly.
    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn retry_after_wins_when_within_cap() {
        let p = RetryPolicy::default();
        assert_eq!(
            p.delay(1, Some(Duration::from_secs(3))),
            Duration::from_secs(3)
        );
        let d = p.delay(1, Some(Duration::from_secs(600)));
        assert!(d <= Duration::from_millis(500), "{d:?}");
    }

    #[test]
    fn backoff_doubles_and_caps() {
        let p = no_jitter();
        assert_eq!(p.delay(1, None), Duration::from_millis(500));
        assert_eq!(p.delay(2, None), Duration::from_millis(1000));
        assert_eq!(p.delay(3, None), Duration::from_millis(2000));
        assert_eq!(p.delay(10, None), Duration::from_secs(5));
    }

    #[test]
    fn jitter_only_subtracts() {
        let p = RetryPolicy::default();
        assert_eq!(p.delay_with(1, None, 0.0), Duration::from_millis(500));
        assert_eq!(p.delay_with(1, None, 1.0), Duration::from_millis(375));
        for _ in 0..1000 {
            let d = p.delay(1, None);
            assert!(
                (Duration::from_millis(375)..=Duration::from_millis(500)).contains(&d),
                "{d:?}"
            );
        }

        let nan = RetryPolicy {
            backoff_jitter: f64::NAN,
            ..RetryPolicy::default()
        };
        assert_eq!(nan.delay_with(1, None, 1.0), Duration::from_millis(500));
        let negative = RetryPolicy {
            backoff_jitter: -0.5,
            ..RetryPolicy::default()
        };
        assert_eq!(
            negative.delay_with(1, None, 1.0),
            Duration::from_millis(500)
        );
        let seven = RetryPolicy {
            backoff_jitter: 7.0,
            ..RetryPolicy::default()
        };
        assert_eq!(seven.delay_with(1, None, 1.0), Duration::ZERO);

        for jitter in [0.0, 0.25] {
            let huge = RetryPolicy {
                backoff_initial: Duration::MAX,
                backoff_max: Duration::MAX,
                backoff_jitter: jitter,
                ..RetryPolicy::default()
            };
            for unit in [0.0, 0.5, 1.0] {
                assert!(huge.delay_with(3, None, unit) <= Duration::MAX);
            }
            let _ = huge.delay(u32::MAX, None);
        }
    }

    #[test]
    fn statuses_default_and_conservative() {
        let status = |code| StatusCode::from_u16(code).unwrap();
        let default = RetryPolicy::default();
        for code in [408, 429, 500, 503, 529, 599] {
            assert!(default.is_retryable(status(code)), "{code}");
        }
        for code in [400, 401, 403, 404, 422, 600] {
            assert!(!default.is_retryable(status(code)), "{code}");
        }
        let conservative = RetryPolicy::conservative();
        for code in [408, 429] {
            assert!(conservative.is_retryable(status(code)), "{code}");
        }
        for code in [500, 503, 529] {
            assert!(!conservative.is_retryable(status(code)), "{code}");
        }
        assert_eq!(conservative.transport, TransportRetry::BeforeSend);
        assert_eq!(
            RetryPolicy {
                http_statuses: default.http_statuses.clone(),
                transport: TransportRetry::Any,
                ..conservative
            },
            default,
            "conservative() changes only the statuses and the transport level"
        );
    }

    #[test]
    fn a_listed_success_is_never_retried() {
        let status = |code| StatusCode::from_u16(code).unwrap();
        let everything = RetryPolicy {
            http_statuses: (100..=599).collect(),
            ..RetryPolicy::default()
        };
        for code in [200, 201, 204, 299] {
            assert!(!everything.is_retryable(status(code)), "{code}");
        }
        for code in [304, 404, 503] {
            assert!(everything.is_retryable(status(code)), "{code}");
        }
    }

    #[test]
    fn budget_stops_before_a_wait_that_reaches_it() {
        let ms = Duration::from_millis;
        let p = RetryPolicy {
            budget: Some(Duration::from_secs(1)),
            ..no_jitter()
        };
        // The first wait is 500 ms: 400 + 500 stays under the 1 s budget, 500 + 500 reaches it.
        assert_eq!(p.next_wait(1, None, ms(400)), Some(ms(500)));
        assert_eq!(p.next_wait(1, None, ms(500)), None);
        assert_eq!(p.next_wait(1, Some(Duration::from_secs(2)), ms(0)), None);
        assert_eq!(p.next_wait(3, None, ms(0)), None);
        assert_eq!(no_jitter().next_wait(3, None, ms(0)), None);
        assert_eq!(
            no_jitter().next_wait(2, None, Duration::from_secs(3600)),
            Some(ms(1000))
        );
        let zero = RetryPolicy {
            budget: Some(Duration::ZERO),
            ..no_jitter()
        };
        assert_eq!(zero.next_wait(1, None, ms(0)), None);
        assert_eq!(zero.next_wait(1, Some(Duration::ZERO), ms(0)), None);
    }

    #[test]
    fn retry_after_forms_and_precedence() {
        let now = at(1_700_000_000);
        let parse = |pairs: &[(&HeaderName, &str)]| parse_retry_after_at(&headers(pairs), now);

        assert_eq!(
            parse(&[(&RETRY_AFTER, "2.5")]),
            Some(Duration::from_millis(2500))
        );
        assert_eq!(
            parse(&[(&RETRY_AFTER, " 3 ")]),
            Some(Duration::from_secs(3))
        );
        assert_eq!(
            parse(&[(&RETRY_AFTER_MS, "120")]),
            Some(Duration::from_millis(120))
        );
        assert_eq!(
            parse(&[(&RETRY_AFTER_MS, "120"), (&RETRY_AFTER, "7")]),
            Some(Duration::from_millis(120)),
            "retry-after-ms wins"
        );
        for fallthrough in ["-1", "soon", "", "inf", "1e400"] {
            assert_eq!(
                parse(&[(&RETRY_AFTER_MS, fallthrough), (&RETRY_AFTER, "7")]),
                Some(Duration::from_secs(7)),
                "retry-after-ms {fallthrough:?} falls through"
            );
        }
        assert_eq!(parse(&[(&RETRY_AFTER_MS, "-1")]), None);

        let ahead = httpdate::fmt_http_date(now + Duration::from_secs(4));
        assert_eq!(
            parse(&[(&RETRY_AFTER, &ahead)]),
            Some(Duration::from_secs(4))
        );
        assert_eq!(
            parse(&[(&RETRY_AFTER, "Wed, 21 Oct 2015 07:28:00 GMT")]),
            Some(Duration::ZERO),
            "a past date means retry now"
        );

        let retry_at = at(1_600_000_000);
        let pairs = [
            (&RETRY_AFTER, httpdate::fmt_http_date(retry_at)),
            (
                &DATE,
                httpdate::fmt_http_date(retry_at - Duration::from_secs(10)),
            ),
        ];
        let pairs: Vec<(&HeaderName, &str)> = pairs.iter().map(|(n, v)| (*n, v.as_str())).collect();
        for local in [now, at(0), at(1_600_000_005)] {
            assert_eq!(
                parse_retry_after_at(&headers(&pairs), local),
                Some(Duration::from_secs(10))
            );
        }
        assert_eq!(
            parse(&[(&RETRY_AFTER, &ahead), (&DATE, "yesterday")]),
            Some(Duration::from_secs(4))
        );

        // RFC 850 and asctime, the two obsolete forms RFC 9110 still accepts.
        let date = [(&DATE, "Sun, 06 Nov 1994 08:49:30 GMT")];
        for form in ["Sunday, 06-Nov-94 08:49:37 GMT", "Sun Nov  6 08:49:37 1994"] {
            assert_eq!(
                parse(&[(&RETRY_AFTER, form), date[0]]),
                Some(Duration::from_secs(7)),
                "{form}"
            );
        }
        assert_eq!(parse(&[]), None);
    }

    #[test]
    fn hostile_retry_after_is_ignored_not_a_panic() {
        for value in ["inf", "infinity", "NaN", "1e400", "-1", "", "soon", "1e20"] {
            let h = headers(&[(&RETRY_AFTER, value)]);
            assert_eq!(parse_retry_after(&h), None, "Retry-After {value:?}");
        }
        for value in ["inf", "infinity", "NaN", "1e400", "-1", "", "soon", "1e30"] {
            let h = headers(&[(&RETRY_AFTER_MS, value)]);
            assert_eq!(parse_retry_after(&h), None, "retry-after-ms {value:?}");
        }
        // Representable, so parsed, then above the cap, so the backoff applies.
        let h = headers(&[(&RETRY_AFTER_MS, "1e20")]);
        let huge = parse_retry_after(&h).unwrap();
        assert!(RetryPolicy::default().delay(1, Some(huge)) <= Duration::from_millis(500));
        let mut h = HeaderMap::new();
        h.insert(RETRY_AFTER, HeaderValue::from_bytes(b"\xff").unwrap());
        assert_eq!(parse_retry_after(&h), None);
    }
}

/// Bounded proofs, run with `cargo kani`; the bounds and their reasons are
/// in `docs/guides/record-replay-and-test.md`, "Bounded proofs".
#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Any `Duration`: `Duration::new` panics only when a nanosecond carry
    /// overflows the seconds, which a part below one second never causes.
    fn any_duration() -> Duration {
        let nanos: u32 = kani::any();
        kani::assume(nanos < 1_000_000_000);
        Duration::new(kani::any(), nanos)
    }

    /// Bounded, or the solver does not finish; `Duration::MAX` keeps the saturating path covered.
    fn policy_duration() -> Duration {
        if kani::any() {
            return Duration::MAX;
        }
        let ms: u64 = kani::any();
        kani::assume(ms <= 3_600_000);
        Duration::from_millis(ms)
    }

    /// Stub for `Duration::try_from_secs_f64`, whose float decoding keeps the
    /// solver from finishing: any result, so what holds for every one holds
    /// for the real function, which never panics by `std`'s contract.
    fn any_try_from_secs_f64(_secs: f64) -> Result<Duration, std::time::TryFromFloatSecsError> {
        if kani::any() {
            Ok(any_duration())
        } else {
            // The same error type, from a call that always fails.
            Duration::try_from_secs_f32(-1.0)
        }
    }

    /// Run with `-Z stubbing`.
    #[kani::proof]
    #[kani::stub(std::time::Duration::try_from_secs_f64, any_try_from_secs_f64)]
    fn delay_never_panics_and_stays_within_the_policy() {
        let policy = RetryPolicy {
            max_retries: kani::any(),
            backoff_initial: policy_duration(),
            backoff_max: policy_duration(),
            backoff_jitter: kani::any(),
            // Not read by `delay_with`; empty keeps the state small.
            http_statuses: BTreeSet::new(),
            retry_after_max: policy_duration(),
            transport: TransportRetry::Any,
            budget: None,
            max_body_bytes: kani::any(),
        };
        // Past ten retries the backoff sits at `backoff_max` anyway.
        let retry: u32 = kani::any();
        kani::assume(retry <= 10);
        let retry_after = if kani::any() {
            Some(any_duration())
        } else {
            None
        };
        let unit: f64 = kani::any();

        let delay = policy.delay_with(retry, retry_after, unit);

        match retry_after {
            Some(ra) if ra <= policy.retry_after_max => assert_eq!(delay, ra),
            _ => {
                let nominal = policy
                    .backoff_initial
                    .saturating_mul(2u32.saturating_pow(retry.saturating_sub(1)))
                    .min(policy.backoff_max);
                assert!(delay <= policy.backoff_max);
                assert!(delay <= nominal);
                if !(policy.backoff_jitter > 0.0) {
                    assert_eq!(delay, nominal);
                }
            }
        }
        kani::cover!(
            retry_after.is_none() && policy.backoff_jitter > 0.0 && delay < policy.backoff_max
        );
        kani::cover!(retry_after.is_some_and(|ra| ra > policy.retry_after_max));
    }

    #[kani::proof]
    #[kani::unwind(8)]
    fn truncate_cuts_on_a_char_boundary_within_the_cap() {
        const N: usize = 6;
        let bytes: [u8; N] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= N);
        let Ok(text) = core::str::from_utf8(&bytes[..len]) else {
            return;
        };
        let max: usize = kani::any();
        kani::assume(max <= N);

        let out = truncate_to(text.to_owned(), max);

        if text.len() <= max {
            assert!(out == text);
        } else {
            assert!(out.ends_with('…'));
            let kept = &out[..out.len() - '…'.len_utf8()];
            assert!(kept.len() <= max);
            assert!(text.starts_with(kept));
            assert!(text.is_char_boundary(kept.len()));
            assert!(max - kept.len() < 4);
        }
        kani::cover!(text.len() > max && !text.is_char_boundary(max));
    }
}
