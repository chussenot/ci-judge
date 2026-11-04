//! `ci-judge gate`: the decision step, for GitHub workflows and anywhere
//! the GitLab `judge-gate` block is not available. It reads the verdict
//! file, applies the mode, and reports the way each platform shows
//! things: a pull request comment, annotations, the step summary and
//! `GITHUB_OUTPUT` on GitHub; a merge request note, a Code Quality
//! report and a dotenv on GitLab; files and the log elsewhere.
//!
//! Exit statuses are the block's: 0 pass, 1 blocked, 2 gate error, 3
//! warning.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::error::{Exit, Failure, Result};
use crate::platform::Platform;
use crate::verdict::VerdictFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[clap(rename_all = "lowercase")]
pub enum Mode {
    Shadow,
    Advise,
    Warn,
    Block,
    Route,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shadow => "shadow",
            Self::Advise => "advise",
            Self::Warn => "warn",
            Self::Block => "block",
            Self::Route => "route",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    pub mode: Mode,
    pub verdict_file: PathBuf,
    pub override_label: String,
    pub routes_dir: PathBuf,
    pub out_dir: PathBuf,
    pub name: Option<String>,
    pub platform: Option<String>,
}

/// What the gate decided, for the log and the tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub exit: Exit,
    pub verdict: String,
    pub route: Option<String>,
    pub overridden: bool,
}

fn log(message: impl AsRef<str>) {
    eprintln!("judge-gate: {}", message.as_ref());
}

pub async fn gate(options: &Options) -> Result<Decision> {
    let platform = Platform::detect(options.platform.as_deref())?;
    let value = VerdictFile::read_loose(&options.verdict_file)?;
    let verdict = value["verdict"].as_str().unwrap_or("fail").to_owned();
    let out = &options.out_dir;
    std::fs::create_dir_all(out)?;

    // Every mode stores the verdict and a dotenv.
    let stored = out.join("judge-gate.verdict.json");
    if !same_file(&options.verdict_file, &stored) {
        std::fs::copy(&options.verdict_file, &stored)?;
    }
    let mut env = format!("JUDGE_GATE_VERDICT={verdict}\n");
    log(format!("verdict {verdict}, mode {}", options.mode.as_str()));
    if options.mode == Mode::Shadow {
        std::fs::write(out.join("judge-gate.env"), &env)?;
        log("shadow mode, verdict stored as an artifact, nothing posted");
        return Ok(Decision {
            exit: Exit::Ok,
            verdict,
            route: None,
            overridden: false,
        });
    }

    for line in finding_lines(&value) {
        log(line);
    }
    if options.mode == Mode::Route {
        let route = value
            .get("route")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| verdict.clone());
        let file = options.routes_dir.join(format!("{route}.yml"));
        if !file.is_file() {
            return Err(Failure::usage(format!(
                "no route pipeline {} for verdict {verdict}",
                file.display()
            )));
        }
        std::fs::copy(&file, out.join("judge-gate.route.yml"))?;
        env.push_str(&format!("JUDGE_GATE_ROUTE={route}\n"));
        std::fs::write(out.join("judge-gate.env"), &env)?;
        github_output(&platform, &verdict, Some(&route))?;
        log(format!(
            "route {route} selected: {} is the child pipeline",
            file.display()
        ));
        return Ok(Decision {
            exit: Exit::Ok,
            verdict,
            route: Some(route),
            overridden: false,
        });
    }
    std::fs::write(out.join("judge-gate.env"), &env)?;
    github_output(&platform, &verdict, None)?;

    let overridden = platform
        .labels()
        .iter()
        .any(|l| l == &options.override_label);
    let name = options.name.clone().unwrap_or_else(|| match &platform {
        Platform::GitLab(gl) => gl
            .job_name
            .clone()
            .unwrap_or_else(|| "judge-gate".to_owned()),
        Platform::GitHub(gh) => gh
            .job_name
            .clone()
            .unwrap_or_else(|| "judge-gate".to_owned()),
        Platform::Local => "judge-gate".to_owned(),
    });
    let marker = format!("<!-- judge-gate:{name} -->");
    let body = render_markdown(
        &value,
        &marker,
        &name,
        options.mode,
        &options.override_label,
        overridden,
        &platform,
    );

    match &platform {
        Platform::GitLab(gl) => {
            std::fs::write(
                out.join("gl-code-quality-judge-gate.json"),
                code_quality(&value),
            )?;
            if gl.mr_iid.is_some() && gl.token.is_some() {
                match gl.upsert_note(&marker, &body).await {
                    Ok(action) => log(format!("merge request note {action}")),
                    Err(e) => log(format!(
                        "WARNING: the merge request note could not be posted: {e:#}"
                    )),
                }
            } else {
                log("no merge request note (needs a merge request pipeline and GITLAB_TOKEN)");
            }
        }
        Platform::GitHub(gh) => {
            for ann in annotations(&value) {
                println!("{ann}");
            }
            if let Some(summary) = &gh.summary_file {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(summary)
                {
                    let _ = writeln!(f, "{body}");
                }
            }
            if gh.pr_number.is_some() && gh.token.is_some() {
                match gh.upsert_comment(&marker, &body).await {
                    Ok(action) => log(format!("pull request comment {action}")),
                    Err(e) => log(format!(
                        "WARNING: the pull request comment could not be posted: {e:#}"
                    )),
                }
            } else {
                log("no pull request comment (needs a pull_request event and GITHUB_TOKEN)");
            }
        }
        Platform::Local => {
            std::fs::write(
                out.join("gl-code-quality-judge-gate.json"),
                code_quality(&value),
            )?;
            std::fs::write(out.join("judge-gate.note.md"), &body)?;
        }
    }

    let exit = match (verdict.as_str(), options.mode, overridden) {
        ("fail", Mode::Warn, _) => {
            log("WARNING: verdict fail, the job passes with warnings (warn mode)");
            Exit::Warn
        }
        ("fail", Mode::Block, true) => {
            log(format!(
                "WARNING: verdict fail overridden by the {} label (block mode)",
                options.override_label
            ));
            Exit::Warn
        }
        ("fail", Mode::Block, false) => {
            log(format!(
                "verdict fail, the job fails (block mode). Human override: add the {} label and run the pipeline again",
                options.override_label
            ));
            Exit::Failed
        }
        _ => Exit::Ok,
    };
    Ok(Decision {
        exit,
        verdict,
        route: None,
        overridden,
    })
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn github_output(platform: &Platform, verdict: &str, route: Option<&str>) -> Result<()> {
    if let Platform::GitHub(gh) = platform
        && let Some(path) = &gh.output_file
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)?;
        writeln!(f, "verdict={verdict}")?;
        if let Some(route) = route {
            writeln!(f, "route={route}")?;
        }
    }
    Ok(())
}

fn findings(value: &Value) -> Vec<&Value> {
    value
        .get("findings")
        .and_then(Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default()
}

fn severity(f: &Value) -> &str {
    match f.get("severity").and_then(Value::as_str) {
        Some(s @ ("info" | "minor" | "major" | "critical" | "blocker")) => s,
        _ => "major",
    }
}

fn location(f: &Value) -> Option<String> {
    let path = f.get("path").and_then(Value::as_str)?;
    Some(match f.get("line").and_then(Value::as_u64) {
        Some(line) => format!("{path}:{line}"),
        None => path.to_owned(),
    })
}

fn finding_lines(value: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(summary) = value
        .get("summary")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        lines.push(summary.to_owned());
    }
    for f in findings(value) {
        let desc = f.get("description").and_then(Value::as_str).unwrap_or("");
        lines.push(match location(f) {
            Some(loc) => format!("  [{}] {loc}: {desc}", severity(f)),
            None => format!("  [{}] {desc}", severity(f)),
        });
    }
    lines
}

/// The Code Quality report, as the block writes it: findings with a path.
pub fn code_quality(value: &Value) -> String {
    use sha2::{Digest, Sha256};
    let report: Vec<Value> = findings(value)
        .into_iter()
        .filter(|f| f.get("path").is_some_and(Value::is_string))
        .map(|f| {
            let path = f["path"].as_str().unwrap_or("");
            let line = f.get("line").and_then(Value::as_u64).unwrap_or(1);
            let desc = f.get("description").and_then(Value::as_str).unwrap_or("");
            let mut hasher = Sha256::new();
            hasher.update(format!("{path}:{line}:{desc}").as_bytes());
            let fp: String = hasher
                .finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            json!({
                "description": desc,
                "check_name": "judge-gate",
                "fingerprint": fp,
                "severity": severity(f),
                "location": { "path": path, "lines": { "begin": line } }
            })
        })
        .collect();
    serde_json::to_string_pretty(&report).unwrap_or_else(|_| "[]".to_owned())
}

/// GitHub workflow commands, one per finding with a path.
pub fn annotations(value: &Value) -> Vec<String> {
    findings(value)
        .into_iter()
        .map(|f| {
            let level = match severity(f) {
                "info" | "minor" => "notice",
                "major" => "warning",
                _ => "error",
            };
            let desc = f
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .replace('%', "%25")
                .replace('\n', "%0A");
            match (
                f.get("path").and_then(Value::as_str),
                f.get("line").and_then(Value::as_u64),
            ) {
                (Some(path), Some(line)) => {
                    format!("::{level} file={path},line={line},title=judge-gate::{desc}")
                }
                (Some(path), None) => format!("::{level} file={path},title=judge-gate::{desc}"),
                _ => format!("::{level} title=judge-gate::{desc}"),
            }
        })
        .collect()
}

fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

#[allow(clippy::too_many_arguments)]
fn render_markdown(
    value: &Value,
    marker: &str,
    name: &str,
    mode: Mode,
    label: &str,
    overridden: bool,
    platform: &Platform,
) -> String {
    let verdict = value["verdict"].as_str().unwrap_or("fail");
    let icon = if verdict == "pass" { "✅" } else { "❌" };
    let job_url = match platform {
        Platform::GitLab(gl) => gl.job_url.clone(),
        Platform::GitHub(gh) => gh
            .run_id
            .as_ref()
            .map(|id| format!("https://github.com/{}/actions/runs/{id}", gh.repository)),
        Platform::Local => None,
    };
    let mut md = String::new();
    md.push_str(marker);
    md.push('\n');
    md.push_str(&format!(
        "### {icon} Judge gate `{name}`: {} (mode `{}`{})\n\n",
        verdict.to_uppercase(),
        mode.as_str(),
        job_url.map(|u| format!(", [job]({u})")).unwrap_or_default()
    ));
    if let Some(summary) = value
        .get("summary")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        md.push_str(summary);
        md.push_str("\n\n");
    }
    let fs = findings(value);
    if !fs.is_empty() {
        md.push_str("| Severity | Location | Finding |\n|---|---|---|\n");
        for f in &fs {
            let loc = location(f).map(|l| format!("`{l}`")).unwrap_or_default();
            md.push_str(&format!(
                "| {} | {loc} | {} |\n",
                severity(f),
                cell(f.get("description").and_then(Value::as_str).unwrap_or(""))
            ));
        }
        md.push('\n');
    }
    if let Some(extra) = value.get("ci_judge") {
        let model = extra.get("model").and_then(Value::as_str).unwrap_or("?");
        let requests = extra.get("requests").and_then(Value::as_u64).unwrap_or(0);
        let tokens = extra
            .get("input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let cost = extra
            .get("cost")
            .and_then(Value::as_f64)
            .map(|c| format!(", ${c:.4}"))
            .unwrap_or_default();
        let rubric = extra
            .pointer("/rubric/name")
            .and_then(Value::as_str)
            .unwrap_or("?");
        md.push_str(&format!(
            "<details><summary>rubric {rubric}, model {model}, {requests} request(s), {tokens} input tokens{cost}</summary>\n\n```json\n{}\n```\n</details>\n\n",
            serde_json::to_string_pretty(&extra.get("answers").cloned().unwrap_or(Value::Null)).unwrap_or_default()
        ));
    }
    match (verdict, mode) {
        ("fail", Mode::Block) if overridden => md.push_str(&format!("_This verdict fails the job, overridden by the `{label}` label._\n")),
        ("fail", Mode::Block) => md.push_str(&format!(
            "_This verdict fails the job. Human override: add the `{label}` label to the merge request and run the pipeline again._\n"
        )),
        ("fail", Mode::Warn) => md.push_str("_This verdict only warns: nothing is blocked._\n"),
        _ => {}
    }
    md
}
