//! What the examples share: where their recordings live, and for the ones
//! that talk to a server, which backend answers. Not an example itself.

#![allow(dead_code, unused_imports)] // each example uses the part it needs

use std::path::{Path, PathBuf};

/// `examples/recordings/<example>/`: the one place every example's
/// recordings live, named after the example that replays them.
pub fn recordings(example: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/recordings")
        .join(example)
}

#[cfg(feature = "http")]
pub use live::{Mode, backend, model};

#[cfg(feature = "http")]
mod live {
    use std::error::Error;
    use std::time::Duration;

    use judgment::{Client, Recorder, Replay, SystemOne};

    /// Where the answers come from.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum Mode {
        /// The committed recordings; the default.
        #[default]
        Replay,
        /// The hosted API, or the server `TYPESAFE_BASE_URL` names.
        Live,
        /// Live, writing every answer over the committed recordings.
        Record,
    }

    impl Mode {
        /// The mode an argument selects, if it is one.
        pub fn parse(arg: &str) -> Option<Self> {
            match arg {
                "--live" => Some(Self::Live),
                "--record" => Some(Self::Record),
                _ => None,
            }
        }

        /// The mode from the command line, which takes nothing else.
        pub fn from_args(usage: &str) -> Result<Self, String> {
            let mut mode = Self::default();
            for arg in std::env::args().skip(1) {
                mode = Self::parse(&arg)
                    .ok_or_else(|| format!("unexpected argument {arg}; {usage}"))?;
            }
            Ok(mode)
        }
    }

    /// The model to ask: `TYPESAFE_MODEL`, or the hosted API's alias.
    pub fn model() -> String {
        std::env::var("TYPESAFE_MODEL").unwrap_or_else(|_| "jev-latest".to_owned())
    }

    /// The backend `mode` names for `example`, and a label for the report.
    /// Recording empties the example's directory first, so a recording of a
    /// question the example no longer asks does not linger beside the new ones.
    pub fn backend(
        example: &str,
        mode: Mode,
    ) -> Result<(Box<dyn SystemOne>, String), Box<dyn Error>> {
        let dir = super::recordings(example);
        if mode == Mode::Replay {
            let replay = Replay::open(&dir).map_err(|e| {
                format!("{e}: no recordings to replay; run with --record and TYPESAFE_API_KEY set")
            })?;
            let label = format!("the {} recordings under {}", replay.len(), dir.display());
            return Ok((Box::new(replay), label));
        }
        let mut builder = Client::builder().timeout(Duration::from_secs(30));
        if let Ok(url) = std::env::var("TYPESAFE_BASE_URL") {
            builder = builder.base_url(url);
        }
        let client = builder.build()?;
        if mode == Mode::Live {
            return Ok((Box::new(client), "the live API".to_owned()));
        }
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        let label = format!("the live API, recorded under {}", dir.display());
        Ok((Box::new(Recorder::new(client, &dir)), label))
    }
}
