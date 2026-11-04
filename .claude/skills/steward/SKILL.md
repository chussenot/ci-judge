---
name: steward
description: This repository's conventions for driving an open pull request to green after it is opened, merging main in, Dependabot batches, and a pull request that gets no checks. Read before acting on a CI, review or merge-conflict event on a pull request here.
---

# Driving a pull request here

`pr-shepherd` reads a red check and tells a failure the change caused from
one it did not. This skill holds what it does not: how this repository
brings `main` in and what to look at afterwards.

## No checks after a push means a conflict

GitHub runs the pull-request workflows on the merge of the branch into its
base. When that merge conflicts, no workflow runs and the check list is
empty, which reads like "still queued". Fetch the pull request: a
`mergeable_state` of `dirty` is the answer, and the conflict is the work.

## Bringing `main` in

- A merge commit, never a rebase or a force push: `git fetch origin main`,
  `git merge --no-ff origin/main`, the message git proposes
  (`Merge remote-tracking branch 'origin/main' into <branch>`; `cog`
  ignores merge commits).
- A conflict in `Cargo.lock` is resolved by taking `main`'s file and
  running `cargo check`, which rewrites what the branch needs.
- A conflict in `CHANGELOG.md` keeps both sides' Unreleased lines.
- A conflict on a README section is resolved from `main`'s text with this
  branch's change applied on top: `main` may have rewritten the paragraph.

## Several Dependabot pull requests merged

Each one's CI ran on its own merge into the `main` it saw, so the set
together was never tested; two majors can each pass and fail combined.
After a batch merges, run CI's gates on `main` itself, the local ones and
the three `mise run ci` leaves out:

```sh
git fetch origin main && git checkout --detach origin/main
mise run ci && mise run deny msrv hack
```

A failure there is a pull request of its own against `main`, not a fix
pushed into whichever branch is open. Read the changelog of any major
(`~/.cargo/registry/src/*/<crate>-<version>/CHANGELOG.md`) against what
the crate uses. A bump of `judgment` is read against its changelog with
particular care: the `.jud` reader, `Rubric::lower`, `Rubric::apply` and
the `Replay` keys are what this crate leans on, and a new `MINOR` of the
format may change what the bundled rubrics may say.

## After the merge

Nothing to close here: the pull request is the record. If the merge
changed the verdict contract, an exit status or `action.yml`, check that
`CHANGELOG.md` says so under Unreleased before the next release.
