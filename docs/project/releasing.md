---
title: Releasing
description: The runbook from a merged main to a release; the changelog section, cog bump, the tag push, what the release workflow builds and publishes, and the dry run to do first.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, release, project]
---

# Releasing

A release is a tag. The tag push runs `.github/workflows/release.yml`,
which runs the CI gates, builds three static binaries, publishes a
GitHub release with checksums and build provenance, builds the container
image from those exact binaries and pushes it. Nothing is published by
hand, and the Bash guard in `.claude/hooks/` denies the commands that
would.

## Before the first tag of a line

Run the workflow by hand (`workflow_dispatch`): it builds everything,
runs the image, and pushes nothing. A failure there is cheaper than a
half-published release.

## The steps

1. On `main`, with a clean tree: write the version's section in
   `CHANGELOG.md` by hand, headed `## [x.y.z] - YYYY-MM-DD`, moving the
   lines out of `Unreleased`. `cog bump` refuses without it.
2. `mise run release:check`: what the next version would be, from the
   Conventional Commits since the last tag.
3. `mise run release`: `cog bump --auto` bumps `Cargo.toml`, runs `cargo
   check` so `Cargo.lock` follows, commits `chore(version): vx.y.z` and
   tags `vx.y.z`.
4. `git push` the commit, then the tag explicitly: `git push origin
   vx.y.z`. The tag push is the release owner's.

## What the workflow does

| Job | Produces |
|---|---|
| `gate` | the CI gates, through `ci.yml` |
| `build` | `ci-judge-x86_64-unknown-linux-musl`, `ci-judge-aarch64-unknown-linux-musl`, `ci-judge-aarch64-apple-darwin`, each with a `.sha256`; each binary run once |
| `publish` | the GitHub release with those files, generated notes, and a build-provenance attestation |
| `image` | `ghcr.io/chussenot/ci-judge` for `linux/amd64` and `linux/arm64` from the `build` job's binaries, tagged `x.y.z`, `x.y` and `latest`, run once as uid 65532, attested |

## After

- The first push to `ghcr.io/chussenot/ci-judge` creates a private
  package; making it public is a one-time setting on the package page.
- `action.yml` resolves its binary from the action's tag, so a workflow
  pinned to `chussenot/ci-judge@vx.y.z` downloads that release.
