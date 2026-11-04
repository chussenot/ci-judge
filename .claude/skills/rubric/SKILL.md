---
name: rubric
description: How to add, change or promote a rubric in this repository, from the three files it is made of to the shadow run, the labels, the eval and the tuning. Read before touching anything under rubrics/ or helping a repository write its own.
---

# A rubric, from first draft to a gate

A rubric is three files in one directory: `manifest.yml` (this crate's
sidecar: sources, request shapes, severities, findings, promotion bars),
`<name>.jud` (the `Rubric` the `judgment` crate reads: questions, criteria,
policy, tuning) and `<name>-cases.jud` (the labelled `Cases`). The
`rubric-author` agent writes them; this skill is the lifecycle around
them.

## The rule that decides everything

A System One answer is a probability with no reason and no locator, so
a finding's file and line come from the request, not the answer: one
request per evidence item, the item under one key, the question gated
with `when: <item key>`. A blocker is always per item. Global questions
(`when: <state key>` or `when: <request name>`) carry no path and may not
block.

## Draft

```sh
cargo run -- check rubrics/<name>                 # the three files read together
cargo run -- explain --rubric rubrics/<name> --platform local   # on a fixture in the working directory
jud check rubrics/<name>/*.jud                    # the crate's own reader, when jud is on PATH
```

`explain` prints each request, the questions its `when` left in, the
locator and the byte size. A request asked no question is a `when` that
names a key the state lacks.

## Shadow

A pipeline runs the rubric with `--record DIR --cases-out FILE` and the
gate in `shadow` mode. Every answer becomes a `.jud` Recording named after
its request; every request becomes an unlabelled case. Reviewers fill
`expect` in the cases file; the recordings are committed beside it.

## Eval and tune

```sh
cargo run -- eval --rubric rubrics/<name> --replay rubrics/<name>/recordings
jud eval rubrics/<name>/<name>.jud rubrics/<name>/<name>-cases.jud --replay rubrics/<name>/recordings
jud tune rubrics/<name>/<name>.jud rubrics/<name>/<name>-cases.jud --replay rubrics/<name>/recordings
```

`eval` exits 3 below the manifest's `promotion` bars (accuracy, calibration
error, labelled count), 1 when a case has no recording. `tune` proposes
the bars; they are copied into `policy` by hand with a `note`, and the
`tuning` block records the cases' fingerprint, the model and the date.
From then on `ci-judge run` refuses a model other than `tuning.model`
unless `--allow-untuned` is passed, and a model change is a re-record and
a re-tune.

## Promote

A rubric moves from `shadow` to `advise` to `warn` to `block` in the
pipeline's `JUDGE_GATE_MODE` (or the action's `mode`), one step at a time,
on the eval's numbers. Only the bundled catalog may hold `critical` and
`blocker` severities; a repository's own rubric stops at `major`, so a
team does not edit the rubric that blocks its own merges.
