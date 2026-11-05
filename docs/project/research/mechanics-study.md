---
title: "Study: the mechanics of ci-judge"
description: What the binary does today, stage by stage, what established gate tools do for the same mechanics, where the implementation is wrong or thin, which Rust crates would earn their place, and the three slices of work this ranks first.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, study, architecture, research, roadmap]
---

# Study: the mechanics of ci-judge

**Date:** 2026-11-04, against `main` at v0.1.1 (4 863 lines of Rust, 10
end-to-end tests, three bundled rubrics) and `judgment` 0.16.1.
**Question:** what should be built, borrowed or left alone so that the
mechanism (collect, reduce, shape, ask, read, write, gate) holds up in
real pipelines, without reinventing what the field already settled.
**Method:** every module and every page of this repository read; the
`judgment` crate's backends, observer and `jud` binary read for what it
already provides; the gate tools in the field and the Rust crate
landscape surveyed from their official documentation and sources
(URLs under [Sources](#sources)). Nothing below was run against a real
model.

## Summary

- **The design holds.** One request per evidence item, the item key as
  the `when` gate, no text generated, a verdict file as the seam, policy
  in two readable files: no tool in the survey does this, and the
  conventions ci-judge chose (Code Quality fingerprints, a marker
  comment updated in place, exit statuses, fail-open by default) are the
  field's.
- **Five defects would show in the first week of real use**, all small:
  the shadow loop loses recordings when a merge request is pushed twice;
  the note and comment lookups read one page; GitHub drops annotations
  past ten per level and ci-judge does not know; the comment body is
  unbounded; a `.tf` locator can point at the wrong file.
- **The `diff` source should come from the platform API before the
  checkout.** Both APIs serve the merge request's diff to the token the
  job already holds; that removes `git`, the full-history fetch and the
  scratch image's biggest gap in one move, and the same pattern gives
  GitHub its `joblog`.
- **Four crates earn their place**, two of them already linked:
  `ignore` for the walk, `anstream` for ANSI stripping, `wiremock` and
  `assert_cmd` for the tests the platform clients and the exit contract
  lack. Everything else (git libraries, platform SDKs, templating,
  config layering, secret scanners, SARIF) costs more than it returns
  today.
- **Three slices of work**, in order: correctness with no new
  dependency; evidence from the API; observability and a manifest
  schema. The last waits for a user who asks.

## 1. The mechanism as built

| Stage | Code | What it guarantees today |
|---|---|---|
| Load | `manifest.rs`, `rubrics.rs`, `run::load` | Unknown manifest fields refused; per-item questions must carry `when: <item key>`; blockers only from the bundled catalog |
| Collect | `collect/mod.rs`, `platform.rs` | Six sources; an `optional` source that fails becomes a note, any other failure ends the run with no verdict file |
| Reduce | `reduce/{tfplan,junit,diff}.rs` | Items with a path and a line where the evidence has one; `tfplan` never carries resource values, only attribute names |
| Redact | `redact.rs` | Ten patterns over every string before any call; a match becomes `[REDACTED:<kind>]` |
| Shape | `requests.rs` | One global request plus one per item; item requests carry no item source, so a global question's `when` never matches them; a byte budget drops the highest-priority-number keys first |
| Ask | `assess.rs` | The crate's `Client` behind a `Pacer`, a `Replay`, or a `Fake`; every answer verified by the crate; recordings written as `.jud` |
| Read | `verdict.rs` | `Rubric::apply` verdicts classified as fail, pass, undecided or report; a description carries no number unless the template asks |
| Write | `verdict.rs` | The gate's four fields validated the way the block's jq does; everything else under `ci_judge` |
| Gate | `gate.rs` | Five modes, the override label, a note or comment updated in place, Code Quality JSON, annotations, a dotenv, `GITHUB_OUTPUT` |
| Check, eval | `eval.rs`, `cases.rs` | The three files bind; recordings graded against labels with the crate's metrics; promotion bars from the manifest |

The invariants worth protecting through any change below: a finding's
locator comes from the request, never from the answer; a run that cannot
judge writes nothing; the `.jud` documents stay the crate's; nothing
generates prose.

## 2. What the field does for the same mechanics

The survey covered the exchange formats (SARIF, Code Quality, reviewdog's
RDFormat, GitHub annotations and the Checks API, GitLab external status
checks, HCP Terraform run tasks), the comment-per-run tools (Danger,
reviewdog, Infracost, Qodo PR-Agent, CodeRabbit), the plan-policy tools
(OPA and Conftest, Checkov, Sentinel, Trivy), the suppression and
baseline conventions (Semgrep, gitleaks, detect-secrets, SonarQube), the
secret scanners and the LLM reviewers. What matters for ci-judge:

| Observation | Consequence for ci-judge |
|---|---|
| GitHub's workflow-command annotations are capped at 10 errors and 10 warnings per step (Checks documentation), 10 notices per step and 50 per job (actions/toolkit); the excess is dropped silently. The Checks API takes 50 per request but only a GitHub App may create a check run, not `GITHUB_TOKEN`. [1] [2] [3] | The gate must truncate by severity itself and say so; a check run is not reachable without shipping an App |
| GitLab reads six Code Quality fields (`description`, `check_name`, `fingerprint`, `location.path`, `location.lines.begin`, `severity`) and collapses identical fingerprints; new and resolved findings are a fingerprint diff against the target branch's report. [4] | The "no number in a description" rule is exactly right; a per-finding fingerprint field would change nothing GitLab reads |
| GitHub code scanning dedups SARIF on `partialFingerprints.primaryLocationLineHash` plus a stable `ruleId` and path. [5] | SARIF from the verdict file is a 40-line writer when a user asks; the identity rule is already compatible |
| The one-comment-per-run marker is a settled convention: Danger's `DangerID: danger-id-<id>;`, reviewdog's `<!-- __reviewdog__:<base64> -->`, Infracost's `--tag`, all matched by body substring; Infracost's behaviours are `update`, `new`, `hide-and-new`, `delete-and-new`, and it truncates with a 1 000-character margin, verbose section first. GitHub's comment cap is 65 536 characters (API error only); GitLab's note cap is 1 000 000 (documented). [6] [7] [8] | ci-judge's marker is the convention; its body needs Infracost's truncation |
| reviewdog's `-filter-mode` (`added`, `diff_context`, `file`, `nofilter`) is the vocabulary for "in the diff", and out-of-diff findings fall back to a non-inline channel. [9] | A `tfplan` finding often points at a `.tf` line the merge request did not touch; it belongs in the summary and the Code Quality report, not only in an inline annotation |
| No Terraform plan-policy tool (OPA, Conftest, Sentinel `tfplan/v2`, Trivy) maps a plan resource to a file and a line; Checkov alone does, with `--repo-root-for-plan-enrichment` and the HCL beside the plan. [10] [11] | ci-judge's locator is the unusual part and the valuable one; it must be right, and it must have a non-blocking fallback when the source tree is absent |
| Exit statuses: Conftest's `--fail-on-warn` ladder is 0 ok, 1 warn, 2 fail; Semgrep reserves 2 for "the tool failed"; TruffleHog uses 183 to avoid collisions. [12] [13] | ci-judge's 0/1/2/3 puts warning above gate error; the README and the gate page say so, and should keep saying so |
| On GitHub a skipped job satisfies a required check and `neutral` counts as passing; on GitLab an external status check fails after two minutes pending. [14] [15] | Fail-closed on GitHub holds only while the gate job itself runs and exits non-zero, which the action's `fail-on: closed` does |
| Sentinel's `advisory`, `soft-mandatory` (overridable by a named group) and `hard-mandatory` map onto `advise`, `warn`, `block` with the label, `block`. Suppression in the field is `tool:skip=<rule>:<reason>`, with an expiry in Trivy's form. [16] [17] | The mode ladder needs no renaming; inline suppression is not needed while the override label is per merge request and auditable |
| gitleaks ships its rules as MIT TOML (`id`, `regex`, `keywords` as a prefilter, `entropy`, `allowlists`); no maintained Rust library consumes them, but the rules themselves are copyable. [18] | Keep the hand-written table, refresh it from gitleaks' catalog and cite the revision |
| No LLM reviewer in CI claims calibration; PR-Agent clips large patches, CodeRabbit runs a check whose failure does not block by default, Copilot and Duo comment only. [19] [20] | The calibration ladder (shadow, label, eval, promote) is the differentiator; nothing in the field competes on it |

## 3. Findings in the implementation

Ranked by what a real pipeline would hit first. "Fix" is the size of the
smallest complete change, callers and tests included.

| # | Finding | Where | Effect | Fix |
|---|---|---|---|---|
| F1 | A recording is written as `<request id>.jud` and overwritten on the next run; a case with the same id is kept as it was. A merge request pushed twice with persisted artifacts ends with the case holding the first state and the recording the last, so `eval` reports "no recording" for it. The crate's own `Recorder` names files by request hash and has no such collision. | `assess.rs` `record`, `cases.rs` `append` | The shadow loop loses the labelled case it was built to produce | Name the recording `<id>-<first 8 of the fingerprint>.jud`; when a case with the id exists and its fingerprint differs, replace its state and drop its labels, with a note. ~20 lines, one test |
| F2 | The note and comment lookups read one page of 100 and no `Link` or `x-next-page` header. On a merge request with more than 100 notes the marker is never found and every pipeline posts a new note. | `platform.rs` `upsert_note`, `upsert_comment` | Duplicate notes on exactly the merge requests that matter | Page until an empty page, or sort newest first and stop at the marker. ~15 lines, a wiremock test each |
| F3 | One `::error` or `::warning` per finding; GitHub keeps ten per level per step and says nothing. `infra-plan` allows 25 items. | `gate.rs` `annotations` | Findings vanish from the Files tab; the comment still has them | Emit at most ten per level, worst first, and add "N more in the comment" to the step summary. ~15 lines, one test |
| F4 | The comment embeds every `ci_judge.answers` entry in a `<details>` block with no bound. | `gate.rs` `render_markdown` | Past 65 536 characters GitHub refuses the body; the gate logs a warning and the reviewers see nothing | Infracost's rule: a fixed margin, the `<details>` block cut first, a line saying it was cut and where the artifact is. ~20 lines, one test |
| F5 | `locate` returns the first `.tf` line whose text starts with `resource "<type>" "<name>"`, in any directory, `module_address` ignored, `.terraform` skipped. Two modules declaring the same type and name, or a commented-out block, give a wrong file and line. | `reduce/tfplan.rs` `locate` | A blocker on the wrong line, and a fingerprint that moves when the wrong file changes | Read `.terraform/modules/modules.json` (`Key` to `Dir`) and scope the scan by `module_address`; skip lines starting with `#` or `//`. ~40 lines, two tests. `hcl-edit` for exact spans only when a commented or heredoc'd match is observed |
| F6 | The `diff` source spawns `git`, needs the merge base in the checkout (`GIT_DEPTH: 0`, `fetch-depth: 0`) and falls back to `HEAD~1` with no base; the scratch image has no `git`. | `collect/mod.rs` `diff` | The most common source is the one that fails most often in a fresh job | Pattern P1 below: the platform API first |
| F7 | One failed request after the crate's retries fails the whole run; the other spawned tasks run on until exit; a job retry re-asks every request. | `assess.rs` `answer_all` | Correct (fail closed) but expensive: a transient failure on 1 of 25 costs 25 calls again | Pattern P2 below: recordings as the retry cache. Not a change to the fail-closed rule |
| F8 | Failed job traces are listed on one page, downloaded whole into memory one after another, then tailed. | `platform.rs` `failed_jobs` | A pipeline with large traces makes the judge slow and large before it asks anything | Page the job list; stream the trace and keep the tail; fetch traces concurrently under the pacer's bound. ~40 lines |
| F9 | `safe_key` truncates at 80 characters and maps `/` to `_`: two long module addresses, or `a/b.rs` and `a_b.rs`, share a request id, so one recording and one case overwrite the other. | `reduce/mod.rs` `safe_key`, `requests.rs` | Rare, silent | Append six hex characters of a hash of the original when the key was altered. ~8 lines, one test |
| F10 | The granularity page says the lowest-priority items are dropped past the cap; the code keeps the first N in reducer order, and items have no priority. | `requests.rs` `build`, `docs/concepts/granularity.md` | A plan with 30 deletions judges the first 25 in plan order | Order items by the position of their matched `filter` value (`delete` before `replace`), then fix the page. ~10 lines, or the page alone |
| F11 | `ci-judge check` says it checks recordings; `eval::check` never opens them. | `main.rs`, `eval.rs` | A help line that is not true | Open `Replay` over the recordings directory and print the count, or correct the help. ~10 lines |
| F12 | The replay description prints "(0 recordings)" with a literal zero; `Replay::len` exists. | `assess.rs` `replay` | A log line that is wrong | One line |
| F13 | Truncation records counts and keys, never which items were dropped. | `requests.rs`, `verdict.rs` | A reviewer cannot tell which deletion was not judged | A note listing the dropped item keys. ~6 lines |
| F14 | The byte budget bounds the global state only; item requests are one item plus the non-item sources and are never measured; the crate refuses a request over its token limits as `max_tokens_exceeded`, which ends the run. | `requests.rs`, `assess.rs` | A large `runbook` plus a large hunk fails every item request, after the global one passed | `explain` already prints bytes per request; warn there above a byte heuristic, and let `max_items` and `priority` do the rest. ~10 lines |
| F15 | A JUnit case without `file` and `line` attributes (JUnit Java, Jest, Go) yields a finding with no path, which never reaches the Code Quality report or an annotation. | `reduce/junit.rs` | The `failed-job-triage` rubric is advisory, so this is a limit, not a defect | Documented; a `locate` glob from `classname` to a file is a later manifest field, if asked |
| F16 | The platform clients have no tests beyond one hand-written `TcpListener` server for one call; the note and comment upserts, the job listing and the traces are untested. | `tests/end_to_end.rs`, `platform.rs` | F2 and F8 are the kind of defect this leaves in | `wiremock` as a dev dependency, as the sibling crate; one test per endpoint |

Two observations that are not defects. The exit status ordering (3 for a
warning above 2 for a gate error) is unusual next to Conftest's ladder
and is already documented on every page that lists statuses; keep
saying it. And the threat model: the description, the diff and the
traces are the author's text, and a System One model reads them. It
cannot be made to generate, but a question whose criteria accept "the
description acknowledges the deletion" without naming the resource can
be satisfied by prose written for the judge. The bundled criteria name
the resource; a rubric reviewer should hold every blocker's criteria to
that standard, and the override label stays the one audited bypass.

## 4. Patterns worth adopting

Each pattern names what it replaces, what in the field it follows, and
what it costs. None needs a new crate.

### P1. Evidence from the platform before the checkout

The `diff` source runs `git`. Both platforms serve the merge request's
diff to the token the job already holds for the note:

- GitLab: `GET /projects/:id/merge_requests/:iid/diffs?unidiff=true`,
  paginated, one entry per file with `old_path`, `new_path`, `diff`,
  `new_file`, `renamed_file`, `deleted_file`, and since 18.4 `collapsed`
  and `too_large` flags for files the instance's diff limits cut. The
  merge request's `diff_refs` carries `base_sha` and `head_sha`. [21]
- GitHub: `GET /repos/{o}/{r}/pulls/{n}` with
  `Accept: application/vnd.github.diff` returns the whole unified diff
  (406 when the diff is too large), and `/pulls/{n}/files` returns one
  entry per file with a `patch`, up to 3 000 files, 100 per page. [22]

The reducer already reads unified text, so the collector is the only
change: `git` when it is present and the base is known, the API when it
is not, an error that names both otherwise. It removes `GIT_DEPTH: 0`
and `fetch-depth: 0` from the guides, makes the scratch image able to
run `mr-hygiene`, and gives a diff that matches what the reviewer sees.
The `too_large` and `collapsed` flags become `truncated` on the item, as
the hunk cap already does. Cost: ~80 lines in `platform.rs` and
`collect/mod.rs`, two wiremock tests, two guide edits.

The same pattern gives GitHub its `joblog`, which the README lists as
GitLab-only: `GET /repos/{o}/{r}/actions/runs/{run_id}/jobs?filter=latest`
lists the run's jobs with a `conclusion` per job and per step;
`GET /repos/{o}/{r}/actions/jobs/{id}/logs` answers 302 with a one-minute
URL, which `reqwest` follows. [23] It needs `actions: read` on the token
and a judge job that runs after the jobs it reads (`needs:` and
`if: failure()`), as the GitLab guide already asks. Cost: ~60 lines, one
test, a guide section.

### P2. Recordings as the retry cache

`evidence_hash` is written "for the audit trail and the cache", and
there is no cache. The crate's `Replay` already matches a recording by
the fingerprint of the state and the questions, and refuses a state it
has not seen. A backend that tries the replay directory and calls the
client on `NoRecording`, writing the new recording as it goes, is ~30
lines over two backends the crate already provides. With
`--record DIR --replay DIR` on the same directory kept as a job
artifact, a retried job re-asks only the requests that failed, and a
merge request pushed again re-asks only the items whose state changed.
This changes no fail-closed rule: a request that fails still fails the
run. It is the one cost mechanism the design lacks, and it uses the
recordings the shadow loop produces anyway.

### P3. Stable identities across pushes

F1 and F9 are one rule: a request id names an item, a fingerprint
names its state, and a file on disk needs both. The crate keys
recordings by fingerprint and tolerates any file name, and `jud eval`
warns on two files with the same fingerprint, so `<id>-<fp8>.jud` is
safe for every reader. The cases document keeps one case per id and
refreshes its state when the fingerprint moves, because a label belongs
to a state, not to an id.

### P4. Bounded reports

F2, F3, F4 and F8 share a shape: an output or an input with a limit the
code does not know. The field's answer is the same each time: page the
reads (notes, comments, jobs); truncate the writes by severity, worst
first, and say in the remaining text what was cut and where the whole
is (the verdict artifact). The annotation rule follows reviewdog: an
inline channel takes what fits, the summary takes everything. Nothing
here touches the verdict file's contract, so no decision record is
needed; the gate page gains a "Limits" section.

### P5. The run's numbers as a report

The verdict already carries `elapsed_ms`, `input_tokens`, `cost`,
`requests` and the finding counts. GitLab's metrics report
(`artifacts: reports: metrics`, one `<name> <value>` line per metric,
Premium and Ultimate) shows them in the merge request widget as new,
changed and unchanged against the target branch, with no dependency and
no collector. [24] `ci-judge gate` writing `judge-gate.metrics.txt` is
~15 lines; the GitHub step summary already carries the same numbers.

OpenTelemetry is the README's "next". The crate has the `otel` feature
(`OtelObserver`, the `gen_ai.*` names in `judgment::semconv`) and the
`jud` binary has the exact pattern a short-lived process needs: start
only when `OTEL_EXPORTER_OTLP_ENDPOINT` is set, OTLP over HTTP with
protobuf, a three-second export timeout so a dead collector delays the
exit by seconds, and a guard whose `Drop` flushes on every exit status.
Copying it is ~80 lines behind a non-default feature; the measured cost
is 28 crates beyond the current graph, sharing `reqwest` 0.13. One root
span per run (rubric, platform, verdict, mode) over the crate's one span
per call is the right shape. It waits for a user with a collector: it
replaces nothing.

### P6. A schema for the manifest

`manifest.yml` is refused on an unknown field at run time and nowhere
earlier. The crate publishes JSON Schemas for the `.jud` documents from
`schemars` under a feature, with a test that fails when a type and the
schema drift. The same pattern over `Manifest` gives editors validation
and the reference page a source of truth; the derive honours
`deny_unknown_fields`, `rename_all` and doc comments. ~30 lines, one
generated file, one test, behind a feature so the binary does not carry
it.

### P7. Secret rules from a catalog, not a table

The ten patterns in `redact.rs` are hand-written and untested against a
corpus. gitleaks' catalog is MIT TOML with an `id`, a regex, `keywords`
as a prefilter and `allowlists`; no maintained Rust library consumes it
(the candidates are GPL, a month old, unpublished or dead), and its
regexes are RE2, which the `regex` crate reads for the common rules. The
right size is not a crate: refresh the table from the catalog's rules
for the providers a CI job sees (cloud keys, platform tokens, private
keys, JWTs, connection strings), keep gitleaks' ids as the `kind`, and
cite the catalog revision in a comment. ~40 lines, a test per rule from
gitleaks' own examples.

## 5. Rust crates: what earns its place

Measured by the survey with `cargo tree` on the current resolver;
"beyond the graph" counts crates not already in `Cargo.lock`.

| Crate | Replaces | Cost | Verdict |
|---|---|---|---|
| `anstream::adapter::strip_str` (1.0.0, already linked through clap) | the ANSI regex in `platform.rs` | 0 crates | **Adopt.** A VT parser drops the OSC and CSI forms a regex misses, GitLab's `section_start…\r\x1b[0K` among them |
| `ignore` 0.4.34 (ripgrep's walker; `globset` and `walkdir` already linked) | the hand-rolled `read_dir` recursion in `collect/mod.rs` | 6 crates | **Adopt.** `.gitignore` honoured without `git`, vendored and generated trees never judged. Set `hidden(false)` and exclude `.git` by name: `.github/` and `.gitlab-ci.yml` are hidden paths |
| `wiremock` 0.6.5 (dev; the sibling crate's choice) | the hand-written `TcpListener` server | 14 crates, dev only | **Adopt.** The only way `platform.rs` gets tests |
| `assert_cmd` 2 + `predicates` 3 (dev) | nothing: the 0/1/2/3 contract has no binary-level test | 14 crates, dev only | **Adopt.** Tests what the README promises |
| `schemars` 1.2.3 behind a feature | nothing: adds the manifest schema (P6) | 17 crates, mostly shared | **Adopt when P6 is done** |
| `diffy` 0.5.2 (`PatchSet` with git extended headers, `no_std`) | `reduce/diff.rs` | 5 crates | **Later**, on the first rename, binary or mode-only defect; the 90 lines are tested and in use |
| `hcl-edit` 0.10.0 (byte spans per block, pure Rust) | the text scan in `tfplan.rs` | 9 crates | **Later**, on the first wrong line a comment or heredoc causes; F5's scoping comes first |
| `insta` (dev) | hand-written assertions on the note and the Code Quality JSON | 12 crates, dev only | **Later**, once those outputs stop moving |
| `opentelemetry` 0.33 + `opentelemetry-otlp` + `tracing-opentelemetry` 0.34 | nothing (P5) | 28 crates, behind a feature | **Later**, for a user with a collector |
| `shellexpand` 3 (env-only function) | the 30-line `${VAR}` expander | 1 crate | **No**: marginal |
| `serde_json::Value::pointer` | the dotted-path lookup | 0 crates | **No** before 1.0: `/a/b` for `a.b` is a manifest syntax break for no present bug |
| `gix` 0.90 (`blob-diff`, `revision`) | the `git` spawn | 88 crates beyond the graph, unmeasured musl size, still no merge base in a shallow checkout | **No**: P1 removes the need |
| `git2` | the same | C | **No** |
| `octocrab` 0.54, `gitlab` 0.1905 | the eight raw calls in `platform.rs` | 36 and 48 crates, a second HTTP stack | **No**: the gap is tests, not code |
| `junit-parser`, `quick-junit` | `reduce/junit.rs` | a parser swap | **No** |
| `keyspoor`, `keyhunter`, `rustleaks`, `rusty_hogs` | `redact.rs` | new, GPL, unpublished, dead | **No**: P7 copies rules instead |
| `minijinja`, `handlebars`, `tinytemplate`, `leon` | the 30-line renderer | 1 to many crates | **No**: filters and conditionals invite prose into a finding, which the design forbids |
| `figment`, `config`, `confique` | the three-layer resolution in `config.rs` | 13 or more crates, serde_yaml 0.9 (archived) in two of them | **No** |
| `jsonschema` | serde's `deny_unknown_fields` | 40 crates | **No** |
| `serde-sarif` 0.8, `rdfmt`, `actions-rs` | nothing today | new outputs | **No** until a user asks for code-scanning upload; a SARIF writer from the verdict file is 40 lines of `serde_json` |
| `ci_info`, `is_ci` | `Platform::detect` | a bool and a vendor name | **No**: none of the merge request variables |
| `cargo-dist` 0.34 (alive, released 2026-11-01) | `release.yml` | owns the workflow | **No**: the workflow also pushes the image |

## 6. Deliberately left out

- **A GitHub check run.** `GITHUB_TOKEN` cannot create one; it needs an
  App. Annotations, the step summary and the exit status are the
  channels, bounded as P4 says.
- **Inline suppression comments.** The override label is per merge
  request, visible in the note and the artifact; a `# judge:skip` in a
  `.tf` file would be a silent bypass of a blocker.
- **More platforms.** `--platform local` with `CI_JUDGE_TITLE`,
  `CI_JUDGE_DESCRIPTION` and `CI_JUDGE_BASE` is the escape hatch;
  Jenkins or Buildkite users pass artifacts.
- **Partial verdicts.** A run with one failed request writes nothing;
  P2 makes that cheap to retry instead of making it lenient.
- **Glob or regex item filters.** Equality is enough for the three
  rubrics; `globset` is already a dependency the day `type: [aws_db_*]`
  is asked for.
- **A template language.** A finding is a sentence from a template and
  the item; the gates fingerprint it.
- **Images as evidence.** The crate supports them; no bundled rubric
  needs a screenshot yet.

## 7. Roadmap

| Slice | Content | New dependencies | Checks it needs |
|---|---|---|---|
| **1. Correctness** (first) | F1, F2, F3, F4, F9, F11, F12, F13; F10 as a page fix; P3 and P4 as the rules behind them | `wiremock`, `assert_cmd`, `predicates` (dev) | One wiremock test per platform endpoint; an `assert_cmd` test per exit status; the end-to-end test extended for a second push of the same request |
| **2. Evidence** | P1 (`diff` from the API, `joblog` on GitHub); F5 (module-scoped location); F8 (paged, streamed traces); `ignore` for the walk; `anstream` for stripping | `ignore` | Wiremock tests on the diff endpoints and the jobs endpoints; a fixture with two modules declaring one resource name; the guides lose `GIT_DEPTH: 0` and `fetch-depth: 0` |
| **3. Observability and schema** (on request) | P5 (metrics report now; OTel behind a feature when a user has a collector); P6 (`schemars` feature, generated schema, drift test); P2 (replay-or-call); P7 (rules from gitleaks' catalog) | `schemars`; the OTel set behind a feature | The schema drift test; a replay-or-call test over `Fake` and a temporary directory; a redaction test per rule from gitleaks' examples |

Slice 1 is a day of work and changes no contract: no decision record.
Slice 2's API diff changes what a guide asks of a job, not the verdict
file: a changelog entry and the guide pages. Slice 3's OTel feature and
the metrics file add outputs the gate did not have; the gate page and
the configuration page carry them, and a decision record is warranted
only if the metrics file joins the block's artifact list.

## 8. Decision criteria

- A change that fixes a defect a pipeline would hit (slice 1) needs no
  further justification.
- A pattern (P1 to P7) is adopted when it removes a requirement from a
  guide, a dependency from a job, or a cost from a run, and never only
  because the field has it.
- A crate is adopted when it deletes code of real risk (a parser, a
  walker, an HTTP fake) for fewer than ten crates beyond the graph and
  no C; a dev dependency is held to the sibling crate's choices.
- Anything that would make the judge lenient (partial verdicts,
  suppression comments, a template language) is refused until the
  calibration loop shows it is needed, and then it is a decision record.

## Sources

1. GitHub, "Workflow commands for GitHub Actions": <https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-commands>
2. GitHub REST, "Check runs" (annotation limits, App-only creation): <https://docs.github.com/en/rest/checks/runs>
3. actions/toolkit, "Problem matchers" (per-step and per-job annotation limits): <https://github.com/actions/toolkit/blob/457303960f03375db6f033e214b9f90d79c3fe5c/docs/problem-matchers.md>
4. GitLab, "Code Quality": <https://docs.gitlab.com/ci/testing/code_quality/>
5. GitHub, "SARIF support for code scanning": <https://docs.github.com/en/code-security/code-scanning/integrating-with-code-scanning/sarif-support-for-code-scanning>
6. danger-js, the comment template and `getDangerCommentIDs`: <https://github.com/danger/danger-js/blob/main/source/runner/templates/githubIssueTemplate.ts>
7. reviewdog, comment metadata: <https://github.com/reviewdog/reviewdog/blob/master/service/serviceutil/comment.go>
8. Infracost, comment behaviours and truncation: <https://github.com/infracost/infracost/blob/master/cmd/infracost/comment_github.go>, <https://github.com/infracost/infracost/blob/master/internal/output/markdown.go>; GitLab notes API limit: <https://docs.gitlab.com/api/notes/>
9. reviewdog, `-filter-mode` and `-fail-level`: <https://github.com/reviewdog/reviewdog>
10. Checkov, "Terraform plan scanning" (`--repo-root-for-plan-enrichment`): <https://www.checkov.io/7.Scan%20Examples/Terraform%20Plan%20Scanning.html>
11. HCP Terraform, the `tfplan/v2` import: <https://developer.hashicorp.com/terraform/cloud-docs/policy-enforcement/import-reference/tfplan-v2>
12. Conftest options (`--fail-on-warn`): <https://www.conftest.dev/options/>
13. Semgrep CLI reference (exit statuses): <https://docs.semgrep.dev/cli-reference>
14. GitHub, "About status checks" and "About protected branches": <https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/collaborating-on-repositories-with-code-quality-features/about-status-checks>
15. GitLab, "External status checks": <https://docs.gitlab.com/user/project/merge_requests/status_checks/>
16. HCP Terraform, policy enforcement levels: <https://developer.hashicorp.com/terraform/cloud-docs/policy-enforcement/manage-policy-sets>
17. Trivy, misconfiguration ignore comments: <https://trivy.dev/docs/latest/scanner/misconfiguration/config/config/>
18. gitleaks, configuration and rule format: <https://github.com/gitleaks/gitleaks>
19. Qodo PR-Agent configuration: <https://github.com/qodo-ai/pr-agent/blob/main/pr_agent/settings/configuration.toml>
20. CodeRabbit configuration reference: <https://docs.coderabbit.ai/reference/configuration>
21. GitLab merge requests API, "List merge request diffs" and `diff_refs`: <https://docs.gitlab.com/api/merge_requests/>
22. GitHub REST, pull requests (diff media type, "List pull requests files"): <https://docs.github.com/en/rest/pulls/pulls>
23. GitHub REST, workflow jobs (list jobs for a run, download job logs): <https://docs.github.com/en/rest/actions/workflow-jobs>
24. GitLab, "Metrics reports": <https://docs.gitlab.com/ci/testing/metrics_reports/>
25. GitLab, predefined variables (`CI_MERGE_REQUEST_DESCRIPTION` at 2 700 characters): <https://docs.gitlab.com/ci/variables/predefined_variables/>
26. Crate facts: crates.io and docs.rs pages of `ignore`, `anstream`, `wiremock`, `assert_cmd`, `schemars`, `diffy`, `hcl-edit`, `gix`, `octocrab`, `gitlab`, `opentelemetry-otlp`; counts measured with `cargo tree` on 2026-11-04.
