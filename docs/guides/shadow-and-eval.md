---
title: Shadow, label, eval, promote
description: How a rubric earns its bars; recording answers in shadow mode, labelling the cases it wrote, grading recordings with ci-judge eval and jud eval, tuning with jud tune, and moving the gate up one mode at a time.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, shadow, eval, tune, how-to]
---

# Shadow, label, eval, promote

**You will** take a rubric from untuned defaults to bars that hold, with
the numbers to show for it. **Prerequisites:** a rubric
([Write a rubric](write-a-rubric.md)); a pipeline that runs it; `jud`
for the tuning step.

## 1. Shadow

Run the gate in `shadow` mode and the judge with recording on:

```sh
ci-judge run --rubric infra-plan \
  --record recordings/infra-plan \
  --cases-out recordings/infra-plan-cases.jud
```

Every answer is kept as a `.jud` Recording named after its request
(`mr-4821-change-aws_db_instance.main.jud`), fingerprinted by the request's
state and questions so `jud` and `Replay` find it. Every request is
appended to the cases document as an unlabelled case tagged with its
shape. Keep both as pipeline artifacts, or commit them to the rubric
directory.

## 2. Label

A reviewer fills `expect` in the cases file, case by case, from the
state it holds:

```yaml
    - id: mr-4821-change-aws_db_instance.main
      state: { change: {...}, mr: {...} }
      expect: { destructive_ack: false }       # the reviewer's call
      tags: [change]
```

A case left without `expect` is asked but never graded. Twenty labelled
cases per gated question is the floor the bundled manifests set
(`labelled_at_least`); a hundred is where bars stop moving.

## 3. Eval

```sh
ci-judge eval --rubric rubrics/infra-plan --replay recordings/infra-plan
```

Grades every labelled case from its recording, no model call: accuracy
with a 95% interval, Brier score and expected calibration error per
question, a confusion table, and the best-F1 threshold for each yes/no
question. Exit 3 when a question is below the manifest's `promotion`
bars, 1 when a case has no recording (a case or a question changed:
record again), 0 otherwise.

`jud eval` over the same files says the same from the crate's own
command line, with its gate tables and `--against` for a second model.

## 4. Tune

```sh
jud tune rubrics/infra-plan/infra-plan.jud rubrics/infra-plan/infra-plan-cases.jud --replay recordings/infra-plan
```

Proposes each gate's bar from the recordings. Copy the ones you accept
into the rubric's `policy`, with a `note` saying why (the proposal, or
the policy choice that overrode it), and let `jud tune --apply` or your
hand write the `tuning` block: the cases' fingerprint, the model, the
server, the date. From then on `ci-judge run` refuses any other model
unless `--allow-untuned`, because bars are tuned per model.

## 5. Promote

Raise `JUDGE_GATE_MODE` (or the action's `mode`) one step: `advise`,
then `warn`, then `block` on production only. The criteria are in
[Modes and promotion](../concepts/modes.md). Keep the weekly eval in the
rubric repository's CI: a rubric that falls below its bars goes back
down.

## When the model changes

The response names the model that answered; a rubric tuned on
`jev-1.14.0` is not tuned on `jev-1.15.0`. `jud record --refresh` on the
same cases, then `jud tune`, then the rubric change: that is the whole
migration, and `jud eval --replay NEW --against OLD` says what moved.
