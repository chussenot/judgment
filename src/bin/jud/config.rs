//! Which System One backend a run talks to. The crate's own conventions
//! come first (`TYPESAFE_API_KEY`, and `TYPESAFE_BASE_URL` as the examples
//! read it), then `~/.config/jud/config.yaml`, then the crate's defaults:
//! the hosted TypeSafe API and `jev-latest`. Any server that speaks the
//! wire is selected by its base URL; there is no other switch, because
//! `SystemOne` is the abstraction and the client already serves every
//! compatible server.
//!
//! Because an unset base URL means the hosted API, a misspelt variable
//! (`JUD_BASE_URL` for `TYPESAFE_BASE_URL`) sends paid calls there without a
//! word. Every `TYPESAFE_*` or `JUD_*` variable jud does not read is
//! therefore named before a server is asked, and in `jud config`.

use std::path::PathBuf;
use std::time::Duration;

use judgment::client::{API_KEY_ENV, DEFAULT_BASE_URL, DEFAULT_MODEL};
use judgment::{Client, Error};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::Fallible;

/// The environment variable the examples read for another server.
pub(crate) const BASE_URL_ENV: &str = "TYPESAFE_BASE_URL";
/// The environment variable the command line reads for `--replay`.
const REPLAY_ENV: &str = "JUD_REPLAY";
/// Every variable under the prefixes below that jud reads; any other one set
/// is a misspelling or meant for another tool, and is named.
const READ_ENV: [&str; 3] = [API_KEY_ENV, BASE_URL_ENV, REPLAY_ENV];
/// The prefixes jud's own variables and the crate's conventions use.
const OWN_PREFIXES: [&str; 2] = ["TYPESAFE_", "JUD_"];
/// The per-attempt timeout of a command-line run; a person is waiting.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// `~/.config/jud/config.yaml`, every field optional.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    base_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    timeout_secs: Option<u64>,
    long_context: Option<bool>,
    shared_images: Option<SharedImages>,
}

/// `shared_images` in the file and in `jud config`: `false` for the
/// `assets` map, `true` for Ollama's `images` array of bare base64, or
/// `data-urls` for the same array of `data:` URLs, Liquid's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged, expecting = "true, false or data-urls")]
enum SharedImages {
    Flag(bool),
    Spelling(Spelling),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Spelling {
    DataUrls,
}

/// Where a value came from, for `jud config`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Source {
    /// A flag of the run (`--long-context`); never in `jud config`, which
    /// takes none.
    CommandLine,
    Environment,
    ConfigFile,
    Default,
}

impl Source {
    /// The name `jud config` prints: `command_line`, `environment`,
    /// `config_file` or `default`.
    fn name(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default()
    }
}

fn serialize_public_url<S: serde::Serializer>(url: &str, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&crate::backend::public_url(url))
}

/// The backend a run uses, with where each value came from.
#[derive(Debug, Serialize)]
pub(crate) struct Resolved {
    /// `None` when neither `XDG_CONFIG_HOME` nor `HOME` is an absolute path.
    config_file: Option<String>,
    config_file_present: bool,
    /// Shown without userinfo, query or fragment, which can carry a
    /// credential; the client is given the whole URL.
    #[serde(serialize_with = "serialize_public_url")]
    base_url: String,
    base_url_from: Source,
    model: String,
    model_from: Source,
    timeout_secs: u64,
    /// Whether every call of the run asks for long context (both context
    /// limits raised to 262k tokens through the header the crate's
    /// `ClientBuilder::long_context` sends); off unless the file or the
    /// run's `--long-context` says so.
    long_context: bool,
    long_context_from: Source,
    /// Whether the images of a request go as a shared `images` array
    /// instead of the `assets` map (the crate's
    /// `ClientBuilder::shared_images`, or `shared_images_as_data_urls` for
    /// `data-urls`); off unless the file says so.
    shared_images: SharedImages,
    /// `environment`, `config_file` or `missing`; never the key.
    api_key: &'static str,
    #[serde(skip)]
    api_key_value: Option<String>,
    /// `TYPESAFE_*` and `JUD_*` variables that are set and that jud does
    /// not read; left out of `jud config` when there are none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    ignored_environment: Vec<String>,
}

/// `$XDG_CONFIG_HOME/jud/config.yaml`, else `$HOME/.config/jud/config.yaml`,
/// each only when it is an absolute path (as the XDG specification says of
/// the first); else no file. A relative one would be read from the working
/// directory, where a checkout could name the server the key is sent to.
pub(crate) fn path() -> Option<PathBuf> {
    let absolute = |name| {
        std::env::var_os(name)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
    };
    absolute("XDG_CONFIG_HOME")
        .or_else(|| absolute("HOME").map(|home| home.join(".config")))
        .map(|base| base.join("jud").join("config.yaml"))
}

/// YAML with no source snippet in an error, as the `.jud` reader reads it:
/// the line next to a typo in a configuration file can be its key.
pub(crate) fn from_yaml<T: DeserializeOwned>(text: &str) -> Result<T, serde_saphyr::Error> {
    let mut options = serde_saphyr::Options::default();
    options.with_snippet = false;
    serde_saphyr::from_str_with_options(text, options)
}

fn read_file(path: &PathBuf) -> Fallible<Option<File>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("cannot read {}: {e}", path.display()).into()),
    };
    if text.trim().is_empty() {
        return Ok(Some(File::default()));
    }
    let file: File = from_yaml(&text)
        .map_err(|e| format!("{} is not a jud configuration: {e}", path.display()))?;
    // A zero timeout would fail every request at once, far from its cause.
    if file.timeout_secs == Some(0) {
        return Err(format!(
            "{} is not a jud configuration: timeout_secs must be at least 1",
            path.display()
        )
        .into());
    }
    Ok(Some(file))
}

/// The names of the `TYPESAFE_*` and `JUD_*` variables set in this process
/// that jud does not read, sorted. Values are never looked at: one may be a
/// key.
fn ignored_environment() -> Vec<String> {
    let mut names: Vec<String> = std::env::vars_os()
        .filter_map(|(name, _)| name.into_string().ok())
        .filter(|name| OWN_PREFIXES.iter().any(|p| name.starts_with(p)))
        .filter(|name| !READ_ENV.contains(&name.as_str()))
        .collect();
    names.sort();
    names
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// Resolve the backend: environment, then the file, then the defaults.
pub(crate) fn resolve() -> Fallible<Resolved> {
    resolve_with(None)
}

/// [`resolve`], with the run's own `--long-context`: `Some(true)` when the
/// flag was given, which wins over the file; `None` leaves it to the file
/// and the default.
pub(crate) fn resolve_with(long_context: Option<bool>) -> Fallible<Resolved> {
    let config_file = path();
    let file = match &config_file {
        Some(path) => read_file(path)?,
        None => None,
    };
    let present = file.is_some();
    let file = file.unwrap_or_default();
    let (base_url, base_url_from) = match (env_var(BASE_URL_ENV), file.base_url) {
        (Some(url), _) => (url, Source::Environment),
        (None, Some(url)) => (url, Source::ConfigFile),
        (None, None) => (DEFAULT_BASE_URL.to_owned(), Source::Default),
    };
    let (model, model_from) = match file.model {
        Some(model) => (model, Source::ConfigFile),
        None => (DEFAULT_MODEL.to_owned(), Source::Default),
    };
    // A blank key in the file is unset, as a blank variable is.
    let file_key = file.api_key.filter(|key| !key.trim().is_empty());
    let (api_key, api_key_value) = match (env_var(API_KEY_ENV), file_key) {
        (Some(key), _) => ("environment", Some(key)),
        (None, Some(key)) => ("config_file", Some(key)),
        (None, None) => ("missing", None),
    };
    let (long_context, long_context_from) = match (long_context, file.long_context) {
        (Some(on), _) => (on, Source::CommandLine),
        (None, Some(on)) => (on, Source::ConfigFile),
        (None, None) => (false, Source::Default),
    };
    Ok(Resolved {
        config_file: config_file.map(|path| path.display().to_string()),
        config_file_present: present,
        base_url,
        base_url_from,
        model,
        model_from,
        timeout_secs: file.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS),
        long_context,
        long_context_from,
        shared_images: file.shared_images.unwrap_or(SharedImages::Flag(false)),
        api_key,
        api_key_value,
        ignored_environment: ignored_environment(),
    })
}

impl Resolved {
    pub(crate) fn model(&self) -> &str {
        &self.model
    }

    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Where the model came from: `environment`, `config_file` or `default`.
    pub(crate) fn model_from(&self) -> String {
        self.model_from.name()
    }

    /// Where the base URL came from: `environment`, `config_file` or
    /// `default`.
    pub(crate) fn base_url_from(&self) -> String {
        self.base_url_from.name()
    }

    /// Whether the run asks for long context on every call.
    pub(crate) fn long_context(&self) -> bool {
        self.long_context
    }

    /// Where the long-context setting came from: `command_line`,
    /// `config_file` or `default`.
    pub(crate) fn long_context_from(&self) -> String {
        self.long_context_from.name()
    }

    /// The long-context setting as a run's messages say it: `on (from
    /// SOURCE)` or `off`, where a source's underscore is a space.
    pub(crate) fn describe_long_context(&self) -> String {
        if self.long_context {
            format!("on (from {})", self.long_context_from().replace('_', " "))
        } else {
            "off".to_owned()
        }
    }

    /// One line on stderr per variable `ignored_environment` found, saying
    /// which server is asked instead. Nothing when there are none.
    pub(crate) fn warn_ignored_environment(&self) {
        for name in &self.ignored_environment {
            crate::out::note!(
                "jud: {name} is set but jud does not read it (it reads {BASE_URL_ENV}, \
                 {API_KEY_ENV} and {REPLAY_ENV}, and the model from `model` in {}); \
                 asking {} at {}, the base URL from {}",
                self.config_file_name(),
                self.model,
                crate::backend::public_url(&self.base_url),
                self.base_url_from().replace('_', " ")
            );
        }
    }

    /// The configuration file as a message names it.
    fn config_file_name(&self) -> &str {
        self.config_file
            .as_deref()
            .unwrap_or("a configuration file (none read: neither XDG_CONFIG_HOME nor HOME is an absolute path)")
    }

    /// Where the key came from: `environment`, `config_file` or `missing`;
    /// never the key.
    pub(crate) fn api_key_source(&self) -> &'static str {
        self.api_key
    }

    /// The crate's client for this backend. A missing key is the one
    /// configuration error a run cannot recover from, so it says where a key
    /// goes.
    pub(crate) fn client(&self) -> Fallible<Client> {
        let mut builder = Client::builder()
            .base_url(self.base_url.clone())
            .model(self.model.clone())
            .timeout(Duration::from_secs(self.timeout_secs))
            .long_context(self.long_context);
        builder = match self.shared_images {
            SharedImages::Flag(on) => builder.shared_images(on),
            SharedImages::Spelling(Spelling::DataUrls) => builder.shared_images_as_data_urls(true),
        };
        if let Some(key) = &self.api_key_value {
            builder = builder.api_key(key.clone());
        }
        builder.build().map_err(|e| match e {
            Error::MissingApiKey => format!(
                "no API key: set {API_KEY_ENV}, or `api_key` in {}",
                self.config_file_name()
            )
            .into(),
            other => format!("backend configuration: {other}").into(),
        })
    }
}

/// `jud config`: the resolved backend as JSON, the key never shown.
pub(crate) fn show() -> Fallible<bool> {
    let resolved = resolve()?;
    crate::out::say!("{}", serde_json::to_string_pretty(&resolved)?);
    Ok(true)
}
