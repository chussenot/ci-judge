//! Where a run's settings come from.
//!
//! Backend settings follow the `jud` command exactly, so a laptop and a
//! runner read the same thing: `TYPESAFE_API_KEY` and `TYPESAFE_BASE_URL`
//! in the environment, then `$XDG_CONFIG_HOME/jud/config.yaml` (else
//! `~/.config/jud/config.yaml`), then the crate's defaults. Everything
//! else is a `CI_JUDGE_*` variable or a flag, flags winning.

use std::path::PathBuf;
use std::time::Duration;

use judgment::client::{API_KEY_ENV, DEFAULT_BASE_URL, DEFAULT_MODEL};
use judgment::{Client, Error as JudgmentError};
use serde::Deserialize;

use crate::error::{Failure, Result};

/// The environment variable the `jud` command and the examples read for
/// another server.
pub const BASE_URL_ENV: &str = "TYPESAFE_BASE_URL";
/// Per-attempt timeout of a run in CI; a job is waiting, not a person.
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// `~/.config/jud/config.yaml`, every field optional; the same fields the
/// `jud` command reads, refused the same way when one is unknown.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(untagged, expecting = "true, false or data-urls")]
enum SharedImages {
    Flag(bool),
    Spelling(Spelling),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Spelling {
    DataUrls,
}

/// Where a value came from, for `ci-judge config` and the log line before
/// a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    CommandLine,
    Environment,
    ConfigFile,
    Default,
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::CommandLine => "command line",
            Self::Environment => "environment",
            Self::ConfigFile => "config file",
            Self::Default => "default",
        })
    }
}

/// The backend a run talks to, resolved.
#[derive(Debug)]
pub struct Backend {
    pub config_file: Option<PathBuf>,
    pub base_url: String,
    pub base_url_from: Source,
    pub model: String,
    pub model_from: Source,
    pub timeout_secs: u64,
    pub long_context: bool,
    shared_images: SharedImages,
    api_key: Option<String>,
    pub api_key_from: &'static str,
}

/// The configuration file's path, as `jud` finds it: only an absolute
/// `XDG_CONFIG_HOME` or `HOME`, never the working directory.
pub fn config_path() -> Option<PathBuf> {
    let absolute = |name: &str| {
        std::env::var_os(name)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
    };
    absolute("XDG_CONFIG_HOME")
        .or_else(|| absolute("HOME").map(|home| home.join(".config")))
        .map(|base| base.join("jud").join("config.yaml"))
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

fn read_file(path: &PathBuf) -> Result<Option<File>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(Failure::usage(format!(
                "cannot read {}: {e}",
                path.display()
            )));
        }
    };
    if text.trim().is_empty() {
        return Ok(Some(File::default()));
    }
    let mut options = serde_saphyr::Options::default();
    options.with_snippet = false;
    let file: File = serde_saphyr::from_str_with_options(&text, options).map_err(|e| {
        Failure::usage(format!(
            "{} is not a jud configuration: {e}",
            path.display()
        ))
    })?;
    if file.timeout_secs == Some(0) {
        return Err(Failure::usage(format!(
            "{} is not a jud configuration: timeout_secs must be at least 1",
            path.display()
        )));
    }
    Ok(Some(file))
}

impl Backend {
    /// Resolve the backend: a `--model` flag, then the environment, then
    /// the file, then the defaults.
    pub fn resolve(model_flag: Option<&str>) -> Result<Self> {
        let config_file = config_path();
        let file = match &config_file {
            Some(path) => read_file(path)?,
            None => None,
        }
        .unwrap_or_default();
        let (base_url, base_url_from) = match (env_var(BASE_URL_ENV), file.base_url) {
            (Some(url), _) => (url, Source::Environment),
            (None, Some(url)) => (url, Source::ConfigFile),
            (None, None) => (DEFAULT_BASE_URL.to_owned(), Source::Default),
        };
        let (model, model_from) = match (model_flag, file.model) {
            (Some(model), _) => (model.to_owned(), Source::CommandLine),
            (None, Some(model)) => (model, Source::ConfigFile),
            (None, None) => (DEFAULT_MODEL.to_owned(), Source::Default),
        };
        let file_key = file.api_key.filter(|key| !key.trim().is_empty());
        let (api_key, api_key_from) = match (env_var(API_KEY_ENV), file_key) {
            (Some(key), _) => (Some(key), "environment"),
            (None, Some(key)) => (Some(key), "config file"),
            (None, None) => (None, "missing"),
        };
        Ok(Self {
            config_file,
            base_url,
            base_url_from,
            model,
            model_from,
            timeout_secs: file.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS),
            long_context: file.long_context.unwrap_or(false),
            shared_images: file.shared_images.unwrap_or(SharedImages::Flag(false)),
            api_key,
            api_key_from,
        })
    }

    /// The base URL without user information, query or fragment: what a
    /// log line may show.
    pub fn public_url(&self) -> String {
        public_url(&self.base_url)
    }

    /// The crate's client for this backend.
    pub fn client(&self) -> Result<Client> {
        let mut builder = Client::builder()
            .base_url(self.base_url.clone())
            .model(self.model.clone())
            .timeout(Duration::from_secs(self.timeout_secs))
            .long_context(self.long_context);
        builder = match self.shared_images {
            SharedImages::Flag(on) => builder.shared_images(on),
            SharedImages::Spelling(Spelling::DataUrls) => builder.shared_images_as_data_urls(true),
        };
        if let Some(key) = &self.api_key {
            builder = builder.api_key(key.clone());
        }
        builder.build().map_err(|e| match e {
            JudgmentError::MissingApiKey => Failure::usage(format!(
                "no API key: set {API_KEY_ENV}, or `api_key` in {}",
                self.config_file
                    .as_ref()
                    .map_or("~/.config/jud/config.yaml".to_owned(), |p| p
                        .display()
                        .to_string())
            )),
            other => Failure::usage(format!("backend configuration: {other}")),
        })
    }
}

/// A URL with its user information, query and fragment removed, so a
/// message never shows a credential carried in one.
pub fn public_url(url: &str) -> String {
    match reqwest::Url::parse(url) {
        Ok(mut parsed) => {
            let _ = parsed.set_username("");
            let _ = parsed.set_password(None);
            parsed.set_query(None);
            parsed.set_fragment(None);
            parsed.to_string().trim_end_matches('/').to_owned()
        }
        Err(_) => url
            .split_once("://")
            .map(|(scheme, rest)| {
                let rest = rest.split(['?', '#']).next().unwrap_or("");
                let rest = rest.rsplit_once('@').map_or(rest, |(_, host)| host);
                format!("{scheme}://{rest}")
            })
            .unwrap_or_else(|| "<url>".to_owned()),
    }
}

/// Expand `${NAME}` and `$NAME` from the environment; an unset variable
/// expands to nothing, as a shell would.
pub fn expand_env(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        let rest = &text[i + 1..];
        let (name, consumed) = if let Some(stripped) = rest.strip_prefix('{') {
            match stripped.find('}') {
                Some(end) => (&stripped[..end], end + 2),
                None => {
                    out.push('$');
                    continue;
                }
            }
        } else {
            let end = rest
                .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .unwrap_or(rest.len());
            (&rest[..end], end)
        };
        if name.is_empty() {
            out.push('$');
            continue;
        }
        out.push_str(&std::env::var(name).unwrap_or_default());
        for _ in 0..consumed {
            chars.next();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_url_strips_credentials_and_query() {
        assert_eq!(
            public_url("https://u:p@host:8080/base?k=v#f"),
            "https://host:8080/base"
        );
        assert_eq!(public_url("http://localhost:8000"), "http://localhost:8000");
    }

    #[test]
    fn expands_braced_and_bare_variables() {
        // SAFETY: tests in this module run single-threaded with respect to
        // this variable name, which nothing else reads.
        unsafe { std::env::set_var("CI_JUDGE_TEST_VAR", "x") };
        assert_eq!(expand_env("a/${CI_JUDGE_TEST_VAR}/b"), "a/x/b");
        assert_eq!(expand_env("a/$CI_JUDGE_TEST_VAR/b"), "a/x/b");
        assert_eq!(expand_env("a/${CI_JUDGE_UNSET_VAR}/b"), "a//b");
        assert_eq!(expand_env("cost $5"), "cost ");
        assert_eq!(expand_env("x$"), "x$");
    }
}
