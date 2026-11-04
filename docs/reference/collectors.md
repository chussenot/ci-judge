---
title: Sources and reducers
description: What ci-judge can collect from a pipeline, what each reducer turns raw output into, and which of them yield items with a file and a line.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, collectors, reducers, reference]
---

# Sources and reducers

A source is where a value comes from; a reducer is what it becomes. A
reducer returns the value the global state carries and, where the output
is a list of things, items with a locator. Items are what per-item
requests are made of ([The granularity rule](../concepts/granularity.md)).

## Sources

| `source` | Reads | Default reducer | Needs |
|---|---|---|---|
| `artifact` | a file, or every file a glob matches, under the working directory | by extension: `.xml` junit, `.json` json, `.diff`/`.patch` diffhunks, else text | the artifact of an earlier job (`needs`) |
| `diff` | `git diff --merge-base <base> HEAD`; without a base, the last commit | diffhunks | the merge base in the checkout (`GIT_DEPTH: 0`, `fetch-depth: 0`) |
| `mr` | the change under review: `title`, `description`, `labels`, `author`, `source_branch`, `target_branch`, `source_sha`, `base_sha`, `draft`, `url`, `number` | none | a merge request pipeline, a pull request event, or `CI_JUDGE_TITLE` and `CI_JUDGE_DESCRIPTION` |
| `file` | a text file from the checkout | text | |
| `env` | a variable's value | text | |
| `joblog` | the failed jobs of this pipeline with the tail of each trace | items per job | GitLab, `CI_PIPELINE_ID`, a token that may read traces |

Several files under one glob become a list of `{file, value}` and their
items pooled, each item's key prefixed with the file's stem.

## Reducers

| `reduce` | Value | Items | Locator |
|---|---|---|---|
| `tfplan` | `summary` (count per action), `changes` | one per resource change that is not `no-op` or `read`: `address`, `type`, `name`, `action` (`create`, `update`, `delete`, `replace`, `other`), `before_keys`, `after_keys`, `action_reason`, `replace_paths` | the `.tf` file and line of `resource "<type>" "<name>"`, found by scanning the checkout |
| `junit` | `total`, `failed`, `skipped`, `failing` | one per failing or erroring test case: `name`, `classname`, `suite`, `failure` (`kind`, `message`, `type`, the last 60 lines of text) | the `file` and `line` attributes when the report has them |
| `diffhunks` | `files`, `additions`, `deletions`, `changed`, `base`, `head` | one per changed file: `path`, `additions`, `deletions`, `hunks` (capped at 4000 characters, `truncated` says so) | the file and its first changed line |
| `json` | the parsed document | none | |
| `text` | the text, or its last `tail_lines` | none | |
| `lines` | the lines as a list | none | |

`tfplan` never carries resource values into the state, only attribute
names (`before_keys`, `after_keys`): the values are where secrets live.

## Redaction

Every string in every collected value goes through the secret patterns
before any network call: private keys, AWS, GitHub, GitLab, Slack and
Google keys, JWTs, bearer tokens, credentials in URLs, and
`password=`-style assignments. A match becomes `[REDACTED:<kind>]`, so a
question can still see that something was there.

## Sizes

The global state is capped at `--budget-bytes` (512 000 by default);
when it is over, the key with the highest `priority` number is dropped
first, item sources last, and the verdict records `truncation.keys_dropped`.
Item requests are small by construction: one item plus the non-item
sources.
