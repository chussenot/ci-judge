---
title: ci-judge documentation
description: The map of this documentation, by what a reader wants to do; tutorials, how-to guides, reference pages and the concepts behind them.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, index]
---

# ci-judge

ci-judge turns the evidence of a merge request into a verdict file a CI
gate can act on. It asks a System One model typed questions and gets
calibrated probabilities back; the rubric's policy and the manifest beside
it turn those into findings with a file and a line. Nothing here generates
text.

The pages are organised by what you want to do.

## Start

| Page | Answers |
|---|---|
| [Install](start/install.md) | How to get the binary onto a runner, a laptop or into a container |
| [First run](start/first-run.md) | A judgment of a fixture, with and without a model |

## Guides

| Page | Answers |
|---|---|
| [Run in GitLab CI](guides/gitlab.md) | How the binary and a `judge-gate` block share one job |
| [Run in GitHub Actions](guides/github.md) | The composite action, `fail-on`, and the outputs a later job reads |
| [Write a rubric](guides/write-a-rubric.md) | The three files a rubric is made of, and the rule that decides their shape |
| [Shadow, label, eval, promote](guides/shadow-and-eval.md) | How a rubric earns the right to block |
| [Run in a container](guides/container.md) | What the image holds, what it lacks, how files reach it |

## Reference

| Page | Answers |
|---|---|
| [Command line](reference/cli.md) | Every subcommand, flag and exit status |
| [Configuration](reference/configuration.md) | Backend settings, `CI_JUDGE_*`, `JUDGE_GATE_*`, what each platform reads |
| [Manifest](reference/manifest.md) | Every field of `manifest.yml` |
| [Sources and reducers](reference/collectors.md) | What can be collected and what each reducer yields |
| [Verdict file](reference/verdict-file.md) | The gate's contract and the audit trail under `ci_judge` |
| [Gate](reference/gate.md) | The five modes, the override label, what each platform gets |
| [Bundled rubrics](reference/rubrics.md) | `infra-plan`, `mr-hygiene`, `failed-job-triage` |
| [Container image](reference/container-image.md) | `ghcr.io/chussenot/ci-judge`: contents, user, tags, provenance |

## Concepts

| Page | Answers |
|---|---|
| [Why ci-judge](concepts/why.md) | The problem, what was rejected, what the design costs |
| [The granularity rule](concepts/granularity.md) | Why a question is asked once per evidence item |
| [Modes and promotion](concepts/modes.md) | Why a rubric starts in shadow and how it moves |

## Project

| Page | Answers |
|---|---|
| [Architecture](project/architecture.md) | The C4 model: context, containers, modules, one run |
| [Contributing](project/contributing.md) | The gates, the hooks, the Claude Code harness, cloud sessions |
| [Releasing](project/releasing.md) | From `cog bump` to the binaries and the image |
| [ADR 0001: Fit the judge-gate block](project/decisions/0001-fit-the-judge-gate-block.md) | Why the job swallows `run`'s status, why a description carries no number, how a cut description is read whole |
| [Study: the mechanics of ci-judge](project/research/mechanics-study.md) | What the field does for the same mechanics, where the implementation is wrong or thin, which crates earn their place, and the three slices of work ranked first |

`CHANGELOG.md` at the repository root is Keep a Changelog.
