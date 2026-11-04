//! From policy verdicts to the verdict file the gate reads.
//!
//! The file's first four fields are the `judge-gate` block's contract:
//! `verdict`, `summary`, `route`, `findings`. Everything ci-judge knows
//! beyond that sits under `ci_judge`, which the block keeps as an
//! artifact and never reads.

use indexmap::IndexMap;
use judgment::jud::{Rubric, Verdict as Policy};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::assess::Answered;
use crate::error::{Failure, Result};
use crate::manifest::{FailsWhen, Manifest, QuestionPolicy, Severity};
use crate::requests::{Truncation, render};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub severity: Severity,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
}

/// How a question's answer was read, for the audit trail and the eval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnswerRecord {
    pub request: String,
    pub question: String,
    /// The policy's verdict, as `Rubric::apply` wrote it.
    pub policy: Value,
    /// `fail`, `pass`, `undecided`, `report`.
    pub outcome: String,
    pub severity: Severity,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Extras {
    pub version: String,
    pub rubric: RubricIdent,
    pub model: Option<String>,
    pub tuning_model: Option<String>,
    pub requests: usize,
    pub evidence_hash: String,
    pub truncation: Truncation,
    pub elapsed_ms: u64,
    pub input_tokens: u64,
    pub cost: Option<f64>,
    pub answers: Vec<AnswerRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RubricIdent {
    pub name: String,
    pub version: Option<String>,
    pub fingerprint: String,
    pub policy_fingerprint: String,
}

/// The verdict file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerdictFile {
    pub verdict: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    pub findings: Vec<Finding>,
    pub ci_judge: Extras,
}

/// How one answer reads against its question's policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Fails,
    Passes,
    Undecided,
    Report,
}

fn classify(q: &QuestionPolicy, policy: &Policy, threshold: f64) -> Outcome {
    if q.report_only {
        return Outcome::Report;
    }
    let Some(fails_when) = &q.fails_when else {
        return Outcome::Report;
    };
    match (policy, fails_when) {
        (
            Policy::Yes { probability } | Policy::No { probability },
            FailsWhen::Yes | FailsWhen::No,
        ) => {
            let p = probability.value();
            if let Some(margin) = q.undecided_margin
                && (p - threshold).abs() < margin
            {
                return Outcome::Undecided;
            }
            let is_yes = matches!(policy, Policy::Yes { .. });
            let fails = match fails_when {
                FailsWhen::Yes => is_yes,
                _ => !is_yes,
            };
            if fails {
                Outcome::Fails
            } else {
                Outcome::Passes
            }
        }
        (
            Policy::Level {
                reached: Some(reached),
                ..
            },
            FailsWhen::Reached | FailsWhen::NotReached,
        ) => {
            let fails = match fails_when {
                FailsWhen::Reached => *reached,
                _ => !*reached,
            };
            if fails {
                Outcome::Fails
            } else {
                Outcome::Passes
            }
        }
        (Policy::Level { reached: None, .. }, FailsWhen::Reached | FailsWhen::NotReached) => {
            Outcome::Undecided
        }
        (Policy::Option { key, .. }, FailsWhen::Option(keys)) => {
            if keys.iter().any(|k| k == key) {
                Outcome::Fails
            } else {
                Outcome::Passes
            }
        }
        (Policy::Deferred(_), _) => Outcome::Undecided,
        _ => Outcome::Undecided,
    }
}

/// The run's inputs to the verdict, beyond the answers.
pub struct Context<'a> {
    pub manifest: &'a Manifest,
    pub rubric: &'a Rubric,
    pub truncation: Truncation,
    pub evidence_hash: String,
    pub elapsed_ms: u64,
    pub notes: Vec<String>,
}

pub fn build(ctx: Context<'_>, answered: &[Answered]) -> Result<VerdictFile> {
    let Context {
        manifest,
        rubric,
        truncation,
        evidence_hash,
        elapsed_ms,
        notes,
    } = ctx;
    let mut findings = Vec::new();
    let mut answers = Vec::new();
    let mut fail = false;
    let mut route_choice: Option<(String, bool)> = None;
    let (mut input_tokens, mut cost) = (0u64, None::<f64>);
    let mut model = None;

    for a in answered {
        model.get_or_insert_with(|| a.response.model.clone());
        input_tokens += a.response.usage.input_tokens;
        if let Some(c) = a.response.usage.cost {
            cost = Some(cost.unwrap_or(0.0) + c);
        }
        let verdicts = rubric.apply(&a.asked, &a.response).map_err(|e| {
            Failure::failed(format!("reading the answers of {}: {e}", a.request.id))
        })?;
        for (id, policy) in &verdicts {
            let Some(q) = manifest.questions.get(id) else {
                continue;
            };
            if q.request != a.request.shape {
                continue;
            }
            let threshold = rubric
                .policy
                .gates
                .get(id)
                .and_then(|g| g.threshold)
                .unwrap_or(0.5);
            let outcome = classify(q, policy, threshold);
            if let Some(route) = &manifest.route_from
                && &route.question == id
            {
                route_choice = Some(match policy {
                    Policy::Option { key, .. } => (key.clone(), true),
                    Policy::Deferred(d) => (
                        d.fallback
                            .clone()
                            .or_else(|| route.fallback.clone())
                            .unwrap_or_default(),
                        false,
                    ),
                    _ => (route.fallback.clone().unwrap_or_default(), false),
                });
            }
            let policy_value = serde_json::to_value(policy).unwrap_or(Value::Null);
            answers.push(AnswerRecord {
                request: a.request.id.clone(),
                question: id.clone(),
                policy: policy_value.clone(),
                outcome: match outcome {
                    Outcome::Fails => "fail",
                    Outcome::Passes => "pass",
                    Outcome::Undecided => "undecided",
                    Outcome::Report => "report",
                }
                .to_owned(),
                severity: q.severity,
            });
            let mut extra: IndexMap<&str, String> = IndexMap::new();
            extra.insert("question", id.clone());
            extra.insert("threshold", format!("{threshold:.2}"));
            extra.insert(
                "verdict",
                policy_value
                    .get("verdict")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
            );
            if let Some(p) = policy_value.get("probability").and_then(Value::as_f64) {
                extra.insert("p", format!("{p:.2}"));
            }
            if let Some(c) = policy_value.get("confidence").and_then(Value::as_f64) {
                extra.insert("confidence", format!("{c:.2}"));
            }
            if let Some(l) = policy_value.get("label").and_then(Value::as_str) {
                extra.insert("level", l.to_owned());
            }
            if let Some(k) = policy_value.get("key").and_then(Value::as_str) {
                extra.insert("option", k.to_owned());
            }
            let text = q
                .finding
                .as_deref()
                .map_or_else(|| format!("{id}: {{verdict}}"), str::to_owned);
            let description = render(&text, &a.request.state, &extra);
            match outcome {
                Outcome::Fails => {
                    fail |= manifest.fail_on.contains(&q.severity);
                    findings.push(Finding {
                        severity: q.severity,
                        description: format!(
                            "{description} (p={}, threshold {})",
                            extra.get("p").cloned().unwrap_or_else(|| extra
                                .get("confidence")
                                .cloned()
                                .unwrap_or_default()),
                            extra["threshold"]
                        ),
                        path: a.request.path.clone(),
                        line: a.request.line,
                    });
                }
                Outcome::Undecided => findings.push(Finding {
                    severity: Severity::Info,
                    description: format!(
                        "undecided: {description} (p={}, threshold {})",
                        extra.get("p").cloned().unwrap_or_else(|| extra
                            .get("confidence")
                            .cloned()
                            .unwrap_or_default()),
                        extra["threshold"]
                    ),
                    path: a.request.path.clone(),
                    line: a.request.line,
                }),
                Outcome::Passes | Outcome::Report => {}
            }
        }
    }

    let failed = findings
        .iter()
        .filter(|f| f.severity != Severity::Info)
        .count();
    let undecided = findings
        .iter()
        .filter(|f| f.severity == Severity::Info && f.description.starts_with("undecided"))
        .count();
    let worst = findings
        .iter()
        .max_by_key(|f| f.severity)
        .map(|f| f.description.clone())
        .unwrap_or_default();
    let items: usize = answered
        .iter()
        .filter(|a| a.request.item_key.is_some())
        .count();
    let mut extra: IndexMap<&str, String> = IndexMap::new();
    extra.insert("failed", failed.to_string());
    extra.insert("undecided", undecided.to_string());
    extra.insert("findings", findings.len().to_string());
    extra.insert("requests", answered.len().to_string());
    extra.insert("items", items.to_string());
    extra.insert("worst", worst);
    let default_summary = if fail {
        "{failed} failing check(s) across {requests} request(s). Worst: {worst}"
    } else if failed > 0 {
        "{failed} check(s) did not pass, none blocking. {undecided} undecided."
    } else if undecided > 0 {
        "No failing check. {undecided} undecided answer(s) to read."
    } else {
        "Every check passed across {requests} request(s)."
    };
    let summary = render(
        manifest.summary.as_deref().unwrap_or(default_summary),
        &Value::Null,
        &extra,
    );

    let route = route_choice
        .map(|(r, _)| r)
        .filter(|r| !r.is_empty())
        .map(|r| crate::reduce::safe_key(&r));

    Ok(VerdictFile {
        verdict: if fail { "fail" } else { "pass" }.to_owned(),
        summary,
        route,
        findings,
        ci_judge: Extras {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            rubric: RubricIdent {
                name: rubric.name.clone(),
                version: rubric.version.clone(),
                fingerprint: rubric.fingerprint(),
                policy_fingerprint: rubric.policy_fingerprint(),
            },
            model,
            tuning_model: rubric.policy.tuning.as_ref().and_then(|t| t.model.clone()),
            requests: answered.len(),
            evidence_hash,
            truncation,
            elapsed_ms,
            input_tokens,
            cost,
            answers,
            notes,
        },
    })
}

impl VerdictFile {
    pub fn write(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| Failure::usage(e.to_string()))?;
        std::fs::write(path, text + "\n")?;
        Ok(())
    }

    /// Read and validate a verdict file as the block's jq validator does:
    /// `verdict` is `pass` or `fail`, `route` matches `[A-Za-z0-9_-]+`,
    /// every finding has a string description and a numeric line.
    pub fn read_loose(path: &std::path::Path) -> Result<Value> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| Failure::usage(format!("no verdict file {}: {e}", path.display())))?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|e| Failure::usage(format!("{} is not JSON: {e}", path.display())))?;
        validate_loose(&value).map_err(|why| {
            Failure::usage(format!("no valid verdict in {}: {why}", path.display()))
        })?;
        Ok(value)
    }
}

/// The block's validation, in Rust.
pub fn validate_loose(value: &Value) -> std::result::Result<(), String> {
    match value.get("verdict").and_then(Value::as_str) {
        Some("pass" | "fail") => {}
        _ => return Err("expected \"verdict\": \"pass\"|\"fail\"".to_owned()),
    }
    if let Some(route) = value.get("route")
        && !route.is_null()
    {
        let ok = route.as_str().is_some_and(|r| {
            !r.is_empty()
                && r.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        });
        if !ok {
            return Err("route must match [A-Za-z0-9_-]+".to_owned());
        }
    }
    if let Some(findings) = value.get("findings")
        && !findings.is_null()
    {
        let Some(list) = findings.as_array() else {
            return Err("findings must be an array".to_owned());
        };
        for f in list {
            if !f.is_object() || !f.get("description").is_some_and(Value::is_string) {
                return Err("every finding needs a string description".to_owned());
            }
            if let Some(line) = f.get("line")
                && !line.is_null()
                && !line.is_number()
            {
                return Err("a finding's line must be a number".to_owned());
            }
        }
    }
    Ok(())
}

/// Hash of everything the model saw, for the audit trail and the cache.
pub fn evidence_hash(requests: &[crate::requests::Request]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for r in requests {
        hasher.update(r.id.as_bytes());
        hasher.update(judgment::eval::canonical::to_string(&r.state).as_bytes());
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(71);
    hex.push_str("sha256:");
    for b in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}
