---
title: Configuration
description: Every setting ci-judge reads and where it comes from; the backend settings shared with the jud command, the CI_JUDGE_* and JUDGE_GATE_* variables, and the platform variables each CI system provides.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, configuration, reference]
---

# Configuration

Three families of settings, with three sources of truth: the backend is
configured the way the `jud` command is; the judge and the gate read
their own variables; the platform is read from what GitLab or GitHub
set.

## The backend

Resolved exactly as `jud` resolves it, so a laptop and a runner agree;
`ci-judge config` prints the result and where each value came from.

| Setting | Flag | Environment | File field | Default |
|---|---|---|---|---|
| base URL | | `TYPESAFE_BASE_URL` | `base_url` | `https://api.typesafe.ai` |
| API key | | `TYPESAFE_API_KEY` | `api_key` | none; `run` exits 2 before any call |
| model | `--model` | `CI_JUDGE_MODEL` | `model` | `jev-latest` |
| timeout per attempt | | | `timeout_secs` | 30 |
| long context | | | `long_context` | false |
| shared images | | | `shared_images` | false |

The file is `$XDG_CONFIG_HOME/jud/config.yaml`, else
`$HOME/.config/jud/config.yaml`, only when that base is an absolute path;
a relative one would read a file from the checkout under review. An
unknown field in the file is refused. Any server that speaks
`POST /v1/systemone` is a base URL; there is no provider switch.

The key is never printed: `config` says `environment`, `config file` or
`missing`.

## The judge

`CI_JUDGE_*`, one per `run` flag ([Command line](cli.md)): `RUBRIC`,
`OUT`, `MODEL`, `REPLAY`, `RECORD`, `CASES_OUT`, `MAX_ITEMS`,
`BUDGET_BYTES`, `MAX_IN_FLIGHT`, `ALLOW_UNTUNED`, `ALLOW_LOCAL_BLOCKERS`,
`PLATFORM`. A flag wins over its variable.

Outside a pipeline (`--platform local`), the change under review has no
API to come from; three variables stand in: `CI_JUDGE_TITLE`,
`CI_JUDGE_DESCRIPTION` and `CI_JUDGE_BASE` (the merge base the `diff`
source diffs against; without it, the last commit).

## The gate

`JUDGE_GATE_*`, the same names the GitLab block reads: `MODE`,
`VERDICT_FILE`, `OVERRIDE_LABEL`, `ROUTES`, `OUT`, `NAME`. `GITLAB_TOKEN`
posts the merge request note; `GITHUB_TOKEN` the pull request comment.

## What each platform provides

The platform is detected from `GITLAB_CI`, then `GITHUB_ACTIONS`, else
local; `CI_JUDGE_PLATFORM` overrides.

| Need | GitLab | GitHub | Local |
|---|---|---|---|
| the change | `CI_MERGE_REQUEST_IID`, `_TITLE`, `_DESCRIPTION` (and `_DESCRIPTION_IS_TRUNCATED`, which sends the `mr` source to the API), `_LABELS`, `_SOURCE_BRANCH_NAME`, `_TARGET_BRANCH_NAME`, `_DIFF_BASE_SHA` (else `_TARGET_BRANCH_SHA`), `CI_COMMIT_SHA`, `GITLAB_USER_LOGIN` | the `pull_request` object of the event at `GITHUB_EVENT_PATH` (`number`, `title`, `body`, `labels`, `head`, `base`, `user`, `draft`, `html_url`), `GITHUB_SHA` | `CI_JUDGE_TITLE`, `CI_JUDGE_DESCRIPTION`, `CI_JUDGE_BASE` |
| the API | `CI_API_V4_URL`, `CI_PROJECT_ID`, `CI_PROJECT_PATH`, `CI_SERVER_URL`; `GITLAB_TOKEN` (`PRIVATE-TOKEN`) or `CI_JOB_TOKEN` (`JOB-TOKEN`, reads only) | `GITHUB_API_URL`, `GITHUB_REPOSITORY`, `GITHUB_TOKEN` | none |
| failed job traces | `CI_PIPELINE_ID` and a token that may read traces (`joblog` source) | not available | not available |
| the gate's report | `CI_JOB_NAME` (the note's marker), `CI_JOB_URL` | `GITHUB_JOB`, `GITHUB_RUN_ID`, `GITHUB_OUTPUT`, `GITHUB_STEP_SUMMARY` | files under `--out` |
| request and case ids | `mr-<iid>-…` | `pr-<number>-…` | `local-…` |

The override label is read from `CI_MERGE_REQUEST_LABELS` or the
event's `labels`, both set when the pipeline started: an override needs
a new pipeline.

## Secrets

`TYPESAFE_API_KEY`, `GITLAB_TOKEN` and `GITHUB_TOKEN` come from the
platform's masked variables or a secret store, never from a file in the
job. Every string in a state is passed through the secret patterns of
`src/redact.rs` before any call; a match becomes `[REDACTED:<kind>]`.
