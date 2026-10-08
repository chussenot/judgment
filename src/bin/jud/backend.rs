//! The backend a run talks to, and the two ways a run fails. Shared by every
//! subcommand that asks a model or replays recordings: `jud RUBRIC`, and
//! `jud record`, `eval` and `tune` (decision 0021).

use std::path::Path;
use std::process::ExitCode;

use judgment::{Replay, SystemOne};
use serde_json::Value;

use crate::{EXIT_BACKEND, EXIT_USAGE, config};

pub(crate) enum Failure {
    /// Fixable before any call: the file, the state, the configuration.
    Usage(String),
    /// The call was made and failed, or its answer did not fit.
    Backend(String),
}

impl Failure {
    /// The message, without the `jud: ` prefix.
    pub(crate) fn message(&self) -> &str {
        match self {
            Self::Usage(m) | Self::Backend(m) => m,
        }
    }

    /// The exit status: 2 for a usage failure, 1 for a backend failure.
    pub(crate) fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) => EXIT_USAGE,
            Self::Backend(_) => EXIT_BACKEND,
        }
    }

    /// Print `jud: MESSAGE` on stderr and give the exit status.
    pub(crate) fn report(&self) -> ExitCode {
        eprintln!("jud: {}", self.message());
        ExitCode::from(self.exit_code())
    }
}

/// Which backend answers: the configured server, or the recordings under
/// a directory. Both are the crate's; the run is the same either side.
#[allow(
    clippy::large_enum_variant,
    reason = "one value per run; the client's size is not worth a box"
)]
pub(crate) enum Backend {
    Server {
        client: judgment::Client,
        resolved: config::Resolved,
    },
    Recordings {
        replay: Replay,
        dir: String,
    },
}

impl Backend {
    pub(crate) fn open(replay: Option<&Path>) -> Result<Self, Failure> {
        if let Some(dir) = replay {
            let replay = Replay::open(dir).map_err(|e| {
                Failure::Usage(format!("cannot replay from {}: {e}", dir.display()))
            })?;
            return Ok(Self::Recordings {
                replay,
                dir: dir.display().to_string(),
            });
        }
        let resolved = config::resolve().map_err(|e| Failure::Usage(e.to_string()))?;
        let client = resolved
            .client()
            .map_err(|e| Failure::Usage(e.to_string()))?;
        Ok(Self::Server { client, resolved })
    }

    pub(crate) fn model(&self) -> &str {
        match self {
            Self::Server { resolved, .. } => resolved.model(),
            Self::Recordings { .. } => judgment::client::DEFAULT_MODEL,
        }
    }

    pub(crate) fn answer(
        &self,
        state: &Value,
        questions: &judgment::Questions,
    ) -> Result<judgment::Response, Failure> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| Failure::Backend(format!("cannot start the runtime: {e}")))?;
        match self {
            Self::Server { client, resolved } => runtime
                .block_on(client.answer(state, self.model(), questions))
                .map_err(|e| {
                    Failure::Backend(format!(
                        "the backend at {} failed: {e}",
                        resolved.base_url()
                    ))
                }),
            Self::Recordings { replay, dir } => runtime
                .block_on(replay.answer(state, self.model(), questions))
                .map_err(|e| {
                    Failure::Backend(format!(
                        "no recording under {dir} answers this state and rubric: {e}"
                    ))
                }),
        }
    }

    /// The base URL that answers, or `None` when recordings do.
    pub(crate) fn server(&self) -> Option<&str> {
        match self {
            Self::Server { resolved, .. } => Some(resolved.base_url()),
            Self::Recordings { .. } => None,
        }
    }

    /// True when the answers come from a directory of recordings.
    pub(crate) fn is_replay(&self) -> bool {
        matches!(self, Self::Recordings { .. })
    }
}
