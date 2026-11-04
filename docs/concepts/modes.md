---
title: Modes and promotion
description: Why a rubric starts in shadow mode, what each of the five gate modes does with a verdict, and the numbers that let a rubric move up.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, concepts, gate, promotion]
---

# Modes and promotion

The judge is a measuring instrument. It earns a blocking role the way a
test earns trust: by not being flaky. The gate runs the same verdict in
every mode; only what happens with it changes, so a rubric moves up a
level without anyone rewriting a pipeline.

| Mode | What the gate does | Exit on `fail` |
|---|---|---|
| `shadow` | Stores the verdict as an artifact, posts nothing | 0 |
| `advise` | One note or comment updated in place, a Code Quality report or annotations | 0 |
| `warn` | Same, the job shows "passed with warnings" | 3 |
| `block` | Same, the job fails; the override label turns it into a warning | 1 (3 overridden) |
| `route` | Copies a route pipeline named after the verdict's `route`; nothing fails | 0 |

## The ladder

| Level | Leaves when |
|---|---|
| `shadow` | enough recordings and labelled cases exist for `ci-judge eval` to mean something (the manifest's `labelled_at_least`) |
| `advise` | accuracy per gated question and calibration error meet the manifest's `promotion` bars, and reviewers found the findings useful |
| `warn` | the override rate is low and the latency is inside the pipeline's budget |
| `block` | stays only while the weekly eval holds; a rubric that falls below its bars goes back to `advise` |

The numbers are in each rubric's `manifest.yml` under `promotion`;
[Shadow, label, eval, promote](../guides/shadow-and-eval.md) is the
procedure.

## Fail-open by design

A judge that is down should not block a merge. The GitLab block allows
exit 2 and 3 (`allow_failure: exit_codes: [2, 3]`), so a broken judge
warns; a consumer that wants to fail closed in `block` mode sets `[3]`.
The GitHub action's `fail-on` input does the same job
([Run in GitHub Actions](../guides/github.md)). ci-judge writes no
verdict file when it cannot judge, so a gate always sees that case as a
gate error and never as a pass, provided the job lets the gate run after
a failed `run` ([Run in GitLab CI](../guides/gitlab.md)).

## Undecided

An answer near its bar is a coin flip. The manifest's `undecided_margin`
turns such an answer into an `info` finding that never fails; a deferred
Choice or Score (below its confidence bar) is read the same way. A
rubric with many undecided answers on one question has a question to
rewrite, not a bar to move.

## The override

In `block` mode, a label on the merge or pull request
(`judge-gate::override` by default) turns the failure into a warning and
the note says so. Labels are read as the pipeline saw them when it
started, so an override needs a new pipeline, not a job retry. The
override is visible in the note and in the verdict artifact; it is never
silent.
