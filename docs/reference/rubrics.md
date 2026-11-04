---
title: Bundled rubrics
description: The three rubrics compiled into ci-judge; what each judges, the evidence it reads, the requests it makes and the questions it asks, and why only this catalog may block.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, rubrics, catalog, reference]
---

# Bundled rubrics

Three rubrics are compiled into the binary and named by `--rubric
<name>`. They are the examples a team rubric copies, and the only place
a `critical` or `blocker` severity may live: changing a blocking rubric
is a release, reviewed like one. `ci-judge catalog` lists them; each
directory under `rubrics/` holds `manifest.yml`, the `.jud` rubric and a
synthetic cases document. None carries a `tuning` block yet: their bars
are defaults, and `run` says so in a note.

## `infra-plan`

What it judges: whether each destructive Terraform change is
acknowledged in the merge request, and whether the plan as a whole is
reversible.

| | |
|---|---|
| sources | `plan` from `tfplan.json` (reducer `tfplan`), `mr`, `runbook` from `docs/runbook.md` (optional) |
| requests | `change`: one per change whose action is `delete` or `replace`, at most 25; `plan`: one over everything |
| per item | `destructive_ack` (Noul, blocker, fails on no, undecided within 0.1 of the bar): is the change acknowledged with a reason in `mr.description`? |
| global | `rollback_obvious` (Noul, minor, fails on no); `blast_radius` (Score over none, team, platform, company; report only) |
| finding | `{change.action} of {change.address} is not acknowledged in the merge request description`, at the resource's `.tf` file and line |
| promotion | accuracy 0.9 on `destructive_ack`, 0.85 on `rollback_obvious`, ECE 0.1, 20 labelled |

The plan job must leave `tfplan.json`: `terraform show -json tfplan >
tfplan.json`.

## `mr-hygiene`

What it judges: whether a merge request is written for its reviewers.

| | |
|---|---|
| sources | `mr`; `diff` (optional) |
| requests | `mr`: one global request |
| questions | `explains_what`, `explains_why` (Noul, minor); `matches_diff` (Noul, major, undecided within 0.1, asked only when `diff` is present); `breaking_flagged` (Noul, major, asked only with `diff`) |
| promotion | accuracy 0.85 on every question, 20 labelled |

Nothing blocks: every severity is at most `major`.

## `failed-job-triage`

What it judges: why a pipeline failed, and whether the change under
review caused it. Advisory by design (`fail_on: []`).

| | |
|---|---|
| sources | `tests` from `**/junit*.xml` (optional), `jobs` from failed job traces (GitLab, optional, 150 lines), `diff` (optional), `mr` |
| requests | `test`: one per failing test, at most 10; `job`: one per failed job, at most 5; `pipeline`: one over everything |
| per test | `test_caused_by_change` (Noul, major, fails on yes, undecided within 0.1); `test_flaky` (Noul, report only) |
| per job | `job_cause` (Choice: test_failure, build_error, infrastructure, configuration, timeout, unknown; report only); `job_caused_by_change` (Noul, major, fails on yes) |
| global | `pipeline_verdict` (Choice: fix_change, retry, fix_pipeline, unclear; report only) |
| summary | `{failed} failure(s) attributed to this change; {undecided} undecided.` |

The job should run `when: on_failure` with `needs` on the test jobs so
the JUnit reports are there.

## Writing your own

[Write a rubric](../guides/write-a-rubric.md). A repository's rubric
lives in its own directory and is named by path; it may not declare
`critical` or `blocker`.
