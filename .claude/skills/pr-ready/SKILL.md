---
name: pr-ready
description: Take this repository's current branch from "the change is done" to an open draft pull request, with the reviewers the diff calls for having reported and the gate passed first. Use when a change is committed and ready for review, or when asked to open a pull request here.
---

# From a finished change to a pull request

CLAUDE.md says to run the reviewers on a diff before a pull request; this
is the order that gets there in one round. It ends by handing the branch
to the `pr-shepherd` agent, which owns the pull request's shape and reads
its checks. Nothing here merges, approves, pushes to `main` or rewrites
history.

## 1. The reviewers this diff needs

List the files: `git diff --name-only origin/main...HEAD`. Then, in
parallel, one agent per row that applies:

| The diff touches | Run | Why |
|---|---|---|
| `src/verdict.rs`, `src/gate.rs`, `src/manifest.rs`, `src/run.rs`, `src/eval.rs`, `src/platform.rs`, `action.yml` | `contract-reviewer` | the verdict file against the gate's validation, the `.jud` documents against the reader, the platform variables against GitLab's and GitHub's |
| `rubrics/**`, a repository's own rubric directory | `contract-reviewer`, asked about the rubric; `rubric-author` first when a question is new | a document another tool reads, and a question whose finding must name a line |
| `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `CLAUDE.md`, a rustdoc module page, `examples/**` | `docs-writer` | claims that no longer hold, and no organisation named |
| `src/**` with no test file in the diff | `test-writer`, first | a check without its negative case is not done |

Reviewers report and do not edit (except `rubric-author` and `docs-writer`,
which edit what they own). Verify each finding in the code before acting on
it: fix the ones that hold and commit; a finding that does not hold gets
one line in the pull request's **Decisions to review**. A row skipped
because its files did not change needs no mention.

A change to the verdict contract, an exit status or `action.yml` also moves
`CHANGELOG.md` (Unreleased) and the README; check both are in the diff.

## 2. The gate, before the push

`mise run ci` runs CI's local gates in CI's order, but not all of CI: the
jobs that need a second toolchain or the network stay outside it. When the
diff touches `Cargo.toml` or `Cargo.lock`, run them too, because that is
the change they exist for:

```sh
mise run deny msrv hack
```

When it touches `Dockerfile`, `scripts/image-context.sh` or the workflows:

```sh
mise run build:musl && scripts/image-context.sh --local
```

and, where Docker runs, `mise run image`. The `musl` CI job does the first
two; only the release workflow's dry run builds the multi-platform image.

## 3. Push, then hand off

- Conventional Commits, one concern per commit (`cog verify` runs on each
  through the commit-msg hook).
- Push the branch (`git push -u origin claude/<name>`); the pre-commit
  hooks already ran fmt and clippy.
- Hand the pushed branch to `pr-shepherd`, which opens a **draft** with the
  body it defines (Summary, Decisions to review, Test plan). Give it the
  reviewers' findings and what the gate ran, so the Test plan is true.
- Subscribe to the pull request's activity and end the turn: checks and
  reviews wake the session. The `steward` skill covers what comes next.
