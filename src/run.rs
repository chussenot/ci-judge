//! `ci-judge run`: collect, prepare, assess, write the verdict file.

use std::path::{Path, PathBuf};
use std::time::Instant;

use judgment::jud::{Document, Rubric, parse};
use serde_json::Value;

use crate::assess::{Assessor, state_bytes};
use crate::config::Backend;
use crate::error::{Exit, Failure, Result};
use crate::manifest::{Loaded, Manifest, Severity};
use crate::platform::Platform;
use crate::requests::{self, Collected, Request, Truncation};
use crate::verdict::{self, VerdictFile};

/// Everything a run takes, from flags and `CI_JUDGE_*` variables.
#[derive(Debug, Clone)]
pub struct Options {
    pub rubric: String,
    pub out: PathBuf,
    pub model: Option<String>,
    pub replay: Option<PathBuf>,
    pub record: Option<PathBuf>,
    pub cases_out: Option<PathBuf>,
    pub max_items: Option<usize>,
    pub budget_bytes: usize,
    pub max_in_flight: usize,
    pub allow_untuned: bool,
    pub allow_local_blockers: bool,
    pub platform: Option<String>,
}

/// The pieces of a run before any call, for `explain` and `collect`.
pub struct Prepared {
    pub loaded: Loaded,
    pub rubric: Rubric,
    pub platform: Platform,
    pub requests: Vec<Request>,
    pub truncation: Truncation,
    pub notes: Vec<String>,
}

pub fn log(message: impl AsRef<str>) {
    eprintln!("ci-judge: {}", message.as_ref());
}

/// Load the manifest and the rubric, apply the severity cap.
pub fn load(options: &Options) -> Result<(Loaded, Rubric)> {
    let loaded = Manifest::load(&options.rubric)?;
    let text = loaded.rubric_text()?;
    let rubric = match parse(&text)? {
        Document::Rubric(rubric) => rubric,
        _ => {
            return Err(Failure::usage(format!(
                "{} is not a Rubric document",
                loaded.rubric_path().display()
            )));
        }
    };
    for (id, q) in &loaded.manifest.questions {
        if !rubric.questions.contains_key(id) {
            return Err(Failure::usage(format!(
                "manifest question `{id}` is not in rubric {} (questions: {})",
                rubric.name,
                rubric
                    .questions
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        if let Some(shape) = loaded.manifest.request(&q.request)
            && !shape.is_global()
        {
            let rq = &rubric.questions[id];
            if rq.when.as_deref() != Some(shape.item_key()) {
                return Err(Failure::usage(format!(
                    "manifest question `{id}` is asked per item of request `{}`, so the rubric must declare `when: {}` on it (found {:?})",
                    shape.name,
                    shape.item_key(),
                    rq.when
                )));
            }
        }
    }
    if !loaded.bundled
        && !options.allow_local_blockers
        && loaded.manifest.highest_severity() >= Severity::Critical
    {
        return Err(Failure::usage(format!(
            "manifest {} declares a critical or blocker severity; only the bundled catalog may (or pass --allow-local-blockers)",
            loaded.dir.display()
        )));
    }
    Ok((loaded, rubric))
}

/// Collect every source and shape the requests, no call.
pub async fn prepare(options: &Options) -> Result<Prepared> {
    let (loaded, rubric) = load(options)?;
    let platform = Platform::detect(options.platform.as_deref())?;
    let mut sources = Vec::new();
    let mut notes = Vec::new();
    for spec in &loaded.manifest.state {
        match crate::collect::collect(spec, &platform).await {
            Ok(mut reduced) => {
                crate::redact::value(&mut reduced.value);
                for item in &mut reduced.items {
                    crate::redact::value(&mut item.value);
                }
                log(format!(
                    "collected `{}` from {:?} ({} bytes, {} items)",
                    spec.key,
                    spec.source,
                    state_bytes(&reduced.value),
                    reduced.items.len()
                ));
                sources.push(Collected {
                    key: spec.key.clone(),
                    priority: spec.priority,
                    reduced,
                });
            }
            Err(e) if spec.optional => {
                log(format!("optional `{}` left out: {e}", spec.key));
                notes.push(format!("{} not collected: {e}", spec.key));
            }
            Err(e) => {
                return Err(Failure {
                    exit: Exit::Failed,
                    message: format!("collecting `{}`: {e}", spec.key),
                });
            }
        }
    }
    let (requests, truncation) = requests::build(
        &loaded.manifest,
        &platform.slug(),
        &sources,
        options.budget_bytes,
        options.max_items,
    );
    if truncation.any() {
        log(format!(
            "truncated: {} item(s) dropped, keys dropped: {:?}",
            truncation.items_dropped.values().sum::<usize>(),
            truncation.keys_dropped
        ));
    }
    Ok(Prepared {
        loaded,
        rubric,
        platform,
        requests,
        truncation,
        notes,
    })
}

/// The whole run.
pub async fn run(options: &Options) -> Result<VerdictFile> {
    let started = Instant::now();
    let prepared = prepare(options).await?;
    let Prepared {
        loaded,
        rubric,
        platform,
        requests,
        truncation,
        mut notes,
    } = prepared;
    let manifest = &loaded.manifest;
    if requests.is_empty() {
        return Err(Failure::usage(
            "no request to make: every source was left out or no shape matched",
        ));
    }

    let assessor = match &options.replay {
        Some(dir) => Assessor::replay(
            dir,
            options.model.as_deref().unwrap_or("replay"),
            &rubric.name,
        )?,
        None => {
            let backend = Backend::resolve(options.model.as_deref())?;
            check_tuning(&rubric, &backend.model, options.allow_untuned, &mut notes)?;
            Assessor::client(&backend, &rubric.name, options.max_in_flight)?
        }
    };
    let assessor = match &options.record {
        Some(dir) => assessor.recording_to(dir),
        None => assessor,
    };
    log(format!(
        "rubric {} v{} · {} request(s) · platform {} · {}",
        rubric.name,
        rubric.version.as_deref().unwrap_or("?"),
        requests.len(),
        platform.name(),
        assessor.describe
    ));
    let evidence_hash = verdict::evidence_hash(&requests);
    if let Some(path) = &options.cases_out {
        let added = crate::cases::append(path, &rubric.name, &requests)?;
        log(format!("cases: {added} added to {}", path.display()));
    }
    let answered = assessor.answer_all(&rubric, requests).await?;
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let file = verdict::build(
        verdict::Context {
            manifest,
            rubric: &rubric,
            truncation,
            evidence_hash,
            elapsed_ms,
            notes,
        },
        &answered,
    )?;
    file.write(&options.out)?;
    log(format!(
        "verdict {} · {} finding(s) · {} ms · {} input tokens → {}",
        file.verdict,
        file.findings.len(),
        elapsed_ms,
        file.ci_judge.input_tokens,
        options.out.display()
    ));
    Ok(file)
}

/// A rubric tuned on one model must not be run on another without saying so.
pub fn check_tuning(
    rubric: &Rubric,
    model: &str,
    allow_untuned: bool,
    notes: &mut Vec<String>,
) -> Result<()> {
    match rubric
        .policy
        .tuning
        .as_ref()
        .and_then(|t| t.model.as_deref())
    {
        Some(tuned) if tuned == model => Ok(()),
        Some(tuned) if allow_untuned => {
            let note = format!("bars tuned on {tuned}, running {model} (--allow-untuned)");
            log(&note);
            notes.push(note);
            Ok(())
        }
        Some(tuned) => Err(Failure::usage(format!(
            "rubric {} is tuned on `{tuned}` but the configured model is `{model}`: re-tune, set the model, or pass --allow-untuned",
            rubric.name
        ))),
        None => {
            let note = format!(
                "rubric {} has no tuning block: its bars are untuned defaults",
                rubric.name
            );
            log(&note);
            notes.push(note);
            Ok(())
        }
    }
}

/// `explain`: the requests and the questions each would send, no call.
pub fn explain(prepared: &Prepared) -> Result<Value> {
    let mut out = Vec::new();
    for r in &prepared.requests {
        let asked = prepared
            .rubric
            .lower(&r.state, &Default::default())
            .map_err(|e| Failure::usage(format!("lowering {}: {e}", r.id)))?;
        out.push(serde_json::json!({
            "id": r.id,
            "shape": r.shape,
            "locator": r.path.as_ref().map(|p| match r.line { Some(l) => format!("{p}:{l}"), None => p.clone() }),
            "bytes": state_bytes(&r.state),
            "questions": asked.ids().collect::<Vec<_>>(),
            "state": r.state,
        }));
    }
    Ok(serde_json::json!({
        "rubric": prepared.rubric.name,
        "platform": prepared.platform.name(),
        "requests": out,
        "truncation": prepared.truncation,
        "notes": prepared.notes,
    }))
}

/// `collect`: write every request's state under `dir`.
pub fn write_requests(prepared: &Prepared, dir: &Path) -> Result<usize> {
    std::fs::create_dir_all(dir)?;
    for r in &prepared.requests {
        let text = serde_json::to_string_pretty(r).map_err(|e| Failure::usage(e.to_string()))?;
        std::fs::write(dir.join(format!("{}.json", r.id)), text + "\n")?;
    }
    Ok(prepared.requests.len())
}
