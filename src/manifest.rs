//! The manifest beside a `.jud` rubric: everything CI-specific that the
//! `.jud` reader would refuse as an unknown field.
//!
//! A rubric says what to ask and how to read the answers. The manifest
//! says where the evidence comes from, how it is shaped into requests,
//! which question is asked in which request, what a failing answer means
//! (severity, finding text) and when the rubric may be promoted.

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::error::{Failure, Result};

/// Code Quality severities, as the `judge-gate` block accepts them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Minor,
    Major,
    Critical,
    Blocker,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Minor => "minor",
            Self::Major => "major",
            Self::Critical => "critical",
            Self::Blocker => "blocker",
        }
    }
}

/// What makes a question's answer a failing one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailsWhen {
    /// A Noul read as yes.
    Yes,
    /// A Noul read as no.
    No,
    /// A Score whose nearest level reached the gate's `level_at_least`.
    Reached,
    /// A Score whose nearest level did not reach it.
    NotReached,
    /// A Choice whose chosen option is one of these keys.
    Option(Vec<String>),
}

/// How one state key is collected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// A file (or glob) left by an earlier job, read and reduced.
    Artifact,
    /// `git diff` against the merge base, reduced to hunks per file.
    Diff,
    /// The merge or pull request's own fields.
    Mr,
    /// A text file from the checkout.
    File,
    /// The traces of the failed jobs of this pipeline.
    Joblog,
    /// An environment variable's value.
    Env,
}

/// One state key and where it comes from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateSource {
    /// The state key the value lands under.
    pub key: String,
    pub source: SourceKind,
    /// A path or glob (`artifact`, `file`), or a variable name (`env`);
    /// `${VAR}` is expanded from the environment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// How the raw value is reduced: `tfplan`, `junit`, `json`, `text`,
    /// `diffhunks`, `jobs`. Each source has a default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reduce: Option<String>,
    /// Lower is kept longer when the state must shrink to the byte budget.
    #[serde(default = "default_priority")]
    pub priority: u32,
    /// A source that cannot be collected is left out instead of failing the run.
    #[serde(default)]
    pub optional: bool,
    /// Lines kept from the end of a text, where the reducer takes a tail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail_lines: Option<usize>,
}

fn default_priority() -> u32 {
    5
}

/// One request shape: a global request over the whole state, or one
/// request per item of a state key that holds a list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestShape {
    pub name: String,
    /// The state key whose items each become a request; absent for a
    /// global request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<String>,
    /// The key one item lands under in its request; defaults to the
    /// request's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_key: Option<String>,
    /// Keep only items whose field has one of these values.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub filter: IndexMap<String, Vec<String>>,
    /// Cap on items; beyond it the run is marked truncated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
}

impl RequestShape {
    pub fn is_global(&self) -> bool {
        self.items.is_none()
    }

    pub fn item_key(&self) -> &str {
        self.item_key.as_deref().unwrap_or(&self.name)
    }
}

/// What ci-judge adds to one rubric question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestionPolicy {
    /// The request shape the question is asked in.
    pub request: String,
    #[serde(default = "default_severity")]
    pub severity: Severity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fails_when: Option<FailsWhen>,
    /// A Noul within this distance of its threshold is undecided: an
    /// `info` finding, never a fail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undecided_margin: Option<f64>,
    /// The finding's text; `{key.path}` reads the request's state,
    /// `{p}` the probability, `{threshold}` the gate's threshold,
    /// `{verdict}` the policy's reading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finding: Option<String>,
    /// Recorded in the verdict, never a finding.
    #[serde(default)]
    pub report_only: bool,
}

fn default_severity() -> Severity {
    Severity::Major
}

/// Thresholds `ci-judge eval` holds a rubric to before it may leave
/// shadow mode.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Promotion {
    /// Accuracy per question (or `*` for every labelled question).
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub accuracy_at_least: IndexMap<String, f64>,
    /// Expected calibration error, at most, per question or `*`.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub ece_at_most: IndexMap<String, f64>,
    /// Labelled cases per question, at least.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labelled_at_least: Option<usize>,
}

/// The route a verdict selects: a Choice question whose chosen option
/// names the route pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteFrom {
    pub question: String,
    /// The route when the question is deferred or absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The `.jud` Rubric, relative to the manifest.
    pub rubric: String,
    /// The `.jud` Cases, relative to the manifest; grown by shadow mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cases: Option<String>,
    /// Recordings directory, relative to the manifest, for `eval`.
    #[serde(default = "default_recordings")]
    pub recordings: String,
    pub state: Vec<StateSource>,
    pub requests: Vec<RequestShape>,
    pub questions: IndexMap<String, QuestionPolicy>,
    /// Severities that fail the verdict; the rest only warn.
    #[serde(default = "default_fail_on")]
    pub fail_on: Vec<Severity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_from: Option<RouteFrom>,
    /// The summary's template; `{failed}`, `{undecided}`, `{findings}`,
    /// `{requests}`, `{items}`, `{worst}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default)]
    pub promotion: Promotion,
}

fn default_recordings() -> String {
    "recordings".to_owned()
}

fn default_fail_on() -> Vec<Severity> {
    vec![Severity::Blocker, Severity::Critical]
}

/// A manifest with the directory it was read from, so its rubric, cases
/// and recordings resolve.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub manifest: Manifest,
    pub dir: PathBuf,
    /// From the bundled catalog, which is the only place a blocker may live.
    pub bundled: bool,
}

impl Loaded {
    pub fn rubric_path(&self) -> PathBuf {
        self.dir.join(&self.manifest.rubric)
    }

    pub fn cases_path(&self) -> Option<PathBuf> {
        self.manifest.cases.as_ref().map(|c| self.dir.join(c))
    }

    pub fn recordings_dir(&self) -> PathBuf {
        self.dir.join(&self.manifest.recordings)
    }

    /// The rubric's text, from disk or from the catalog.
    pub fn rubric_text(&self) -> Result<String> {
        if self.bundled {
            return crate::rubrics::file(&self.manifest.name, &self.manifest.rubric)
                .map(str::to_owned)
                .ok_or_else(|| {
                    Failure::usage(format!(
                        "bundled rubric {} has no {}",
                        self.manifest.name, self.manifest.rubric
                    ))
                });
        }
        std::fs::read_to_string(self.rubric_path()).map_err(|e| {
            Failure::usage(format!("cannot read {}: {e}", self.rubric_path().display()))
        })
    }

    /// The cases' text, when the manifest names a cases document that exists.
    pub fn cases_text(&self) -> Result<Option<String>> {
        let Some(name) = &self.manifest.cases else {
            return Ok(None);
        };
        if self.bundled {
            return Ok(crate::rubrics::file(&self.manifest.name, name).map(str::to_owned));
        }
        let path = self.dir.join(name);
        match std::fs::read_to_string(&path) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Failure::usage(format!(
                "cannot read {}: {e}",
                path.display()
            ))),
        }
    }
}

impl Manifest {
    pub fn parse(text: &str) -> Result<Self> {
        let mut options = serde_saphyr::Options::default();
        options.with_snippet = false;
        let manifest: Self = serde_saphyr::from_str_with_options(text, options)
            .map_err(|e| Failure::usage(format!("manifest: {e}")))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Load by catalog name (`infra-plan`), by manifest path, or by a
    /// directory holding `manifest.yml`.
    pub fn load(rubric: &str) -> Result<Loaded> {
        if let Some(text) = crate::rubrics::manifest(rubric) {
            return Ok(Loaded {
                manifest: Self::parse(text)?,
                dir: PathBuf::from(format!("<bundled>/{rubric}")),
                bundled: true,
            });
        }
        let path = Path::new(rubric);
        let (file, dir) = if path.is_dir() {
            (path.join("manifest.yml"), path.to_path_buf())
        } else {
            (
                path.to_path_buf(),
                path.parent()
                    .map_or_else(|| PathBuf::from("."), Path::to_path_buf),
            )
        };
        let text = std::fs::read_to_string(&file).map_err(|e| {
            Failure::usage(format!(
                "no rubric `{rubric}`: not in the catalog ({}) and cannot read {}: {e}",
                crate::rubrics::names().join(", "),
                file.display()
            ))
        })?;
        Ok(Loaded {
            manifest: Self::parse(&text)?,
            dir,
            bundled: false,
        })
    }

    fn validate(&self) -> Result<()> {
        if self.state.is_empty() {
            return Err(Failure::usage("manifest: `state` names no source"));
        }
        if self.requests.is_empty() {
            return Err(Failure::usage(
                "manifest: `requests` names no request shape",
            ));
        }
        let mut keys = std::collections::HashSet::new();
        for source in &self.state {
            if !keys.insert(source.key.as_str()) {
                return Err(Failure::usage(format!(
                    "manifest: state key `{}` is declared twice",
                    source.key
                )));
            }
            if matches!(
                source.source,
                SourceKind::Artifact | SourceKind::File | SourceKind::Env
            ) && source.path.is_none()
            {
                return Err(Failure::usage(format!(
                    "manifest: state `{}` needs a `path`",
                    source.key
                )));
            }
        }
        for shape in &self.requests {
            if let Some(items) = &shape.items
                && !keys.contains(items.as_str())
            {
                return Err(Failure::usage(format!(
                    "manifest: request `{}` takes items from `{items}`, which no state source provides",
                    shape.name
                )));
            }
            if shape.item_key().is_empty() {
                return Err(Failure::usage(format!(
                    "manifest: request `{}` has an empty item key",
                    shape.name
                )));
            }
        }
        for (id, q) in &self.questions {
            if !self.requests.iter().any(|r| r.name == q.request) {
                return Err(Failure::usage(format!(
                    "manifest: question `{id}` is asked in request `{}`, which is not declared",
                    q.request
                )));
            }
            if !q.report_only && q.fails_when.is_none() {
                return Err(Failure::usage(format!(
                    "manifest: question `{id}` needs `fails_when` or `report_only: true`"
                )));
            }
            if let Some(margin) = q.undecided_margin
                && !(0.0..0.5).contains(&margin)
            {
                return Err(Failure::usage(format!(
                    "manifest: question `{id}`: undecided_margin must be in [0, 0.5)"
                )));
            }
        }
        if let Some(route) = &self.route_from
            && !self.questions.contains_key(&route.question)
        {
            return Err(Failure::usage(format!(
                "manifest: route_from names `{}`, which is not among the questions",
                route.question
            )));
        }
        Ok(())
    }

    /// The request shape by name.
    pub fn request(&self, name: &str) -> Option<&RequestShape> {
        self.requests.iter().find(|r| r.name == name)
    }

    /// The highest severity a repo-local manifest may carry.
    pub fn highest_severity(&self) -> Severity {
        self.questions
            .values()
            .filter(|q| !q.report_only)
            .map(|q| q.severity)
            .max()
            .unwrap_or(Severity::Info)
    }
}
