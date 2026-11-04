//! The `ci-judge` command line.

use std::path::PathBuf;
use std::process::ExitCode;

use ci_judge::error::Failure;
use ci_judge::{eval, gate, manifest, run};
use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ci-judge", version, about = "Merge-request evidence to a calibrated verdict file for a CI gate", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Args, Clone)]
struct RunArgs {
    /// A bundled rubric name (infra-plan, mr-hygiene, failed-job-triage), a
    /// manifest path, or a directory holding manifest.yml.
    #[arg(long, env = "CI_JUDGE_RUBRIC")]
    rubric: String,
    /// The verdict file to write; the judge-gate block reads it.
    #[arg(long, env = "CI_JUDGE_OUT", default_value = "judge-gate.verdict.json")]
    out: PathBuf,
    /// The model to ask; else the jud configuration's.
    #[arg(long, env = "CI_JUDGE_MODEL")]
    model: Option<String>,
    /// Answer from recordings under DIR; no key, no network.
    #[arg(long, env = "CI_JUDGE_REPLAY", value_name = "DIR")]
    replay: Option<PathBuf>,
    /// Record every answer under DIR (shadow mode).
    #[arg(long, env = "CI_JUDGE_RECORD", value_name = "DIR")]
    record: Option<PathBuf>,
    /// Append this run's requests as unlabelled cases to FILE (shadow mode).
    #[arg(long, env = "CI_JUDGE_CASES_OUT", value_name = "FILE")]
    cases_out: Option<PathBuf>,
    /// Cap on per-item requests per shape.
    #[arg(long, env = "CI_JUDGE_MAX_ITEMS")]
    max_items: Option<usize>,
    /// Cap on the global state's size; lowest-priority keys are dropped first.
    #[arg(long, env = "CI_JUDGE_BUDGET_BYTES", default_value_t = 512_000)]
    budget_bytes: usize,
    /// Requests in flight at once.
    #[arg(long, env = "CI_JUDGE_MAX_IN_FLIGHT", default_value_t = 4)]
    max_in_flight: usize,
    /// Run a rubric tuned on another model.
    #[arg(long, env = "CI_JUDGE_ALLOW_UNTUNED")]
    allow_untuned: bool,
    /// Let a repo-local manifest declare critical or blocker severities.
    #[arg(long, env = "CI_JUDGE_ALLOW_LOCAL_BLOCKERS")]
    allow_local_blockers: bool,
    /// gitlab, github or local; detected from the environment by default.
    #[arg(long, env = "CI_JUDGE_PLATFORM")]
    platform: Option<String>,
}

impl From<RunArgs> for run::Options {
    fn from(a: RunArgs) -> Self {
        Self {
            rubric: a.rubric,
            out: a.out,
            model: a.model,
            replay: a.replay,
            record: a.record,
            cases_out: a.cases_out,
            max_items: a.max_items,
            budget_bytes: a.budget_bytes,
            max_in_flight: a.max_in_flight,
            allow_untuned: a.allow_untuned,
            allow_local_blockers: a.allow_local_blockers,
            platform: a.platform,
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Collect the evidence, ask the rubric, write the verdict file.
    Run(RunArgs),
    /// Collect and shape the requests, write them under a directory, no call.
    Collect {
        #[command(flatten)]
        args: RunArgs,
        /// Where the requests go.
        #[arg(long, default_value = "ci-judge-requests")]
        dir: PathBuf,
    },
    /// Print the requests and the questions each would send, no call.
    Explain(RunArgs),
    /// Turn a verdict file into a pipeline decision (the judge-gate block, in Rust).
    Gate {
        /// shadow, advise, warn, block or route.
        #[arg(long, env = "JUDGE_GATE_MODE", default_value = "shadow")]
        mode: gate::Mode,
        /// The verdict file the judge wrote.
        #[arg(
            long,
            env = "JUDGE_GATE_VERDICT_FILE",
            default_value = "judge-gate.verdict.json"
        )]
        verdict: PathBuf,
        /// The label that turns a block into a warning.
        #[arg(
            long,
            env = "JUDGE_GATE_OVERRIDE_LABEL",
            default_value = "judge-gate::override"
        )]
        override_label: String,
        /// Directory of the route pipelines, route mode.
        #[arg(long, env = "JUDGE_GATE_ROUTES", default_value = ".gitlab/judge-gate")]
        routes: PathBuf,
        /// Where the gate's files go.
        #[arg(long, env = "JUDGE_GATE_OUT", default_value = ".")]
        out: PathBuf,
        /// The gate's name in its note; the job's name by default.
        #[arg(long, env = "JUDGE_GATE_NAME")]
        name: Option<String>,
        #[arg(long, env = "CI_JUDGE_PLATFORM")]
        platform: Option<String>,
    },
    /// Check a rubric, its manifest, its cases and recordings.
    Check {
        /// Bundled names, manifest paths or directories; every bundled rubric by default.
        rubrics: Vec<String>,
    },
    /// Grade a rubric's labelled cases from recordings, hold the promotion thresholds.
    Eval {
        #[arg(long, env = "CI_JUDGE_RUBRIC")]
        rubric: String,
        /// Recordings directory; the manifest's `recordings` by default.
        #[arg(long, env = "CI_JUDGE_REPLAY")]
        replay: Option<PathBuf>,
        /// Print the report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// The resolved backend and where each value came from; never the key.
    Config,
    /// Print the bundled rubric catalog.
    Catalog,
    /// Print the version, for a script that expects a subcommand.
    Version,
}

fn fail(failure: &Failure) -> ExitCode {
    eprintln!("ci-judge: {}", failure.message);
    failure.exit.into()
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(cli.command).await {
        Ok(exit) => exit.into(),
        Err(failure) => fail(&failure),
    }
}

async fn dispatch(command: Command) -> Result<ci_judge::Exit, Failure> {
    use ci_judge::Exit;
    match command {
        Command::Run(args) => {
            run::run(&args.into()).await?;
            Ok(Exit::Ok)
        }
        Command::Collect { args, dir } => {
            let prepared = run::prepare(&args.into()).await?;
            let n = run::write_requests(&prepared, &dir)?;
            run::log(format!("{n} request(s) written under {}", dir.display()));
            Ok(Exit::Ok)
        }
        Command::Explain(args) => {
            let prepared = run::prepare(&args.into()).await?;
            let explained = run::explain(&prepared)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&explained).unwrap_or_default()
            );
            Ok(Exit::Ok)
        }
        Command::Gate {
            mode,
            verdict,
            override_label,
            routes,
            out,
            name,
            platform,
        } => {
            let decision = gate::gate(&gate::Options {
                mode,
                verdict_file: verdict,
                override_label,
                routes_dir: routes,
                out_dir: out,
                name,
                platform,
            })
            .await?;
            Ok(decision.exit)
        }
        Command::Check { rubrics } => {
            let names: Vec<String> = if rubrics.is_empty() {
                ci_judge::rubrics::names()
                    .iter()
                    .map(|s| (*s).to_owned())
                    .collect()
            } else {
                rubrics
            };
            let mut status = Exit::Ok;
            for name in names {
                match manifest::Manifest::load(&name).and_then(|loaded| eval::check(&loaded)) {
                    Ok(lines) => {
                        println!("{name}: ok");
                        for line in lines {
                            println!("  {line}");
                        }
                    }
                    Err(e) => {
                        println!("{name}: error: {}", e.message);
                        status = Exit::Usage;
                    }
                }
            }
            Ok(status)
        }
        Command::Eval {
            rubric,
            replay,
            json,
        } => {
            let loaded = manifest::Manifest::load(&rubric)?;
            let replay = replay.unwrap_or_else(|| loaded.recordings_dir());
            let report = eval::eval(&loaded, &replay).await?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).unwrap_or_default()
                );
            } else {
                println!(
                    "rubric {}: {} case(s), {} graded",
                    report.rubric, report.cases, report.graded
                );
                for (id, m) in &report.questions {
                    println!(
                        "  {id}: accuracy {} over {} labelled, brier {}, ece {}{}",
                        m.accuracy.map_or("-".to_owned(), |a| format!("{a:.3}")),
                        m.labelled,
                        m.brier.map_or("-".to_owned(), |b| format!("{b:.3}")),
                        m.ece.map_or("-".to_owned(), |e| format!("{e:.3}")),
                        report
                            .proposed_thresholds
                            .get(id)
                            .map_or(String::new(), |t| format!(", best-F1 threshold {t:.2}"))
                    );
                }
                for f in &report.failures {
                    println!("  below bar: {f}");
                }
                if !report.missing_recordings.is_empty() {
                    eprintln!(
                        "ci-judge: no recording answers {} case(s): {}; record them first with a shadow run",
                        report.missing_recordings.len(),
                        report.missing_recordings.join(", ")
                    );
                }
            }
            Ok(report.exit())
        }
        Command::Config => {
            let backend = ci_judge::config::Backend::resolve(None)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "config_file": backend.config_file.as_ref().map(|p| p.display().to_string()),
                    "base_url": backend.public_url(),
                    "base_url_from": backend.base_url_from.to_string(),
                    "model": backend.model,
                    "model_from": backend.model_from.to_string(),
                    "timeout_secs": backend.timeout_secs,
                    "long_context": backend.long_context,
                    "api_key": backend.api_key_from,
                }))
                .unwrap_or_default()
            );
            Ok(Exit::Ok)
        }
        Command::Version => {
            println!("ci-judge {}", env!("CARGO_PKG_VERSION"));
            Ok(Exit::Ok)
        }
        Command::Catalog => {
            for b in ci_judge::rubrics::CATALOG {
                let files: Vec<&str> = b.files.iter().map(|(f, _)| *f).collect();
                println!("{}: {}", b.name, files.join(", "));
            }
            Ok(Exit::Ok)
        }
    }
}
