//! Collectors read one state source each and hand it to a reducer.

use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, anyhow};
use serde_json::{Value, json};

use crate::config::expand_env;
use crate::manifest::{SourceKind, StateSource};
use crate::platform::Platform;
use crate::reduce::{self, Reduced};

/// What `collect` could not do; `optional` sources turn it into a note.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct CollectError(pub String);

pub async fn collect(spec: &StateSource, platform: &Platform) -> Result<Reduced, CollectError> {
    let fail = |e: anyhow::Error| CollectError(format!("{e:#}"));
    match spec.source {
        SourceKind::Artifact => artifact(spec).map_err(fail),
        SourceKind::File => {
            let path = expand_env(spec.path.as_deref().unwrap_or_default());
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {path}"))
                .map_err(fail)?;
            reduce::apply(
                spec.reduce.as_deref().unwrap_or("text"),
                &text,
                spec.tail_lines,
            )
            .map_err(|e| CollectError(e.message))
        }
        SourceKind::Env => {
            let name = expand_env(spec.path.as_deref().unwrap_or_default());
            let value =
                std::env::var(&name).map_err(|_| CollectError(format!("{name} is not set")))?;
            reduce::apply(
                spec.reduce.as_deref().unwrap_or("text"),
                &value,
                spec.tail_lines,
            )
            .map_err(|e| CollectError(e.message))
        }
        SourceKind::Mr => {
            let change = platform.change();
            Ok(Reduced {
                value: serde_json::to_value(&change).unwrap_or(Value::Null),
                items: Vec::new(),
            })
        }
        SourceKind::Diff => diff(platform).await.map_err(fail),
        SourceKind::Joblog => joblog(spec, platform).await.map_err(fail),
    }
}

fn artifact(spec: &StateSource) -> anyhow::Result<Reduced> {
    let pattern = expand_env(spec.path.as_deref().unwrap_or_default());
    let reducer = spec
        .reduce
        .as_deref()
        .unwrap_or_else(|| default_reducer(&pattern));
    let paths = glob(&pattern)?;
    if paths.is_empty() {
        return Err(anyhow!("no file matches {pattern}"));
    }
    if paths.len() == 1 {
        let text =
            std::fs::read_to_string(&paths[0]).with_context(|| format!("reading {}", paths[0]))?;
        return reduce::apply(reducer, &text, spec.tail_lines).map_err(|e| anyhow!(e.message));
    }
    // Several files: each reduced, values listed, items pooled with the file in the key.
    let mut out = Reduced {
        value: Value::Array(Vec::new()),
        items: Vec::new(),
    };
    for path in paths {
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {path}"))?;
        let mut one =
            reduce::apply(reducer, &text, spec.tail_lines).map_err(|e| anyhow!(e.message))?;
        if let Value::Array(list) = &mut out.value {
            list.push(json!({ "file": path, "value": one.value }));
        }
        for item in &mut one.items {
            item.key = reduce::safe_key(&format!(
                "{}-{}",
                Path::new(&path)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy(),
                item.key
            ));
        }
        out.items.extend(one.items);
    }
    Ok(out)
}

fn default_reducer(path: &str) -> &'static str {
    if path.ends_with(".xml") {
        "junit"
    } else if path.ends_with(".json") {
        "json"
    } else if path.ends_with(".diff") || path.ends_with(".patch") {
        "diffhunks"
    } else {
        "text"
    }
}

/// Files matching a glob, sorted; a plain path is itself.
fn glob(pattern: &str) -> anyhow::Result<Vec<String>> {
    if !pattern.contains(['*', '?', '[']) {
        return Ok(if Path::new(pattern).is_file() {
            vec![pattern.to_owned()]
        } else {
            Vec::new()
        });
    }
    let matcher = globset::GlobBuilder::new(pattern)
        .literal_separator(false)
        .build()
        .with_context(|| format!("glob {pattern}"))?
        .compile_matcher();
    let root = pattern.split(['*', '?', '[']).next().unwrap_or("");
    let root = Path::new(root)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != ".git") {
                    stack.push(path);
                }
            } else {
                let rel = path
                    .strip_prefix("./")
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                if matcher.is_match(&rel) || matcher.is_match(&path) {
                    found.push(rel);
                }
            }
        }
    }
    found.sort();
    Ok(found)
}

async fn git(args: &[&str]) -> anyhow::Result<String> {
    let out = tokio::process::Command::new("git")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await
        .context("running git")?;
    if !out.status.success() {
        return Err(anyhow!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `git diff` from the merge base of the change to the working tree's
/// HEAD. Needs history (`GIT_DEPTH: 0` on GitLab, `fetch-depth: 0` on
/// GitHub); without the base sha, the diff of the last commit.
async fn diff(platform: &Platform) -> anyhow::Result<Reduced> {
    let change = platform.change();
    let text = match change.base_sha.as_deref() {
        Some(base) => match git(&["diff", "--no-color", "--unified=3", "--merge-base", base, "HEAD"]).await {
            Ok(text) => text,
            Err(_) => git(&["diff", "--no-color", "--unified=3", base, "HEAD"])
                .await
                .context("diff against the base sha (is the history fetched? GIT_DEPTH: 0 / fetch-depth: 0)")?,
        },
        None => git(&["diff", "--no-color", "--unified=3", "HEAD~1", "HEAD"])
            .await
            .context("diff of the last commit (no base sha known)")?,
    };
    let mut reduced = reduce::diff::reduce(&text);
    if let Value::Object(map) = &mut reduced.value {
        map.insert("base".to_owned(), json!(change.base_sha));
        map.insert("head".to_owned(), json!(change.source_sha));
    }
    Ok(reduced)
}

async fn joblog(spec: &StateSource, platform: &Platform) -> anyhow::Result<Reduced> {
    let tail = spec.tail_lines.unwrap_or(200);
    match platform {
        Platform::GitLab(gl) => {
            let jobs = gl.failed_jobs(tail).await?;
            let items = jobs
                .iter()
                .map(|j| reduce::Item {
                    key: reduce::safe_key(&format!("job-{}", j.name)),
                    value: serde_json::to_value(j).unwrap_or(Value::Null),
                    path: None,
                    line: None,
                })
                .collect::<Vec<_>>();
            Ok(Reduced {
                value: json!({ "failed": jobs.len(), "jobs": jobs }),
                items,
            })
        }
        Platform::GitHub(_) => Err(anyhow!(
            "joblog is not available on GitHub Actions yet: pass the failing step's log as an artifact source"
        )),
        Platform::Local => Err(anyhow!("joblog needs a CI pipeline")),
    }
}
