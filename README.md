---
title: ci-judge
description: Why ci-judge exists and what it is; merge-request evidence to a calibrated verdict file for a CI gate, on the judgment crate.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, readme]
---

# ci-judge

Merge-request evidence to a calibrated verdict file for a CI gate.

## Why

A CI gate decides on evidence. Tests, linters and policy engines decide
what a rule can express. They cannot decide whether the deletion of a
database is acknowledged in the merge request with a reason, whether a
failed test failed because of the change, or whether a runbook is
adequate. Reviewers decide those, when they read; on a long plan or a red
pipeline, they skim.

A generative model can read, but it answers in text that has to be
parsed, is not calibrated, moves between runs and can be steered by what
it reads. A gate on it is a flaky test with a bill.

A System One model answers typed questions about a JSON state with
calibrated probabilities and generates nothing. The
[`judgment`](https://github.com/chussenot/judgment) crate talks to such
models, reads rubrics from `.jud` files, verifies every answer, records
and replays them, and measures accuracy and calibration against labels.

ci-judge is what sits between a pipeline and that crate. It collects a
change's evidence, reduces it to items that carry a file and a line,
shapes one request per item plus one over everything, asks the rubric,
reads the answers through its policy, and writes one file. A gate
validates the file and decides: on GitLab a `judge-gate` block, on
GitHub `ci-judge gate` or the composite action here.

The model answers; code decides. Thresholds, severities, what fails,
what blocks: policy, in two files a reviewer can read.

## What it is

- One static binary (`x86_64` and `aarch64` Linux, macOS arm64) and a
  `FROM scratch` image, `ghcr.io/chussenot/ci-judge`.
- Three bundled rubrics: `infra-plan`, `mr-hygiene`, `failed-job-triage`.
  A repository writes its own in a directory of three files.
- Five gate modes, `shadow` to `block`, so a rubric earns its place on
  measured accuracy before it may fail a merge.
- A verdict file whose first four fields are the gate's contract and
  whose rest is the audit trail: fingerprints, the model, every answer
  and its bar.

```sh
ci-judge run --rubric infra-plan        # writes judge-gate.verdict.json
ci-judge gate --mode advise             # decides, posts, exits 0/1/2/3
```

## Documentation

[docs/index.md](docs/index.md) is the map: install and a first run, the
guides for GitLab, GitHub, rubrics, shadow mode and the container, the
reference for the command line, configuration, the manifest, the
verdict file and the gate, and the concepts behind the design.
`CHANGELOG.md` is Keep a Changelog; `CONTRIBUTING.md` points at the
developer pages.

## Status

0.1.0. The three bundled rubrics run end to end; the `joblog` source is
GitLab-only; images as evidence and an OpenTelemetry observer are next.

## License

MIT.
