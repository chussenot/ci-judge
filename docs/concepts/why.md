---
title: Why ci-judge
description: The problem a judgment gate solves, what a System One model changes about it, what was rejected and what the design costs.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, concepts, design]
---

# Why ci-judge

## The problem

A CI gate is a function from evidence to a decision. Today the function
is code: a test suite, a linter, a policy engine. Those answer questions a
rule can express. They cannot answer "is the deletion of this database
acknowledged in the merge request, with a reason?", "did this test fail
because of this change or because the network blinked?", or "is this
runbook adequate?". Reviewers answer those, when they read; on a long
Terraform plan or a failed pipeline, they skim.

A model can answer them. A generative model answers in text, which has to
be parsed, is not calibrated, moves between runs, and can be steered by
the content it reads. Gating a merge on it means gating on a flaky test
with a bill.

## What was decided

A **System One model** does not generate text. It evaluates a JSON state
against typed questions (yes or no, one of these options, a level on a
scale) and returns probabilities meant to be calibrated. The
[`judgment`](https://github.com/chussenot/judgment) crate speaks to such
models, reads a rubric of questions and a policy of bars from a `.jud`
file, verifies every answer against the questions asked, records and
replays answers, and measures accuracy and calibration against labels.

ci-judge is the part between a pipeline and that crate:

1. It collects a change's evidence (a plan, test reports, the diff, the
   request's own fields, failed job traces) and reduces each to items
   that carry a file and a line.
2. It shapes one request per item plus one over everything, so a yes
   about one item points at one line. This is the
   [granularity rule](granularity.md).
3. It asks the rubric and reads the answers through its policy.
4. It writes one file, `judge-gate.verdict.json`, in a contract a gate
   validates before deciding.

The gate is a separate thing on purpose: on GitLab a `judge-gate` block,
on GitHub `ci-judge gate` or the composite action. Judging and deciding
are joined by a file, so each can be tested alone and the decision layer
can be shell.

**The model answers, code decides.** Thresholds, severities, what counts
as a failure, what blocks: all policy, in two files a reviewer can read.

## What was rejected

- **A generative judge with reasons and quotes.** It explains itself
  better, and that is the only thing it does better. Calibration, replay,
  verification and determinism all cost more to add than the explanation
  is worth, and the explanation was never the gate's job.
- **Judging inside the gate.** One job that collects, asks and decides is
  shorter to write and impossible to test without a model. The file in
  between costs one `jq` validation and buys a decision layer that runs
  on fixtures.
- **One request per change.** A global question over a whole plan
  returns one number nobody can act on. The cost of N+1 small requests is
  accepted; the item cap bounds it.
- **A rubric format of its own.** The `.jud` format is the crate's, read
  by `jud` and by any other tool. Everything CI-specific lives in a
  manifest beside the rubric, because the reader refuses unknown fields
  and a rubric that only this binary could read would lose `jud check`,
  `jud eval` and `jud tune`.

## What it costs

- **No prose.** A finding is a template filled from the item and the
  probability. A reviewer reads "delete of aws_db_instance.main is not
  acknowledged (p=0.20, threshold 0.70)", not a paragraph.
- **Calibration work.** A rubric starts in shadow mode, collects
  recordings, needs labelled cases and a tune before it may warn, and
  more before it may block. [Modes and promotion](modes.md) is the
  ladder.
- **A backend.** The hosted API, or a System One server you run. The
  binary reads the same settings the `jud` command does, so a laptop and
  a runner agree.
- **Questions are a craft.** A question that reads the whole state
  yields noise; one that names its keys in backticks and has `criteria`
  for both answers yields a bar that holds. The bundled rubrics are the
  examples.
