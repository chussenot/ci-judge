//! The whole path with a scripted backend: fixtures on disk, the bundled
//! infra-plan rubric, a `Fake` that answers as a model would, the verdict
//! file the `judge-gate` block validates, the gate in every mode, and an
//! eval over what a `Recorder` kept.

use std::path::Path;

use ci_judge::assess::Assessor;
use ci_judge::gate::{self, Mode};
use ci_judge::manifest::Manifest;
use ci_judge::requests::{self, Collected};
use ci_judge::run;
use ci_judge::verdict::{self, validate_loose};
use ci_judge::{Exit, eval};
use judgment::jud::{Document, parse};
use judgment::{Fake, Recorder};

const TFPLAN: &str = r#"{"format_version":"1.2","resource_changes":[
 {"address":"aws_db_instance.main","type":"aws_db_instance","name":"main","change":{"actions":["delete"],"before":{"id":"db","password":"hunter2"},"after":null}},
 {"address":"aws_security_group.app","type":"aws_security_group","name":"app","change":{"actions":["delete","create"],"before":{},"after":{}}},
 {"address":"aws_s3_bucket.logs","type":"aws_s3_bucket","name":"logs","change":{"actions":["update"],"before":{},"after":{}}}
]}"#;

struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("tfplan.json"), TFPLAN).unwrap();
        std::fs::create_dir_all(dir.path().join("infra")).unwrap();
        std::fs::write(
            dir.path().join("infra/main.tf"),
            "resource \"aws_db_instance\" \"main\" {}\n\nresource \"aws_security_group\" \"app\" {}\n",
        )
        .unwrap();
        Self { dir }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }
}

/// A scripted model: the deletion is not acknowledged (0.2), the plan is
/// not reversible (0.3), blast radius is platform with high confidence.
fn fake() -> Fake {
    Fake::new()
        .model("fake-1.0")
        .noul("destructive_ack", 0.2)
        .unwrap()
        .noul("rollback_obvious", 0.3)
        .unwrap()
        .score("blast_radius", [0.05, 0.1, 0.7, 0.15], 0.8)
        .unwrap()
}

fn options(fixture: &Fixture) -> run::Options {
    run::Options {
        rubric: "infra-plan".to_owned(),
        out: fixture.path().join("judge-gate.verdict.json"),
        model: Some("fake-1.0".to_owned()),
        replay: None,
        record: None,
        cases_out: None,
        max_items: None,
        budget_bytes: 512_000,
        max_in_flight: 2,
        allow_untuned: true,
        allow_local_blockers: false,
        platform: Some("local".to_owned()),
    }
}

/// Run the pipeline the way `ci-judge run` does, with `backend` in place
/// of the configured client, from inside `fixture`.
async fn run_with(
    fixture: &Fixture,
    backend: impl judgment::SystemOne + 'static,
    record: Option<&Path>,
) -> verdict::VerdictFile {
    let _guard = chdir(fixture.path());
    let prepared = run::prepare(&options(fixture)).await.unwrap();
    let assessor = Assessor::with_backend(backend, "fake-1.0", &prepared.rubric.name);
    let assessor = match record {
        Some(dir) => assessor.recording_to(dir),
        None => assessor,
    };
    let evidence_hash = verdict::evidence_hash(&prepared.requests);
    let answered = assessor
        .answer_all(&prepared.rubric, prepared.requests.clone())
        .await
        .unwrap();
    verdict::build(
        verdict::Context {
            manifest: &prepared.loaded.manifest,
            rubric: &prepared.rubric,
            truncation: prepared.truncation.clone(),
            evidence_hash,
            elapsed_ms: 1,
            notes: prepared.notes.clone(),
        },
        &answered,
    )
    .unwrap()
}

/// Tests share one process: hold the lock while the working directory is changed.
static CWD: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Guard {
    previous: std::path::PathBuf,
    _lock: std::sync::MutexGuard<'static, ()>,
}

fn chdir(dir: &Path) -> Guard {
    let lock = CWD.lock().unwrap_or_else(|e| e.into_inner());
    let previous = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir).unwrap();
    Guard {
        previous,
        _lock: lock,
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.previous);
    }
}

#[tokio::test]
async fn one_request_per_destructive_change_plus_one_global() {
    let fixture = Fixture::new();
    let _guard = chdir(fixture.path());
    let prepared = run::prepare(&options(&fixture)).await.unwrap();
    let shapes: Vec<&str> = prepared.requests.iter().map(|r| r.shape.as_str()).collect();
    assert_eq!(shapes, ["change", "change", "plan"]);
    assert_eq!(prepared.requests[0].path.as_deref(), Some("infra/main.tf"));
    assert_eq!(prepared.requests[0].line, Some(1));
    // An item request carries its item and never the item source; the global one carries the source.
    assert!(prepared.requests[0].state.get("change").is_some());
    assert!(prepared.requests[0].state.get("plan").is_none());
    assert!(prepared.requests[2].state.get("plan").is_some());
    // The password value never reaches the state, only the attribute name.
    let text = serde_json::to_string(&prepared.requests[0].state).unwrap();
    assert!(!text.contains("hunter2"), "{text}");
    // `when` gives each request only its questions.
    let asked_item = prepared
        .rubric
        .lower(&prepared.requests[0].state, &Default::default())
        .unwrap();
    assert_eq!(asked_item.ids().collect::<Vec<_>>(), ["destructive_ack"]);
    let asked_plan = prepared
        .rubric
        .lower(&prepared.requests[2].state, &Default::default())
        .unwrap();
    assert_eq!(
        asked_plan.ids().collect::<Vec<_>>(),
        ["rollback_obvious", "blast_radius"]
    );
}

#[tokio::test]
async fn verdict_fails_on_a_blocker_and_passes_the_blocks_validation() {
    let fixture = Fixture::new();
    let file = run_with(&fixture, fake(), None).await;
    assert_eq!(file.verdict, "fail");
    let blockers: Vec<_> = file
        .findings
        .iter()
        .filter(|f| f.severity == ci_judge::manifest::Severity::Blocker)
        .collect();
    assert_eq!(blockers.len(), 2, "{:?}", file.findings);
    assert_eq!(blockers[0].path.as_deref(), Some("infra/main.tf"));
    assert!(
        blockers[0]
            .description
            .contains("delete of aws_db_instance.main"),
        "{}",
        blockers[0].description
    );
    assert!(
        file.findings
            .iter()
            .any(|f| f.description.contains("manual steps"))
    );
    // The gates fingerprint `path:line:description`: no probability in it.
    assert!(
        file.findings.iter().all(|f| !f.description.contains("(p=")),
        "{:?}",
        file.findings
    );
    assert_eq!(file.ci_judge.requests, 3);
    assert_eq!(file.ci_judge.model.as_deref(), Some("fake-1.0"));
    // The block's jq validator, in Rust.
    let value = serde_json::to_value(&file).unwrap();
    validate_loose(&value).unwrap();
    assert!(
        file.summary
            .contains("3 failing check(s) over 2 destructive change(s)"),
        "{}",
        file.summary
    );
}

#[tokio::test]
async fn undecided_answers_become_info_findings_not_failures() {
    let fixture = Fixture::new();
    let backend = Fake::new()
        .model("fake-1.0")
        .noul("destructive_ack", 0.65) // threshold 0.7, margin 0.1: undecided
        .unwrap()
        .noul("rollback_obvious", 0.9)
        .unwrap()
        .score("blast_radius", [0.9, 0.1, 0.0, 0.0], 0.9)
        .unwrap();
    let file = run_with(&fixture, backend, None).await;
    assert_eq!(file.verdict, "pass");
    assert!(
        file.findings
            .iter()
            .all(|f| f.severity == ci_judge::manifest::Severity::Info)
    );
    assert_eq!(file.findings.len(), 2);
    assert!(file.findings[0].description.starts_with("undecided"));
}

#[tokio::test]
async fn gate_applies_every_mode_like_the_block() {
    let fixture = Fixture::new();
    let file = run_with(&fixture, fake(), None).await;
    let verdict_path = fixture.path().join("judge-gate.verdict.json");
    file.write(&verdict_path).unwrap();
    let out = fixture.path().join("gate");
    let opts = |mode| gate::Options {
        mode,
        verdict_file: verdict_path.clone(),
        override_label: "judge-gate::override".to_owned(),
        routes_dir: fixture.path().join("routes"),
        out_dir: out.clone(),
        name: Some("test".to_owned()),
        platform: Some("local".to_owned()),
    };
    let shadow = gate::gate(&opts(Mode::Shadow)).await.unwrap();
    assert_eq!(shadow.exit, Exit::Ok);
    assert!(out.join("judge-gate.env").is_file());
    assert!(!out.join("gl-code-quality-judge-gate.json").exists());

    let advise = gate::gate(&opts(Mode::Advise)).await.unwrap();
    assert_eq!(advise.exit, Exit::Ok);
    let cq: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out.join("gl-code-quality-judge-gate.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        cq.as_array().unwrap().len(),
        2,
        "only findings with a path reach the report"
    );
    assert_eq!(cq[0]["severity"], "blocker");
    assert_eq!(cq[0]["location"]["path"], "infra/main.tf");
    let note = std::fs::read_to_string(out.join("judge-gate.note.md")).unwrap();
    assert!(note.starts_with("<!-- judge-gate:test -->"));
    assert!(note.contains("| blocker |"));

    assert_eq!(
        gate::gate(&opts(Mode::Warn)).await.unwrap().exit,
        Exit::Warn
    );
    assert_eq!(
        gate::gate(&opts(Mode::Block)).await.unwrap().exit,
        Exit::Failed
    );

    // Route mode copies the route file named after the route, or the verdict.
    std::fs::create_dir_all(fixture.path().join("routes")).unwrap();
    std::fs::write(
        fixture.path().join("routes/fail.yml"),
        "full:\n  script: [echo full]\n",
    )
    .unwrap();
    let route = gate::gate(&opts(Mode::Route)).await.unwrap();
    assert_eq!(route.exit, Exit::Ok);
    assert_eq!(route.route.as_deref(), Some("fail"));
    assert!(out.join("judge-gate.route.yml").is_file());
    assert!(
        std::fs::read_to_string(out.join("judge-gate.env"))
            .unwrap()
            .contains("JUDGE_GATE_ROUTE=fail")
    );
}

/// GitLab cuts `CI_MERGE_REQUEST_DESCRIPTION` at 2700 characters; the
/// `mr` source then reads the whole text from the merge request API.
#[tokio::test]
async fn gitlab_description_comes_from_the_merge_request_api() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let api_url = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).unwrap();
        let body = r#"{"iid":7,"description":"the whole text"}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        String::from_utf8_lossy(&buf[..n]).to_ascii_lowercase()
    });
    let gitlab = ci_judge::platform::GitLab {
        api_url,
        project_id: "42".to_owned(),
        project_path: None,
        pipeline_id: None,
        job_name: None,
        job_url: None,
        mr_iid: Some(7),
        token: Some("t".to_owned()),
        job_token: None,
    };
    assert_eq!(gitlab.description().await.unwrap(), "the whole text");
    let request = server.join().unwrap();
    assert!(
        request.starts_with("get /projects/42/merge_requests/7 "),
        "{request}"
    );
    assert!(request.contains("private-token: t"), "{request}");
}

#[tokio::test]
async fn invalid_verdict_is_a_gate_error() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("v.json");
    std::fs::write(&bad, r#"{ "verdict": "maybe", "findings": "none" }"#).unwrap();
    let err = gate::gate(&gate::Options {
        mode: Mode::Advise,
        verdict_file: bad,
        override_label: "x".to_owned(),
        routes_dir: dir.path().to_path_buf(),
        out_dir: dir.path().join("out"),
        name: None,
        platform: Some("local".to_owned()),
    })
    .await
    .unwrap_err();
    assert_eq!(err.exit, Exit::Usage);
    assert!(err.message.contains("no valid verdict"), "{}", err.message);
}

#[tokio::test]
async fn recordings_replay_and_grade_against_labels() {
    let fixture = Fixture::new();
    let recordings = fixture.path().join("recordings");
    // Shadow run: record what the fake answered, named after each request.
    let _ = run_with(
        &fixture,
        Recorder::new(fake(), &recordings),
        Some(&recordings),
    )
    .await;
    let names: Vec<String> = std::fs::read_dir(&recordings)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        names
            .iter()
            .any(|n| n == "local-change-aws_db_instance.main.jud"),
        "{names:?}"
    );
    // Those recordings are `.jud` Recording documents the crate reads back.
    let text = std::fs::read_to_string(recordings.join("local-plan.jud")).unwrap();
    assert!(matches!(parse(&text).unwrap(), Document::Recording(_)));

    // A Cases document labelling the same states as the requests, graded from the recordings.
    let _guard = chdir(fixture.path());
    let prepared = run::prepare(&options(&fixture)).await.unwrap();
    let mut cases = judgment::jud::Cases {
        name: "t".to_owned(),
        rubric: Some("infra-plan".to_owned()),
        ..Default::default()
    };
    for r in &prepared.requests {
        let mut case = judgment::jud::Case::new(r.state.clone());
        case.id = Some(r.id.clone());
        if r.shape == "change" {
            case.expect.insert(
                "destructive_ack".to_owned(),
                judgment::jud::Expect::Bool(false),
            );
        } else {
            case.expect.insert(
                "rollback_obvious".to_owned(),
                judgment::jud::Expect::Bool(true),
            ); // the fake says 0.3: wrong
            case.expect.insert(
                "blast_radius".to_owned(),
                judgment::jud::Expect::Text("platform".to_owned()),
            );
        }
        cases.cases.push(case);
    }
    let rubric_dir = fixture.path().join("rubric");
    std::fs::create_dir_all(&rubric_dir).unwrap();
    std::fs::write(
        rubric_dir.join("infra-plan.jud"),
        ci_judge::rubrics::file("infra-plan", "infra-plan.jud").unwrap(),
    )
    .unwrap();
    std::fs::write(rubric_dir.join("cases.jud"), cases.to_yaml().unwrap()).unwrap();
    let manifest = ci_judge::rubrics::manifest("infra-plan")
        .unwrap()
        .replace("cases: infra-plan-cases.jud", "cases: cases.jud");
    std::fs::write(rubric_dir.join("manifest.yml"), manifest).unwrap();
    let loaded = Manifest::load(rubric_dir.to_str().unwrap()).unwrap();
    let report = eval::eval(&loaded, &recordings).await.unwrap();
    assert_eq!(report.graded, 3);
    assert!(report.missing_recordings.is_empty());
    assert_eq!(report.questions["destructive_ack"].accuracy, Some(1.0));
    assert_eq!(report.questions["rollback_obvious"].accuracy, Some(0.0));
    assert_eq!(report.questions["blast_radius"].accuracy, Some(1.0));
    // Below the manifest's promotion bars (and too few labelled): exit 3.
    assert_eq!(report.exit(), Exit::Warn);
}

#[test]
fn bundled_rubrics_check_clean() {
    for name in ci_judge::rubrics::names() {
        let loaded = Manifest::load(name).unwrap();
        eval::check(&loaded).unwrap_or_else(|e| panic!("{name}: {}", e.message));
    }
}

#[test]
fn repo_local_manifests_cannot_declare_blockers() {
    let dir = tempfile::tempdir().unwrap();
    for f in ["manifest.yml", "infra-plan.jud", "infra-plan-cases.jud"] {
        std::fs::write(
            dir.path().join(f),
            ci_judge::rubrics::file("infra-plan", f).unwrap(),
        )
        .unwrap();
    }
    let mut opts = run::Options {
        rubric: dir.path().to_str().unwrap().to_owned(),
        out: dir.path().join("v.json"),
        model: None,
        replay: None,
        record: None,
        cases_out: None,
        max_items: None,
        budget_bytes: 1000,
        max_in_flight: 1,
        allow_untuned: true,
        allow_local_blockers: false,
        platform: Some("local".to_owned()),
    };
    let err = run::load(&opts).unwrap_err();
    assert!(err.message.contains("blocker"), "{}", err.message);
    opts.allow_local_blockers = true;
    run::load(&opts).unwrap();
}

#[test]
fn byte_budget_drops_lowest_priority_keys_first() {
    let manifest = Manifest::parse(
        r#"
name: t
rubric: t.jud
state:
  - {key: a, source: env, path: A, priority: 1}
  - {key: b, source: env, path: B, priority: 9}
requests:
  - {name: all}
questions: {}
"#,
    )
    .unwrap();
    let big = "x".repeat(400);
    let sources = vec![
        Collected {
            key: "a".into(),
            priority: 1,
            reduced: ci_judge::reduce::Reduced {
                value: serde_json::json!("small"),
                items: vec![],
            },
        },
        Collected {
            key: "b".into(),
            priority: 9,
            reduced: ci_judge::reduce::Reduced {
                value: serde_json::json!(big),
                items: vec![],
            },
        },
    ];
    let (requests, truncation) = requests::build(&manifest, "local", &sources, 200, None);
    assert_eq!(truncation.keys_dropped, ["b"]);
    assert!(requests[0].state.get("a").is_some());
    assert!(requests[0].state.get("b").is_none());
}
