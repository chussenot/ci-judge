---
title: Gate
description: What ci-judge gate does with a verdict file in each of the five modes, how the override label is read, what each platform receives, and the four exit statuses it shares with the GitLab block.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, gate, reference]
---

# Gate

`ci-judge gate` is the decision step: the GitLab `judge-gate` block's
logic in Rust, for GitHub workflows and anywhere a block is not
available. The same contract, the same exit statuses, the same files, so
a pipeline can swap one for the other.

## Steps

1. Read the verdict file and validate it ([Verdict file](verdict-file.md)); an invalid file is exit 2.
2. Copy it to `<out>/judge-gate.verdict.json` and write `<out>/judge-gate.env` with `JUDGE_GATE_VERDICT`.
3. In `shadow` mode, stop: exit 0, nothing posted.
4. Log the summary and one line per finding.
5. In `route` mode, copy `<routes>/<route>.yml` to `<out>/judge-gate.route.yml`, add `JUDGE_GATE_ROUTE` to the dotenv, exit 0; a missing route file is exit 2.
6. Read the override label from the labels the pipeline started with.
7. Report, per platform (below).
8. Exit by mode and verdict.

## Exit

| Verdict | `advise` | `warn` | `block` | `block`, label present |
|---|---|---|---|---|
| `pass` | 0 | 0 | 0 | 0 |
| `fail` | 0 | 3 | 1 | 3 |

2 is a gate error at any step: no valid verdict file, an unknown mode, a
missing route file.

## What each platform receives

| | GitLab | GitHub | Local |
|---|---|---|---|
| the note | one merge request note per gate, marker `<!-- judge-gate:<name> -->`, updated in place; needs `GITLAB_TOKEN` and a merge request pipeline | one pull request comment, same marker, updated in place; needs `GITHUB_TOKEN` and a `pull_request` event | `<out>/judge-gate.note.md` |
| the findings | findings with a path in `<out>/gl-code-quality-judge-gate.json`, the Code Quality report; fingerprint over `path:line:description` so a re-run shows nothing new | one `::error`, `::warning` or `::notice` workflow command per finding, by severity, attached to the file and line when the finding has them | the Code Quality report |
| the summary | | appended to `GITHUB_STEP_SUMMARY` | |
| variables for later jobs | `judge-gate.env` as a dotenv report | `verdict` and `route` in `GITHUB_OUTPUT`, plus `judge-gate.env` | `judge-gate.env` |

A note that cannot be posted (no token, no permission) is a warning in
the log, not a failure: the verdict is in the artifacts and the log
either way.

## The note

```
### ❌ Judge gate `judge:plan`: FAIL (mode `block`, [job](…))

<summary>

| Severity | Location | Finding |
|---|---|---|
| blocker | `infra/rds.tf:12` | delete of aws_db_instance.main is not acknowledged … (p=0.20, threshold 0.70) |

<details><summary>rubric infra-plan, model jev-1.14.0, 3 request(s), 2310 input tokens</summary> … answers … </details>

_This verdict fails the job. Human override: add the `judge-gate::override` label to the merge request and run the pipeline again._
```

The closing line says what to do: nothing on a pass, that nothing is
blocked in `warn`, the override in `block`, that it was overridden when
the label is there.

## The override

`--override-label`, `judge-gate::override` by default. On GitLab the
labels are `CI_MERGE_REQUEST_LABELS`, on GitHub the event's
`pull_request.labels`; both are what the pipeline saw when it started,
so the label needs a new pipeline. An overridden block exits 3 and the
note says so: the override is visible, never silent.
