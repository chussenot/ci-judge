//! `ci-judge check` and `ci-judge eval`: hold a rubric to its labels
//! without a model call, from the recordings a shadow run kept.

use std::path::Path;

use indexmap::IndexMap;
use judgment::eval::{ECE_BINS, QuestionMetrics};
use judgment::jud::{Cases, Document, Rubric, grade, parse};
use judgment::{Replay, SystemOne};
use serde::Serialize;

use crate::error::{Exit, Failure, Result};
use crate::manifest::Loaded;

#[derive(Debug, Serialize)]
pub struct Report {
    pub rubric: String,
    pub cases: usize,
    pub graded: usize,
    pub missing_recordings: Vec<String>,
    pub questions: IndexMap<String, QuestionMetrics>,
    pub proposed_thresholds: IndexMap<String, f64>,
    pub failures: Vec<String>,
}

fn load_docs(loaded: &Loaded) -> Result<(Rubric, Option<Cases>)> {
    let rubric = match parse(&loaded.rubric_text()?)? {
        Document::Rubric(r) => r,
        _ => {
            return Err(Failure::usage(
                "the manifest's rubric is not a Rubric document",
            ));
        }
    };
    let cases = match loaded.cases_text()? {
        Some(text) => match parse(&text)? {
            Document::Cases(c) => Some(c),
            _ => {
                return Err(Failure::usage(
                    "the manifest's cases file is not a Cases document",
                ));
            }
        },
        None => None,
    };
    Ok((rubric, cases))
}

/// `check`: every document reads, the cases bind to the rubric, every
/// manifest question exists and is asked where the manifest says.
pub fn check(loaded: &Loaded) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    let (rubric, cases) = load_docs(loaded)?;
    lines.push(format!(
        "rubric {} v{}: {} question(s), {} gate(s), fingerprint {}",
        rubric.name,
        rubric.version.as_deref().unwrap_or("?"),
        rubric.questions.len(),
        rubric.policy.gates.len(),
        rubric.fingerprint()
    ));
    for (id, q) in &loaded.manifest.questions {
        let Some(rq) = rubric.questions.get(id) else {
            return Err(Failure::usage(format!(
                "manifest question `{id}` is not in the rubric"
            )));
        };
        let shape = loaded.manifest.request(&q.request).ok_or_else(|| {
            Failure::usage(format!(
                "question `{id}` names request `{}`, not declared",
                q.request
            ))
        })?;
        if !shape.is_global() && rq.when.as_deref() != Some(shape.item_key()) {
            return Err(Failure::usage(format!(
                "question `{id}` is per item of `{}` but the rubric lacks `when: {}`",
                shape.name,
                shape.item_key()
            )));
        }
        if shape.is_global()
            && let Some(when) = rq.when.as_deref()
            && !loaded.manifest.state.iter().any(|s| s.key == when)
            && shape.name != when
        {
            return Err(Failure::usage(format!(
                "question `{id}` has `when: {when}` but neither a state source nor the global request `{}` provides that key",
                shape.name
            )));
        }
        if matches!(
            q.severity,
            crate::manifest::Severity::Blocker | crate::manifest::Severity::Critical
        ) && shape.is_global()
        {
            lines.push(format!(
                "warning: question `{id}` is {} but global, so its finding has no path or line",
                q.severity.as_str()
            ));
        }
    }
    for (id, rq) in &rubric.questions {
        if !loaded.manifest.questions.contains_key(id) {
            lines.push(format!("note: rubric question `{id}` is asked but the manifest does not read it (when: {:?})", rq.when));
        }
    }
    if let Some(cases) = &cases {
        cases.bind(&rubric)?;
        let labelled = cases.cases.iter().filter(|c| !c.expect.is_empty()).count();
        lines.push(format!(
            "cases {}: {} case(s), {} labelled",
            cases.name,
            cases.cases.len(),
            labelled
        ));
    } else {
        lines.push("no cases document yet".to_owned());
    }
    Ok(lines)
}

/// `eval`: grade every labelled case from its recording.
pub async fn eval(loaded: &Loaded, replay: &Path) -> Result<Report> {
    let (rubric, cases) = load_docs(loaded)?;
    let cases = cases.ok_or_else(|| {
        Failure::usage("eval needs a cases document; shadow mode writes one with --cases-out")
    })?;
    cases.bind(&rubric)?;
    let backend = Replay::open(replay)
        .map_err(|e| Failure::usage(format!("replay {}: {e}", replay.display())))?;
    let mut judgments: IndexMap<String, Vec<judgment::eval::Judgment>> = IndexMap::new();
    let mut missing = Vec::new();
    let mut graded = 0;
    for (index, case) in cases.cases.iter().enumerate() {
        if case.expect.is_empty() {
            continue;
        }
        let asked = case.request(&rubric)?;
        let response = match backend.answer(&case.state, "replay", &asked).await {
            Ok(r) => r,
            Err(judgment::Error::NoRecording(_)) => {
                missing.push(case.name(index));
                continue;
            }
            Err(e) => {
                return Err(Failure::failed(format!(
                    "replaying {}: {e}",
                    case.name(index)
                )));
            }
        };
        graded += 1;
        for (id, judgment) in grade(&rubric, case, index, &response)? {
            judgments.entry(id).or_default().push(judgment);
        }
    }
    let mut questions = IndexMap::new();
    let mut proposed = IndexMap::new();
    for (id, js) in &judgments {
        questions.insert(id.clone(), QuestionMetrics::summarise(js.iter(), ECE_BINS));
        let rows = judgment::eval::tuning::threshold_sweep(
            js.iter(),
            &judgment::eval::tuning::default_thresholds(),
        );
        if let Some(t) = judgment::eval::tuning::best_threshold(&rows) {
            proposed.insert(id.clone(), t);
        }
    }
    let mut failures = Vec::new();
    let promotion = &loaded.manifest.promotion;
    for (id, m) in &questions {
        let bar = promotion
            .accuracy_at_least
            .get(id)
            .or_else(|| promotion.accuracy_at_least.get("*"));
        if let (Some(bar), Some(acc)) = (bar, m.accuracy)
            && acc < *bar
        {
            failures.push(format!("{id}: accuracy {acc:.3} below {bar:.3}"));
        }
        let ece_bar = promotion
            .ece_at_most
            .get(id)
            .or_else(|| promotion.ece_at_most.get("*"));
        if let (Some(bar), Some(ece)) = (ece_bar, m.ece)
            && ece > *bar
        {
            failures.push(format!("{id}: calibration error {ece:.3} above {bar:.3}"));
        }
        if let Some(min) = promotion.labelled_at_least
            && m.labelled < min
        {
            failures.push(format!("{id}: {} labelled, fewer than {min}", m.labelled));
        }
    }
    Ok(Report {
        rubric: rubric.name,
        cases: cases.cases.len(),
        graded,
        missing_recordings: missing,
        questions,
        proposed_thresholds: proposed,
        failures,
    })
}

impl Report {
    /// The exit the report asks for: 1 when a case has no recording, 3
    /// when a bar is not met, else 0.
    pub fn exit(&self) -> Exit {
        if !self.missing_recordings.is_empty() {
            Exit::Failed
        } else if !self.failures.is_empty() {
            Exit::Warn
        } else {
            Exit::Ok
        }
    }
}
