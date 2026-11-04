---
title: "ADR 0001: Fit the binary to the judge-gate block"
description: Why the GitLab job swallows run's exit status rather than the binary remapping it, why a finding's description carries no probability, how a description GitLab cut is read whole, and which differences with the block stay.
status: accepted
last_reviewed: 2026-11-04
tags: [ci-judge, adr, gate, gitlab]
---

# ADR 0001: Fit the binary to the judge-gate block

**Status:** accepted, 2026-11-04.
**Scope:** the seam between `ci-judge run` and a GitLab `judge-gate`
block, as found by running `ci-judge gate` and the block on the same
fixtures before the binary went into a pipeline.

## Context

The block and the binary were written against one contract: the verdict
file, the four exit statuses, the note marker, the Code Quality report.
On the block's fixtures the two agree in every mode on exit status,
dotenv, Code Quality entries and note body, and the block accepts the
verdict ci-judge writes. Four things would not have survived a real
merge request job.

1. `run` exits 1 when a collector or a call fails and 2 when the
   configuration is wrong, and the example ran it in the job's
   `before_script`. GitLab ends a job on the first failing
   `before_script` line, so the block's `script` never ran: no log line,
   no note, no artifact, and the job carried `run`'s status, which the
   block's `allow_failure: exit_codes: [2, 3]` leaves red for a 1. The
   documentation said the gate "sees a gate error". It saw nothing.
2. Every failing or undecided finding ended with
   `(p=0.20, threshold 0.70)`. Both gates fingerprint a Code Quality
   entry over `path:line:description`, so a probability that moved by a
   hundredth between pipelines made the same finding "resolved" and
   "new" on every run.
3. The `mr` source read the description from
   `CI_MERGE_REQUEST_DESCRIPTION`, which GitLab stores up to 2700
   characters only and flags with
   `CI_MERGE_REQUEST_DESCRIPTION_IS_TRUNCATED`. The `infra-plan` blocker
   asks whether the description acknowledges a destructive change; a
   long description with the acknowledgement at its end failed.
4. The example's `artifacts: paths: [recordings/]` replaced the block's
   paths: `extends` merges hashes and not arrays. The verdict file, the
   dotenv, the Code Quality JSON and the route file stopped being
   artifacts; the `reports:` hashes survived, so the widget still worked,
   but shadow mode stored nothing.

## Decision

1. **The job owns "could not judge", not the binary.** `run` keeps
   `jud`'s statuses (1 a call or a collector failed, 2 wrong before any
   call), because `eval`, `check` and the GitHub action read them that
   way. The GitLab job swallows the status:

   ```yaml
   - ci-judge run … || echo "ci-judge: could not judge (status $?); the gate reports a gate error"
   ```

   The block then finds no verdict file, says so and exits 2, the gate
   error the pipeline's `allow_failure` was written for. `action.yml`
   already did this on GitHub. *Rejected:* exit 2 on every failure of
   `run`. The job would have ended with an allowed status, but still
   without the block's log line, note or artifacts, and `run`'s statuses
   would no longer mean what the tools that share them expect.
2. **No number in a description unless the template asks.**
   `verdict::build` writes the template's text as the description;
   `{p}`, `{threshold}` and `{confidence}` stay available to a manifest's
   `finding` template for a rubric that wants them and accepts the churn.
   The probability stays in `ci_judge.answers`, and the `<details>` block
   of `ci-judge gate`'s note shows it. *Rejected:* a per-finding
   `fingerprint` field the block would prefer over its own. A contract
   change on both sides, for a case the template rule already covers.
3. **A cut description is read whole.** When
   `CI_MERGE_REQUEST_DESCRIPTION_IS_TRUNCATED` is `true`, the `mr` source
   reads `GET /projects/:id/merge_requests/:iid` with `GITLAB_TOKEN`, or
   the job token. A failed read is a collection failure, `run` exits 1
   and the gate reports a gate error. *Rejected:* judge the 2700
   characters and add a note. A blocker decided on text the model never
   saw is the false fail the gate exists to avoid.
4. **The example restates the block's artifact paths** beside
   `recordings/`, with the reason in a comment. Documented, not coded:
   nothing in the binary can merge a consumer's YAML.

## Consequences

- A finding stays the same across pipelines as long as its path, line
  and template text do; Code Quality shows it once. A manifest with
  `{p}` in a `finding` template opts back into the churn, knowingly.
- A long description needs a token: a job without `GITLAB_TOKEN`, on an
  instance that refuses the job token on the merge request API, gets a
  gate error on such a merge request, with the reason in the log.
- Two differences with the block remain, accepted. The block
  fingerprints with the base64 of `path:line:description`, `ci-judge
  gate` with its SHA-256: a repository that switches implementation sees
  every finding as new once. `ci-judge gate`'s note carries a
  `<details>` block with the answers; the block's does not.
- The GitLab guide, the verdict file, gate, sources and configuration
  references, `CLAUDE.md` and the changelog say the above;
  `tests/end_to_end.rs` pins the description shape and the API read.
