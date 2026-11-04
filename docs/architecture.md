# Architecture

The C4 model of ci-judge, from the systems around it down to the modules
inside it. Every box names a file or a crate so the diagram can be checked
against the code; when they disagree, the code wins and this page is
wrong. Rendered by GitHub and by any Mermaid viewer.

## Level 1: system context

A merge request produces evidence. ci-judge turns that evidence into a
verdict file; a gate turns the file into a pipeline decision; a System One
backend is the only place a model runs.

```mermaid
C4Context
  title ci-judge in its context

  Person(engineer, "Engineer", "Opens a merge request and reads the gate's findings")
  Person(reviewer, "Reviewer", "Labels shadow-mode cases so a rubric can be tuned")

  System(cijudge, "ci-judge", "Collects the evidence of a change, asks a .jud rubric, writes judge-gate.verdict.json")
  System(gate, "Gate", "A GitLab judge-gate block, or ci-judge gate and action.yml on GitHub: mode, override label, exit 0/1/2/3")

  System_Ext(ci, "CI platform", "GitLab CI or GitHub Actions: the pipeline, the artifacts, the MR or PR API")
  System_Ext(backend, "System One backend", "TypeSafe Jev, or any server speaking POST /v1/systemone (a local model behind transformers serve or Ollama)")
  System_Ext(jud, "jud", "The judgment crate's command line: check, eval, tune the same .jud files")

  Rel(engineer, ci, "pushes a change, reads notes and annotations")
  Rel(ci, cijudge, "runs in a job with the artifacts and CI_* / GITHUB_* variables")
  Rel(cijudge, ci, "reads the diff, the MR fields, failed job traces", "git, REST")
  Rel(cijudge, backend, "one request per evidence item plus one global request", "HTTPS, judgment crate")
  Rel(cijudge, gate, "judge-gate.verdict.json")
  Rel(gate, ci, "note or comment, Code Quality, annotations, dotenv, route, exit status")
  Rel(reviewer, jud, "labels cases, tunes bars from recordings")
  Rel(jud, cijudge, "rubric, cases and recordings, shared files")
```

## Level 2: containers

One static binary does the judging on both platforms. The decision is a
block on GitLab and a subcommand of the same binary on GitHub. Files on
disk are the seams: the verdict file between judging and deciding, the
`.jud` documents between this crate and `jud`.

```mermaid
C4Container
  title The CI job that judges a change

  Person(engineer, "Engineer")

  System_Boundary(job, "CI job (GitLab runner or GitHub Actions runner)") {
    Container(bin, "ci-judge", "Rust, static musl binary, also ghcr.io/chussenot/ci-judge", "run: collect, redact, shape requests, ask, write the verdict. gate: decide on GitHub or without a block")
    Container(block, "judge-gate block", "GitLab CI hidden job, shell and jq", "Validates the verdict file, applies JUDGE_GATE_MODE, posts the note, writes the Code Quality report and the dotenv")
    ContainerDb(verdict, "judge-gate.verdict.json", "JSON file", "verdict, summary, route, findings; everything else under ci_judge")
    ContainerDb(rubric, "rubric directory", ".jud Rubric and Cases, manifest.yml, recordings/", "Bundled in the binary (rubrics/) or in the repository under review")
    ContainerDb(evidence, "evidence", "tfplan.json, junit*.xml, the checkout", "Artifacts of earlier jobs and the working tree")
  }

  System_Ext(backend, "System One backend", "POST /v1/systemone")
  System_Ext(api, "GitLab or GitHub API", "MR or PR fields, job traces, notes and comments")
  System_Ext(ui, "Merge request UI", "Note or comment, Code Quality widget, annotations, job state")

  Rel(evidence, bin, "artifact, diff, file sources")
  Rel(api, bin, "mr, joblog sources", "REST, job token or GITLAB_TOKEN / GITHUB_TOKEN")
  Rel(rubric, bin, "manifest.yml binds questions to evidence; the .jud is lowered to questions")
  Rel(bin, backend, "Questions + state, one call per request", "judgment::Client, paced and retried")
  Rel(bin, verdict, "writes; none when it cannot judge")
  Rel(bin, rubric, "shadow mode: recordings/ and the cases document")
  Rel(verdict, block, "reads and validates (jq)")
  Rel(verdict, bin, "ci-judge gate reads and validates (verdict::validate_loose)")
  Rel(block, ui, "note, gl-code-quality-judge-gate.json, judge-gate.env, exit 0/1/2/3")
  Rel(bin, ui, "gate: comment, ::error annotations, step summary, GITHUB_OUTPUT, exit 0/1/2/3")
  Rel(ui, engineer, "")
```

## Level 3: components of the binary

`src/lib.rs` lists the modules; `src/main.rs` is the clap surface over
them. `run` is a straight line through the first row; `gate`, `check`
and `eval` are side doors into the same modules.

```mermaid
C4Component
  title Modules of ci-judge (src/)

  Container_Boundary(bin, "ci-judge") {
    Component(main, "main.rs", "clap", "run, collect, explain, gate, check, eval, config, catalog; CI_JUDGE_* and JUDGE_GATE_* bindings")
    Component(config, "config.rs", "", "Backend settings as the jud command reads them: env, ~/.config/jud/config.yaml, defaults; ${VAR} expansion")
    Component(platform, "platform.rs", "reqwest", "GitLab, GitHub or local from the environment; the change under review in one shape; failed job traces; note and comment upserts")
    Component(manifest, "manifest.rs + rubrics.rs", "serde-saphyr", "The manifest beside a rubric: sources, request shapes, severities, fails_when, finding templates, promotion; the bundled catalog")
    Component(collect, "collect/", "", "artifact, diff, mr, file, env, joblog: one source to a reduced value")
    Component(reduce, "reduce/", "roxmltree", "tfplan, junit, diffhunks, json, text, lines: raw output to a value and items with a path and a line")
    Component(redact, "redact.rs", "regex", "Secret patterns over every string before any network call")
    Component(requests, "requests.rs", "", "One global request plus one request per item; byte budget; item cap; template rendering")
    Component(assess, "assess.rs", "judgment crate", "Client (paced), Replay, Recorder, any backend for tests; Rubric::lower per request; .jud recordings")
    Component(verdict, "verdict.rs", "sha2", "Rubric::apply verdicts to findings and the verdict file; the gate's validation in Rust; evidence hash")
    Component(gate, "gate.rs", "", "Modes, override label, note or comment, annotations, Code Quality, dotenv, route; exit 0/1/2/3")
    Component(eval, "eval.rs + cases.rs", "judgment::eval", "check: documents read together; eval: Replay, grade, accuracy, Brier, ECE, promotion bars; cases: shadow-mode writer")
    Component(run, "run.rs", "", "load, prepare, run, explain: the pipeline that wires the above")
    Component(error, "error.rs", "", "Exit 0/1/2/3 and Failure")
  }

  System_Ext(judgment, "judgment crate", "jud::parse, Rubric::lower, Rubric::apply, SystemOne, Replay, Recorder, eval")
  System_Ext(backend, "System One backend")
  System_Ext(api, "GitLab / GitHub API")

  Rel(main, run, "run, collect, explain")
  Rel(main, gate, "gate")
  Rel(main, eval, "check, eval")
  Rel(run, manifest, "Manifest::load, rubric text")
  Rel(run, platform, "Platform::detect, change, slug")
  Rel(run, collect, "one call per state source")
  Rel(collect, reduce, "reduce::apply")
  Rel(collect, platform, "diff base sha, failed jobs")
  Rel(run, redact, "every collected value")
  Rel(run, requests, "requests::build")
  Rel(run, assess, "Assessor::client or replay, answer_all")
  Rel(assess, config, "Backend::resolve, client")
  Rel(assess, judgment, "lower, answer, recording_to_yaml")
  Rel(judgment, backend, "POST /v1/systemone")
  Rel(run, verdict, "verdict::build, write")
  Rel(verdict, judgment, "Rubric::apply")
  Rel(gate, verdict, "read_loose")
  Rel(gate, platform, "labels, upsert_note / upsert_comment")
  Rel(platform, api, "REST")
  Rel(eval, judgment, "Replay, grade, QuestionMetrics, threshold_sweep")
  Rel(run, eval, "cases::append in shadow mode")
```

## Dynamic: one `ci-judge run`

```mermaid
sequenceDiagram
  autonumber
  participant J as ci-judge run
  participant M as manifest + rubric
  participant C as collect / reduce / redact
  participant R as requests
  participant B as backend (judgment)
  participant V as verdict
  participant G as gate

  J->>M: Manifest::load, jud::parse, refuse a local blocker or an untuned model
  loop every state source
    J->>C: collect(spec) → reduce → redact
    C-->>J: value + items (path, line)
  end
  J->>R: build(global + one per item, budget, cap)
  R-->>J: requests, truncation
  loop every request, bounded by the pacer
    J->>B: Rubric::lower(state) → answer(state, model, questions)
    B-->>J: Response (verified against the questions)
    opt --record
      J->>J: write <id>.jud Recording
    end
  end
  J->>V: build: Rubric::apply per response, fails_when, undecided margin, fail_on
  V-->>J: judge-gate.verdict.json (or nothing, exit 1/2)
  G->>V: read_loose (the jq validation)
  G-->>G: mode → exit 0 / 1 / 2 / 3, note or comment, Code Quality, dotenv, route
```

## Where to read next

| Question | File |
|---|---|
| What a manifest may say | `src/manifest.rs` |
| What a request looks like and why item requests carry no item source | `src/requests.rs` |
| What the verdict file holds and what the gate validates | `src/verdict.rs` |
| What each mode does on each platform | `src/gate.rs` |
| How a rubric is written and promoted | `.claude/skills/rubric/SKILL.md`, `rubrics/infra-plan/` |
| How the image is built | `Dockerfile`, `scripts/image-context.sh`, `CONTRIBUTING.md` |
