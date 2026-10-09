//! The backend a run talks to, and the ways a run fails. Shared by every
//! subcommand that asks a model or replays recordings: `jud RUBRIC`, and
//! `jud record`, `eval` and `tune` (decision 0021).

use std::future::Future;
use std::path::Path;
use std::process::ExitCode;

use judgment::{Error, Replay, SystemOne};
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
    pub(crate) fn open(replay: Option<&Path>) -> Result<Self, Failure> {
        let kind = if let Some(dir) = replay {
            let replay = Replay::open(dir).map_err(|e| {
                Failure::Usage(format!("cannot replay from {}: {e}", dir.display()))
            })?;
            Kind::Recordings {
                replay,
                dir: dir.display().to_string(),
            }
        } else {
            let resolved = config::resolve().map_err(|e| Failure::Usage(e.to_string()))?;
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

    pub(crate) fn answer(
        &self,
        state: &Value,
        questions: &judgment::Questions,
    ) -> Result<judgment::Response, Failure> {
        match &self.kind {
            Kind::Server { client, resolved } => self
                .runtime
                .block_on(client.answer(state, self.model(), questions))
                .map_err(|e| {
                    Failure::Backend(format!(
                        "the backend at {} failed: {e}",
                        public_url(resolved.base_url())
                    ))
                }),
            Kind::Recordings { replay, dir } => self
                .runtime
                .block_on(replay.answer(state, self.model(), questions))
                .map_err(|e| match e {
                    // The wording `jud RUBRIC --replay` has always used for a
                    // state nobody recorded.
                    Error::NoRecording(_) => Failure::Missing(format!(
                        "no recording under {dir} answers this state and rubric: {e}"
                    )),
                    // A recording is there and cannot answer: the request
                    // matched and the response no longer fits the questions,
                    // which is a different thing to fix than a missing one.
                    other => Failure::Backend(format!(
                        "a recording under {dir} matches this state and rubric but cannot answer it: {other}"
                    )),
                }),
        }
    }

    /// Before a run that asks a server once per case (`jud record`, `jud
    /// eval`): one line naming the model and the server, and where the
    /// base URL came from, so a run aimed at the wrong server, the hosted
    /// API by default, can be stopped before it has paid for every case.
    /// Nothing under a replay, which costs nothing.
    pub(crate) fn announce(&self, cases: usize) {
        if let Kind::Server { resolved, .. } = &self.kind {
            crate::out::note!(
                "asking {} at {} for up to {cases} case{} (base URL from {})",
                resolved.model(),
                public_url(resolved.base_url()),
                if cases == 1 { "" } else { "s" },
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
/// as it is.
pub(crate) fn public_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_owned();
    };
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
}
