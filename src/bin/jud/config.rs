//! Which System One backend a run talks to. The crate's own conventions
//! come first (`TYPESAFE_API_KEY`, and `TYPESAFE_BASE_URL` as the examples
//! read it), then `~/.config/jud/config.yaml`, then the crate's defaults:
//! the hosted TypeSafe API and `jev-latest`. Any server that speaks the
//! wire is selected by its base URL; there is no other switch, because
//! `SystemOne` is the abstraction and the client already serves every
//! compatible server.

use std::path::PathBuf;
use std::time::Duration;

use judgment::client::{API_KEY_ENV, DEFAULT_BASE_URL, DEFAULT_MODEL};
use judgment::{Client, Error};
use serde::{Deserialize, Serialize};

use crate::Fallible;

/// The environment variable the examples read for another server.
pub(crate) const BASE_URL_ENV: &str = "TYPESAFE_BASE_URL";
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
}

/// Where a value came from, for `jud config`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Source {
    Environment,
    ConfigFile,
    Default,
}

/// The backend a run uses, with where each value came from.
#[derive(Debug, Serialize)]
pub(crate) struct Resolved {
    config_file: String,
    config_file_present: bool,
    base_url: String,
    base_url_from: Source,
    model: String,
    model_from: Source,
    timeout_secs: u64,
    /// `environment`, `config_file` or `missing`; never the key.
    api_key: &'static str,
    #[serde(skip)]
    api_key_value: Option<String>,
}

/// `$XDG_CONFIG_HOME/jud/config.yaml`, else `$HOME/.config/jud/config.yaml`.
pub(crate) fn path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join("jud").join("config.yaml")
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
    serde_saphyr::from_str(&text)
        .map(Some)
        .map_err(|e| format!("{} is not a jud configuration: {e}", path.display()).into())
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// Resolve the backend: environment, then the file, then the defaults.
pub(crate) fn resolve() -> Fallible<Resolved> {
    let config_file = path();
    let file = read_file(&config_file)?;
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
    let (api_key, api_key_value) = match (env_var(API_KEY_ENV), file.api_key) {
        (Some(key), _) => ("environment", Some(key)),
        (None, Some(key)) => ("config_file", Some(key)),
        (None, None) => ("missing", None),
    };
    Ok(Resolved {
        config_file: config_file.display().to_string(),
        config_file_present: present,
        base_url,
        base_url_from,
        model,
        model_from,
        timeout_secs: file.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS),
        api_key,
        api_key_value,
    })
}

impl Resolved {
    pub(crate) fn model(&self) -> &str {
        &self.model
    }

    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The crate's client for this backend. A missing key is the one
    /// configuration error a run cannot recover from, so it says where a key
    /// goes.
    pub(crate) fn client(&self) -> Fallible<Client> {
        let mut builder = Client::builder()
            .base_url(self.base_url.clone())
            .model(self.model.clone())
            .timeout(Duration::from_secs(self.timeout_secs));
        if let Some(key) = &self.api_key_value {
            builder = builder.api_key(key.clone());
        }
        builder.build().map_err(|e| match e {
            Error::MissingApiKey => format!(
                "no API key: set {API_KEY_ENV}, or `api_key` in {}",
                self.config_file
            )
            .into(),
            other => format!("backend configuration: {other}").into(),
        })
    }
}

/// `jud config`: the resolved backend as JSON, the key never shown.
pub(crate) fn show() -> Fallible<bool> {
    let resolved = resolve()?;
    println!("{}", serde_json::to_string_pretty(&resolved)?);
    Ok(true)
}
