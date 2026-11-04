---
title: Contributing
description: How to work on ci-judge; the checkout, the gates and the CI jobs that run them, the git hooks, the Claude Code harness, and what a cloud session's start hook installs.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, contributing, project]
---

# Contributing

## The checkout

```sh
mise install        # rust 1.97.0 with rustfmt and clippy, cargo-deny, cargo-hack, cocogitto, jud, prek, gh
mise run setup      # the git hooks (prek) and cargo fetch
mise run ci         # fmt:check, lint, test, check, deny
```

`mise tasks` lists every task. A `.env` beside the checkout, gitignored
and loaded by mise, holds `TYPESAFE_API_KEY`, `TYPESAFE_BASE_URL` and the
platform tokens for `mise run judge:run`; `.env.example` documents them.

## The gates

| Gate | Local | CI job |
|---|---|---|
| formatting | `mise run fmt:check` | `fmt, clippy, test, check` |
| clippy, warnings as errors, every target and feature | `mise run lint` | same |
| unit and end-to-end tests, a `Fake` backend, no key | `mise run test` | same |
| the bundled rubrics, manifests and cases read together | `mise run check` | same |
| licences, advisories, duplicate versions, sources (`deny.toml`) | `mise run deny` | `licences, advisories, bans, sources` |
| the crate builds on its `rust-version` | `mise run msrv` | `builds on rust-version` |
| every feature combination builds | `mise run hack` | none yet |
| the static binary and the image context | `mise run build:musl` then `scripts/image-context.sh --local` | `static binary` |
| the pre-commit hooks on every file | `mise run hooks` | `pre-commit hooks (prek)` |
| Conventional Commits on a pull request's commits | `cog check` | `conventional commits (cog check)` |
| the documentation builds with no broken link, every page in the nav, front matter on every page | `mise run docs:check` | `docs` |
| the judge shaping its own pull request, no key | `cargo run -- explain --rubric mr-hygiene` | `explain on this pull request` |

`mise run ci` runs the first five. A change to `Cargo.toml` or
`Cargo.lock` also runs `deny`, `msrv` and `hack`; a change to the
`Dockerfile`, `scripts/image-context.sh` or a workflow also runs the musl
build and, where Docker runs, `mise run image`; a change under `docs/`
runs `docs:check`.

## Hooks

`mise run setup` installs the git hooks through prek
(`.pre-commit-config.yaml`): whitespace and file checks, `cargo fmt
--check`, `cargo clippy`, and `cog verify` on every commit message. `prek
run --all-files` runs them by hand.

## Documentation

`docs/` follows Diátaxis: `start/` tutorials, `guides/` how-to,
`reference/` facts, `concepts/` explanation, `project/` what is about
the repository. Every page, and every Markdown file in the repository,
carries front matter: `title`, `description` (one sentence saying what
the page answers), `status` (`current`, `draft`), `last_reviewed` (ISO
date) and `tags`; `scripts/check-frontmatter.sh` holds them to it. A new
page goes in `mkdocs.yml`'s nav; `mkdocs build --strict` refuses a page
the nav lacks and a link that does not resolve. `mise run docs:serve`
previews the site. The README is for a reader deciding whether to use
the binary; anything a user needs after that decision lives under
`docs/`.

## Commits and releases

Commits are Conventional Commits (`feat:`, `fix:`, `build:`, `docs:`,
`chore(deps):`, ...), one concern per commit. [Releasing](releasing.md)
is the runbook.

## The Claude Code harness

`.claude/` makes a Claude Code session work the way a contributor does:

- Agents: `contract-reviewer` (the verdict file against the gate's
  validation, the `.jud` documents against the reader, the platform
  variables), `rubric-author` (a rubric in its three files),
  `test-writer`, `docs-writer`, `refactor-scout`, `pr-shepherd` (opens
  the pull request and reads its checks). Reviewers report, writers edit.
- Skills: `pr-ready` (the reviewers a diff needs, the gate, the
  hand-off), `steward` (an open pull request: a merge commit to bring
  `main` in, Dependabot batches, no checks meaning a conflict), `rubric`
  (a rubric's lifecycle).
- Hooks: Rust files are formatted after every edit, whether through the
  editor tools or a shell command; a Bash guard denies pushes to `main`,
  committing `.env`, pushing a tag, `docker push`, `gh release` and
  `cargo publish`.
- Plugins (`.claude/settings.json`): `jud` from `chussenot/judgment`
  (`/jud:rubric`, `/jud:cases`, `/jud:check`), `typesafe` from
  `typesafe-ai/skills` (the skill to load before writing a question),
  `ponytail` from `DietrichGebert/ponytail` pinned to `v5.2.0`, the
  less-code discipline every change is held to.

## Cloud sessions

A Claude Code cloud session starts from a fresh container whose sandbox
cannot list versions through the backends `mise.toml` names.
`.claude/hooks/session-start.sh`, registered in `.claude/settings.json`
and run only there (`CLAUDE_CODE_REMOTE`), makes the container the
checkout above describes: mise from its release tarball; `MISE_ENV=cloud`
so `mise.cloud.toml` supplies the same tools at the same versions from
pinned release URLs (prek, cocogitto, cargo-deny, cargo-hack; rust stays
rustup's, from `rust-toolchain.toml`); the musl target and musl-tools;
`mise install` and `mise run setup`; the plugins through
`scripts/claude-plugins.sh`, since a cloud session never installs what
the settings file declares; `jud` with cargo when
`CI_JUDGE_SESSION_JUD=1`; a warm build of every test target
(`CI_JUDGE_SESSION_SKIP_BUILD=1` skips it); and a probe of the hosts the
work depends on, handing the session the blocked ones. The environment's
allowed domains have to include `index.crates.io`, `static.crates.io`,
`static.rust-lang.org`, `docs.rs`, `github.com`,
`release-assets.githubusercontent.com`, `objects.githubusercontent.com`,
`ghcr.io`, and `api.typesafe.ai` with `docs.typesafe.ai` for a real run.
A version bump in `mise.toml` is a bump in `mise.cloud.toml` too. The
container is snapshotted after the hook, so later sessions pay only for
what changed.
