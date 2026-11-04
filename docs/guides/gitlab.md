---
title: Run in GitLab CI
description: How ci-judge and a judge-gate block share one merge request job, where the binary and its credentials come from, and how a job fails open or closed.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, gitlab, how-to]
---

# Run in GitLab CI

**You will** have a merge request job that judges a change and decides
on the verdict. **Prerequisites:** a `judge-gate` block, a hidden job
that validates `judge-gate.verdict.json` and applies `JUDGE_GATE_MODE`
with exit 0 pass, 1 blocked, 2 gate error, 3 warning; a System One
backend; a token that may post merge request notes.

The division of labour: ci-judge runs in the job's `before_script` and
writes the verdict file; the block's `script` is the decision. A block is
any job with that contract; `ci-judge gate` is one implementation when
no block is available ([Gate](../reference/gate.md)).

## The job

```yaml
include:
  - project: 'your-group/building-blocks'
    file: 'blocks/judge-gate/block.yml'
    ref: 'judge_gate/v0.1.0'

variables:
  CI_JUDGE_VERSION: "0.1.0"
  CI_JUDGE_URL: "https://github.com/chussenot/ci-judge/releases/download"

.ci-judge:install:
  before_script:
    - |
      mkdir -p "${HOME}/.local/bin"
      curl -fsSL -o "${HOME}/.local/bin/ci-judge" "${CI_JUDGE_URL}/v${CI_JUDGE_VERSION}/ci-judge-x86_64-unknown-linux-musl"
      curl -fsSL -o /tmp/ci-judge.sha256 "${CI_JUDGE_URL}/v${CI_JUDGE_VERSION}/ci-judge-x86_64-unknown-linux-musl.sha256"
      (cd "${HOME}/.local/bin" && echo "$(cut -d' ' -f1 /tmp/ci-judge.sha256)  ci-judge" | sha256sum -c -) || { echo "ci-judge checksum mismatch"; exit 2; }
      chmod +x "${HOME}/.local/bin/ci-judge"
      export PATH="${HOME}/.local/bin:${PATH}"
      ci-judge version

judge-gate:plan:
  extends: .judge-gate:decide
  stage: test
  needs: [terraform-plan]                      # the job that leaves tfplan.json
  variables:
    JUDGE_GATE_MODE: warn
    CI_JUDGE_RUBRIC: infra-plan
    GIT_DEPTH: 0                               # the diff source needs the merge base
  allow_failure:
    exit_codes: [3]                            # fail closed: a broken judge fails the job
  before_script:
    - !reference [.ci-judge:install, before_script]
    # TYPESAFE_API_KEY and GITLAB_TOKEN come from your secret store, never from a file in the job.
    - ci-judge run --record recordings/infra-plan --cases-out recordings/infra-plan-cases.jud
  artifacts:
    when: always
    paths: [recordings/]
```

`examples/gitlab-ci.yml` in the repository is this file.

## What the job needs

| Need | Why | How |
|---|---|---|
| the merge base in the checkout | the `diff` source runs `git diff --merge-base` | `GIT_DEPTH: 0` |
| the artifacts of earlier jobs | `artifact` sources read files from `needs` | `needs: [job]` |
| `TYPESAFE_API_KEY` (and `TYPESAFE_BASE_URL` for another server) | the backend | a masked, protected variable, or your secret store in `before_script` |
| `GITLAB_TOKEN` with the `api` scope | the block posts the note; ci-judge reads failed job traces with it | a short-lived project token from your secret store; the job token cannot create notes |
| `CI_MERGE_REQUEST_*` | the `mr` source and the override label | a merge request pipeline (`rules: - if: $CI_MERGE_REQUEST_IID`) |

## Fail open or closed

The block's default, `allow_failure: exit_codes: [2, 3]`, lets a gate
error (no verdict file, an invalid one) and a warning pass the pipeline:
a judge that is down warns. In `block` mode that is a hole, so the job
above sets `[3]`. ci-judge never writes a verdict when it cannot judge,
so the two cases are never confused with a pass.

## The override

Add the `judge-gate::override` label (or the block's
`JUDGE_GATE_OVERRIDE_LABEL`) to the merge request and run a new pipeline.
Labels are read from `CI_MERGE_REQUEST_LABELS`, set when the pipeline is
created, so retrying the job alone changes nothing.

## Route mode

```yaml
judge-gate:risk:
  extends: .judge-gate:decide
  variables:
    JUDGE_GATE_MODE: route
    JUDGE_GATE_ROUTES: .gitlab/judge-gate       # low.yml, medium.yml, high.yml
    CI_JUDGE_RUBRIC: change-risk
  before_script:
    - !reference [.ci-judge:install, before_script]
    - ci-judge run

judge-gate:route:
  needs: [judge-gate:risk]
  trigger:
    include:
      - artifact: judge-gate.route.yml
        job: judge-gate:risk
    strategy: mirror
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
```

The route is the chosen option of the manifest's `route_from` question,
or the verdict itself. Downstream jobs also get `JUDGE_GATE_VERDICT` and
`JUDGE_GATE_ROUTE` through the dotenv report.

## Inside a job image

The container image has no shell, so a job that runs *inside* it fails
before its script. Copy the binary into a job image of your own:

```dockerfile
FROM alpine:3.21
COPY --from=ghcr.io/chussenot/ci-judge:0.1.0 /usr/local/bin/ci-judge /usr/local/bin/ci-judge
RUN apk add --no-cache git
```
