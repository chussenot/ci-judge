---
title: Run in GitHub Actions
description: How the composite action installs ci-judge, judges a pull request and gates on the verdict, what fail-on means where there is no allow_failure, and the outputs a later job reads.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, github-actions, how-to]
---

# Run in GitHub Actions

**You will** have a pull request workflow that judges a change, posts a
comment and annotations, and fails or not according to a mode.
**Prerequisites:** a System One backend; `GITHUB_TOKEN` with
`pull-requests: write` for the comment.

On GitHub there is no block: the composite action in this repository
downloads the release binary for the runner's architecture, verifies its
checksum, runs `ci-judge run` and then `ci-judge gate`.

## The workflow

```yaml
name: judge
on:
  pull_request:

permissions:
  contents: read
  pull-requests: write

jobs:
  mr-hygiene:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0                 # the diff source needs the merge base
      - uses: chussenot/ci-judge@v0.1.0
        with:
          rubric: mr-hygiene
          mode: advise                   # shadow, advise, warn, block or route
        env:
          TYPESAFE_API_KEY: ${{ secrets.TYPESAFE_API_KEY }}
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

`examples/github-workflow.yml` adds a second job over an artifact
(`tfplan.json` from a plan job, downloaded with
`actions/download-artifact`).

## Inputs

| Input | Default | Meaning |
|---|---|---|
| `rubric` | required | A bundled name, a manifest path, or a directory with `manifest.yml` |
| `mode` | `shadow` | The gate mode |
| `version` | the action's tag | The release to download |
| `model` | the jud configuration's | The model to ask |
| `replay`, `record`, `cases-out` | | Replay from recordings; record answers; grow the cases document |
| `override-label` | `judge-gate::override` | The label that turns a block into a warning |
| `routes` | `.github/judge-gate` | Route pipelines directory, route mode |
| `allow-untuned` | `false` | Run a rubric tuned on another model |
| `extra-args` | | More flags for `ci-judge run` |
| `fail-on` | `block` | Which gate statuses fail the step (below) |

## `fail-on`

GitHub has no `allow_failure`: a non-zero step fails the job. The input
maps the gate's four statuses onto that:

| `fail-on` | fails the step on | annotates and passes on |
|---|---|---|
| `block` (default) | 1 blocked | 2 gate error, 3 warning: the GitLab block's `[2, 3]` behaviour |
| `closed` | 1, 2 | 3: a broken judge blocks |
| `warn` | 1, 3 | 2 |
| `never` | nothing | everything; read the `status` output |

## Outputs

`verdict` (`pass` or `fail`), `route` (route mode) and `status` (0, 1, 2
or 3). A job exposes them for a later job to gate on:

```yaml
  mr-hygiene:
    runs-on: ubuntu-latest
    outputs:
      verdict: ${{ steps.judge.outputs.verdict }}
    steps:
      - uses: actions/checkout@v4
        with: { fetch-depth: 0 }
      - id: judge
        uses: chussenot/ci-judge@v0.1.0
        with: { rubric: mr-hygiene, mode: advise }
        env:
          TYPESAFE_API_KEY: ${{ secrets.TYPESAFE_API_KEY }}
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

  e2e:
    needs: mr-hygiene
    if: needs.mr-hygiene.outputs.verdict == 'fail'
    runs-on: ubuntu-latest
    steps:
      - run: echo "the full suite runs only when the judge found something"
```

## What the gate posts

One pull request comment per gate, updated in place on every run
(marker `<!-- judge-gate:<job> -->`), `::error` and `::warning`
annotations on the files the findings name, a section in the step
summary, and the files `judge-gate.verdict.json` and `judge-gate.env` in
the working directory.

## Without the action

The two commands the action runs, for a workflow that installs the
binary itself:

```yaml
      - run: ci-judge run
        env:
          CI_JUDGE_RUBRIC: infra-plan
          TYPESAFE_API_KEY: ${{ secrets.TYPESAFE_API_KEY }}
      - run: ci-judge gate --mode warn
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

The `joblog` source is GitLab-only today; on GitHub pass a failing step's
log as an `artifact` source.
