---
title: Command line
description: Every ci-judge subcommand, its flags and the environment variable behind each, what it writes, and the exit status each one returns.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, cli, reference]
---

# Command line

`ci-judge <subcommand> [flags]`. Every flag of `run` has a `CI_JUDGE_*`
variable, every flag of `gate` a `JUDGE_GATE_*` one, so a pipeline sets
variables and a shell sets flags; a flag wins. Messages go to stderr as
`ci-judge: MESSAGE`; the gate's log lines as `judge-gate: MESSAGE`, its error like any other; stdout carries
results.

## `run`

Collect the evidence, ask the rubric, write the verdict file.

| Flag | Variable | Default | Meaning |
|---|---|---|---|
| `--rubric` | `CI_JUDGE_RUBRIC` | required | A bundled name (`infra-plan`, `mr-hygiene`, `failed-job-triage`), a manifest path, or a directory holding `manifest.yml` |
| `--out` | `CI_JUDGE_OUT` | `judge-gate.verdict.json` | The verdict file; the gate's `JUDGE_GATE_VERDICT_FILE` must match |
| `--model` | `CI_JUDGE_MODEL` | the jud configuration's | The model to ask; must equal the rubric's `tuning.model` when it has one |
| `--replay DIR` | `CI_JUDGE_REPLAY` | | Answer from recordings; no key, no network; a request nobody recorded is exit 1 |
| `--record DIR` | `CI_JUDGE_RECORD` | | Keep every answer as a `.jud` Recording named after its request |
| `--cases-out FILE` | `CI_JUDGE_CASES_OUT` | | Append every request as an unlabelled case to a Cases document, created if absent |
| `--max-items N` | `CI_JUDGE_MAX_ITEMS` | the manifest's | Cap on per-item requests per shape; the lower of the two applies |
| `--budget-bytes N` | `CI_JUDGE_BUDGET_BYTES` | `512000` | Cap on the global state; lowest-priority keys are dropped first, item sources last |
| `--max-in-flight N` | `CI_JUDGE_MAX_IN_FLIGHT` | `4` | Requests in flight at once |
| `--allow-untuned` | `CI_JUDGE_ALLOW_UNTUNED` | off | Run a rubric whose `tuning.model` is not the configured model |
| `--allow-local-blockers` | `CI_JUDGE_ALLOW_LOCAL_BLOCKERS` | off | Let a repo-local manifest declare `critical` or `blocker` |
| `--platform` | `CI_JUDGE_PLATFORM` | detected | `gitlab`, `github` or `local` |

Exit 0 when a verdict was written. Exit 1 when a collector that is not
optional, or a call after the crate's retries, failed. Exit 2 before any
call: no rubric, a manifest or `.jud` the reader refuses, a question the
rubric lacks, a per-item question without `when`, a local blocker, an
untuned model, no key. On 1 and 2 no verdict file is written.

## `explain`

The same flags as `run`; prints, as JSON on stdout, each request with its
shape, locator, byte size, the questions its `when` left in, and its
state. No call. Exit 0; 1 when a collector that is not optional failed; 2 before that.

## `collect`

The same flags as `run` plus `--dir` (default `ci-judge-requests`);
writes each request as `<id>.json` under it. No call. Exit 0, 1 or 2 as `explain`.

## `gate`

Turn a verdict file into a decision ([Gate](gate.md)).

| Flag | Variable | Default |
|---|---|---|
| `--mode` | `JUDGE_GATE_MODE` | `shadow` |
| `--verdict FILE` | `JUDGE_GATE_VERDICT_FILE` | `judge-gate.verdict.json` |
| `--override-label` | `JUDGE_GATE_OVERRIDE_LABEL` | `judge-gate::override` |
| `--routes DIR` | `JUDGE_GATE_ROUTES` | `.gitlab/judge-gate` |
| `--out DIR` | `JUDGE_GATE_OUT` | `.` |
| `--name` | `JUDGE_GATE_NAME` | the job's name |
| `--platform` | `CI_JUDGE_PLATFORM` | detected |

Exit 0 pass, 1 blocked, 2 gate error (no valid verdict file, unknown
mode, missing route file), 3 warning.

## `check [RUBRIC...]`

Read a rubric's manifest, `.jud` rubric, cases and recordings together;
every bundled rubric by default. Prints one line per document and a
`warning:` for a blocker that is global. Exit 0, or 2 when any is
refused.

## `eval`

| Flag | Variable | Default |
|---|---|---|
| `--rubric` | `CI_JUDGE_RUBRIC` | required |
| `--replay DIR` | `CI_JUDGE_REPLAY` | the manifest's `recordings` |
| `--json` | | the report as JSON |

Grades every labelled case from its recording. Exit 0; 1 when a case
has no recording; 3 when a question is below a promotion bar
([Shadow, label, eval, promote](../guides/shadow-and-eval.md)).

## `config`

The resolved backend as JSON: the configuration file path, base URL,
model, timeout, long context, and where the base URL and the model came from. The key is
reported as `environment`, `config file` or `missing`, never printed.

## `catalog`

The bundled rubrics and their files.

## `version`

`ci-judge version` prints `ci-judge <version>`, the same as
`ci-judge --version`, for a `before_script` that expects a subcommand.

## Exit status, in one table

| Status | `run`, `explain`, `collect` | `gate` | `check` | `eval` |
|---|---|---|---|---|
| 0 | verdict written (or printed) | pass | every document reads | every bar met |
| 1 | a collector or a call failed | blocked | | a case has no recording |
| 2 | wrong before any call | gate error | a document refused | wrong before grading |
| 3 | | warning | | a bar not met |
