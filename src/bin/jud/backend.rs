//! The backend a run talks to, and the ways a run fails. Shared by every
//! subcommand that asks a model or replays recordings: `jud RUBRIC`, and
//! `jud record`, `eval` and `tune` (decision 0021).

use std::future::Future;
use std::path::Path;
use std::process::ExitCode;

use judgment::{Assets, Error, Replay, SystemOne};
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{EXIT_BACKEND, EXIT_USAGE, config};

#[derive(Debug)]
pub(crate) enum Failure {
    /// Fixable before any call: the file, the state, the configuration.
    Usage(String),
    /// The call was made and failed, or its answer did not fit.
    Backend(String),
    /// Under a replay: no recording answers this request. Exit status 1
    /// like any backend failure, but a distinct variant because a run over
    /// many cases keeps going to name every case that has none, where any
    /// other failure stops it.
    Missing(String),
}

impl Failure {
    /// The message, without the `jud: ` prefix.
    pub(crate) fn message(&self) -> &str {
        match self {
            Self::Usage(m) | Self::Backend(m) | Self::Missing(m) => m,
        }
    }

    /// The exit status: 2 for a usage failure, 1 for the others.
    pub(crate) fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) => EXIT_USAGE,
            Self::Backend(_) | Self::Missing(_) => EXIT_BACKEND,
        }
    }

    /// Print `jud: MESSAGE` on stderr and give the exit status.
    pub(crate) fn report(&self) -> ExitCode {
        crate::out::note!("jud: {}", self.message());
        ExitCode::from(self.exit_code())
    }
}

/// Which backend answers: the configured server, or the recordings under
/// a directory. Both are the crate's; the run is the same either side.
#[allow(
    clippy::large_enum_variant,
    reason = "one value per run; the client's size is not worth a box"
)]
enum Kind {
    Server {
        client: judgment::Client,
        resolved: config::Resolved,
    },
    Recordings {
        replay: Replay,
        dir: String,
    },
}

pub(crate) struct Backend {
    kind: Kind,
    /// One runtime for the life of the run, so a loop over cases keeps the
    /// client's connection pool alive instead of tearing it down per call.
    runtime: Runtime,
}

fn runtime() -> Result<Runtime, Failure> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| Failure::Backend(format!("cannot start the runtime: {e}")))
}

impl Backend {
    /// The recordings under `replay`, or the configured server;
    /// `long_context` is the run's own `--long-context` (`Some(true)` when
    /// given), which wins over the configuration file and means nothing
    /// under a replay.
    pub(crate) fn open(replay: Option<&Path>, long_context: Option<bool>) -> Result<Self, Failure> {
        let kind = if let Some(dir) = replay {
            let replay = Replay::open(dir).map_err(|e| {
                Failure::Usage(format!("cannot replay from {}: {e}", dir.display()))
            })?;
            Kind::Recordings {
                replay,
                dir: dir.display().to_string(),
            }
        } else {
            let resolved =
                config::resolve_with(long_context).map_err(|e| Failure::Usage(e.to_string()))?;
            let client = resolved
                .client()
                .map_err(|e| Failure::Usage(e.to_string()))?;
            // Before any call: a misspelt variable is why a run would ask a
            // server it was not meant to.
            resolved.warn_ignored_environment();
            Kind::Server { client, resolved }
        };
        Ok(Self {
            kind,
            runtime: runtime()?,
        })
    }

    pub(crate) fn model(&self) -> &str {
        match &self.kind {
            Kind::Server { resolved, .. } => resolved.model(),
            Kind::Recordings { .. } => judgment::client::DEFAULT_MODEL,
        }
    }

    /// Answer one request, its images included, from the server or the
    /// recordings.
    pub(crate) fn answer(
        &self,
        state: &Value,
        questions: &judgment::Questions,
        assets: &Assets,
    ) -> Result<judgment::Response, Failure> {
        match &self.kind {
            Kind::Server { client, resolved } => self
                .runtime
                .block_on(client.answer_with_assets(state, self.model(), questions, assets))
                // The error carries what the server sent: shown, never obeyed.
                .map_err(|e| {
                    Failure::Backend(format!(
                        "the backend at {} failed: {}",
                        public_url(resolved.base_url()),
                        crate::out::plain(&e.to_string())
                    ))
                }),
            Kind::Recordings { replay, dir } => self
                .runtime
                .block_on(replay.answer_with_assets(state, self.model(), questions, assets))
                .map_err(|e| match e {
                    // The wording `jud RUBRIC --replay` has always used for a
                    // state nobody recorded.
                    Error::NoRecording(_) => Failure::Missing(format!(
                        "no recording under {dir} answers this state and rubric: {}",
                        crate::out::plain(&e.to_string())
                    )),
                    // A recording is there and cannot answer: the request
                    // matched and the response no longer fits the questions,
                    // which is a different thing to fix than a missing one.
                    other => Failure::Backend(format!(
                        "a recording under {dir} matches this state and rubric but cannot answer it: {}",
                        crate::out::plain(&other.to_string())
                    )),
                }),
        }
    }

    /// Before a run that asks a server once per request (`jud record`,
    /// `jud eval`): one line naming the model and the server, and where the
    /// base URL came from, so a run aimed at the wrong server, the hosted
    /// API by default, can be stopped before it has paid for every request.
    /// The count is of the requests planned, before `jud record` knows
    /// which its directory already answers, hence "up to". Nothing under a
    /// replay, which costs nothing. Long context is named only when it is
    /// on: it is the one setting that changes what the server does with
    /// every request, and it is off by default.
    pub(crate) fn announce(&self, requests: usize) {
        if let Kind::Server { resolved, .. } = &self.kind {
            let long_context = if resolved.long_context() {
                format!(", long context {}", resolved.describe_long_context())
            } else {
                String::new()
            };
            crate::out::note!(
                "asking {} at {} for up to {requests} request{} (base URL from {}{long_context})",
                resolved.model(),
                public_url(resolved.base_url()),
                if requests == 1 { "" } else { "s" },
                resolved.base_url_from().replace('_', " ")
            );
        }
    }

    /// Run a future to completion on the run's own runtime.
    pub(crate) fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }

    /// The server that answers, as it is safe to write into a file or a
    /// message: no credentials, no query. `None` when recordings answer.
    pub(crate) fn server(&self) -> Option<String> {
        match &self.kind {
            Kind::Server { resolved, .. } => Some(public_url(resolved.base_url())),
            Kind::Recordings { .. } => None,
        }
    }
}

/// `url` as it may be written into a recording, a tuning block or a
/// message: without the userinfo (`https://user:token@host/`), the query and
/// the fragment, which can carry a credential and which a recording that is
/// meant to be committed must not. A string that is not a URL is returned
/// as it is; one the split below cannot place (no `://`, as `https:user:pw@host`,
/// or extra slashes after it) goes through the URL parser, which clears the
/// parts — and one the parser refuses too still has its authority stripped
/// by hand, so no spelling keeps a credential by being malformed enough.
pub(crate) fn public_url(url: &str) -> String {
    // Extra slashes (`https:///user@host`) move the authority, so they go to the parser too.
    let split = url
        .split_once("://")
        .filter(|(_, rest)| !rest.starts_with(['/', '\\']));
    let Some((scheme, rest)) = split else {
        let Ok(mut parsed) = reqwest::Url::parse(url) else {
            // The parser refused (a bad port, a bad host, stray slashes with
            // garbage after them). Strip by hand what the parser would have:
            // keep less, never more, of a string that held a `://`.
            return match url.split_once("://") {
                Some((scheme, rest)) => {
                    let rest = rest.trim_start_matches(['/', '\\']);
                    strip_authority(scheme, rest)
                }
                None => url.to_owned(),
            };
        };
        let _ = parsed.set_username("");
        let _ = parsed.set_password(None);
        parsed.set_query(None);
        parsed.set_fragment(None);
        return parsed.into();
    };
    strip_authority(scheme, rest)
}

/// `scheme://host` + path with the userinfo, query and fragment dropped, by
/// text alone: the fallback for what the URL parser cannot read.
fn strip_authority(scheme: &str, rest: &str) -> String {
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let path = tail.split(['?', '#']).next().unwrap_or("");
    format!("{scheme}://{host}{path}")
}

#[cfg(test)]
mod tests {
    use super::public_url;

    #[test]
    fn a_url_loses_its_credentials_query_and_fragment() {
        assert_eq!(
            public_url("https://user:s3cret@host.example:8443/v1/x?key=abc#frag"),
            "https://host.example:8443/v1/x"
        );
        assert_eq!(public_url("https://tok@host"), "https://host");
        assert_eq!(
            public_url("http://127.0.0.1:11434"),
            "http://127.0.0.1:11434"
        );
        assert_eq!(public_url("http://h/?a=b"), "http://h/");
        assert_eq!(public_url("not a url"), "not a url");
    }

    /// What `reqwest::Url` (the oracle) sees as a credential in `url`, and
    /// whether `public_url` left any of it in.
    fn leaks(url: &str) -> Vec<String> {
        let out = public_url(url);
        let Ok(u) = reqwest::Url::parse(url) else {
            return Vec::new();
        };
        let mut secrets = vec![
            u.username(),
            u.password().unwrap_or(""),
            u.query().unwrap_or(""),
            u.fragment().unwrap_or(""),
        ];
        secrets.retain(|s| !s.is_empty());
        secrets
            .into_iter()
            .filter(|s| out.contains(s))
            .map(|s| format!("{s:?} left in {out:?}"))
            .collect()
    }

    #[test]
    fn every_spelling_the_oracle_reads_as_a_credential_is_stripped() {
        for url in [
            "https://SECRETU:SECRETP@h/v1?k=SECRETQ#SECRETF",
            "https:SECRETU:SECRETP@h/v1",
            "https:/SECRETU:SECRETP@h/v1",
            "https:\\\\SECRETU:SECRETP@h\\v1",
            "https://SECRETU:SECRETP@[::1]:8080/v1",
            "https://SECRETU@[2001:db8::1]/v1?SECRETQ",
            "https://h?SECRETQ",
            "https://h#SECRETF",
            "https://h\\x?SECRETQ",
            "https://a@SECRETU:SECRETP@h/v1",
            "https://SECRETU:SECRETP@h:80",
            "HTTPS://SECRETU:SECRETP@h/",
        ] {
            assert_eq!(leaks(url), Vec::<String>::new(), "{url}");
        }
    }

    #[test]
    fn a_string_the_oracle_refuses_still_loses_its_userinfo() {
        for url in [
            "https://SECRETU:SECRETP@bad host/v1",
            "https://SECRETU:SECRETP@h:99999/v1",
            "https://SECRETU:SECRETP@[::1/v1",
            "https://SECRETU:SECRETP@h<>/v1?SECRETQ",
            // Leading slashes move the authority AND the parser refuses the
            // remainder: the fallback still strips by hand.
            "https://\\SECRETU:SECRETP@h:99999/v1",
            "https:///SECRETU:SECRETP@h:99999/v1?SECRETQ",
        ] {
            assert!(reqwest::Url::parse(url).is_err(), "{url} parses");
            let out = public_url(url);
            assert!(!out.contains("SECRET"), "{url} -> {out}");
        }
    }

    use proptest::prelude::*;

    proptest! {
    #![proptest_config(ProptestConfig {
                failure_persistence: Some(Box::new(proptest::test_runner::FileFailurePersistence::Direct(
                    "proptest-regressions/jud-backend.txt",
                ))),
                ..ProptestConfig::default()
            })]
            /// Any mix of the separators a URL has, around marked secrets: what
            /// the oracle reads as userinfo, query or fragment is gone from the
            /// output; and a `://` string it cannot parse loses its `userinfo@`.
                    #[test]
            fn public_url_never_leaks_what_the_oracle_calls_a_credential(
                scheme in prop::sample::select(vec!["https://", "https:", "https:/", "http:\\\\", "HTTP://", "https:///"]),
                user in prop::sample::select(vec!["", "SECRETU@", "SECRETU:SECRETP@", "a@SECRETU:SECRETP@"]),
                host in prop::sample::select(vec!["h", "h:8080", "[::1]:1", "[2001:db8::1]", "h:99999", "bad host", "h<>"]),
                path in prop::sample::select(vec!["", "/", "/v1", "\\v1", "/a:b@c"]),
                query in prop::sample::select(vec!["", "?SECRETQ", "?k=SECRETQ#SECRETF", "#SECRETF", "?#"]),
            ) {
                let url = format!("{scheme}{user}{host}{path}{query}");
                let out = public_url(&url);
                if reqwest::Url::parse(&url).is_ok() {
                    prop_assert_eq!(leaks(&url), Vec::<String>::new());
                } else if scheme == "https://" || scheme == "HTTP://" {
                    // ponytail: an unparseable string the split cannot place (no
                    // `://`, extra slashes) is returned as it is; callers pass the
                    // base URL the client already parsed, so it never arrives.
                    prop_assert!(!out.contains("SECRETU"), "{} -> {}", url, out);
                }
            }
        }
}
