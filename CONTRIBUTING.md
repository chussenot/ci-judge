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
| the pre-commit hooks on every file | `prek run --all-files` | `pre-commit hooks (prek)` |
| Conventional Commits on a pull request's commits | `cog check` | `conventional commits (cog check)` |
| the judge shaping its own pull request, no key | `cargo run -- explain --rubric mr-hygiene` | `explain on this pull request` |

`mise run ci` runs the first five. A change to `Cargo.toml` or
`Cargo.lock` also runs `deny`, `msrv` and `hack`; a change to the
`Dockerfile`, `scripts/image-context.sh` or a workflow also runs the musl
build and, where Docker runs, `mise run image`.

## Hooks

`mise run setup` installs the git hooks through prek
(`.pre-commit-config.yaml`): whitespace and file checks, `cargo fmt
--check`, `cargo clippy`, and `cog verify` on every commit message. `prek
run --all-files` runs them by hand.

## Commits and releases

Commits are Conventional Commits (`feat:`, `fix:`, `build:`, `docs:`,
`chore(deps):`, ...), one concern per commit. `mise run release:check`
says what the next release would be; `mise run release` runs `cog bump
--auto`, which bumps the version from the commits, writes the
`CHANGELOG.md` section and tags. The release owner pushes the tag
(`git push --follow-tags`); `.github/workflows/release.yml` then runs the
gates, builds the three binaries, publishes the GitHub release with
checksums and provenance, builds the image for `linux/amd64` and
`linux/arm64` from those binaries, pushes `ghcr.io/chussenot/ci-judge` with
the version, its major.minor and `latest`, and attests it. A
`workflow_dispatch` of the same workflow is a dry run that pushes nothing;
run one before the first tag of a release line.

## The container image

`Dockerfile` is `FROM scratch` and copies the binary from a build context
that `scripts/image-context.sh` fills, from release assets (`--dist`) or
from this checkout's musl build (`--local`). `mise run image` builds
`ghcr.io/chussenot/ci-judge:dev` locally and runs it; `mise run
image:check` mounts the rubrics read-only and runs `check` as uid 65532.
The image carries no shell and no `git`.

## Cloud sessions

A Claude Code cloud session starts from a fresh container, and its sandbox
cannot list versions through the backends `mise.toml` names
(api.github.com answers only for this repository).
`.claude/hooks/session-start.sh`, registered in `.claude/settings.json`,
makes the container the checkout above describes before the session's
first command, and only there (`CLAUDE_CODE_REMOTE`): it installs mise from
its release tarball, sets `MISE_ENV=cloud` so `mise.cloud.toml` supplies
the same tools at the same versions from pinned release URLs (prek,
cocogitto, cargo-deny, cargo-hack; rust stays rustup's, from
`rust-toolchain.toml`), adds the musl target and installs musl-tools, runs
`mise install` and `mise run setup`, installs the Claude Code plugins
(below), installs `jud` with cargo when `CI_JUDGE_SESSION_JUD=1` is set
(a build of the `judgment` crate's `cli` feature, minutes), warms the build
of every test target (`CI_JUDGE_SESSION_SKIP_BUILD=1` skips it), and
probes the hosts the work depends on, handing the session the list of
blocked ones. The environment's allowed domains have to include
`index.crates.io` and `static.crates.io` (cargo), `static.rust-lang.org`
(rustup), `docs.rs`, `github.com`, `release-assets.githubusercontent.com`
and `objects.githubusercontent.com` (the pinned tools and the plugins),
`ghcr.io` (the image), and `api.typesafe.ai` with `docs.typesafe.ai` for a
real judge run and the `typesafe` skill. A version bump in `mise.toml` is a
bump in `mise.cloud.toml` too. The container is snapshotted after the
hook, so later sessions pay only for what changed.

The plugins are the three `.claude/settings.json` enables: `jud` from
`chussenot/judgment` (the `/jud:rubric`, `/jud:cases` and `/jud:check`
commands and the `jud` skill), `typesafe` from `typesafe-ai/skills`
(the skill `CLAUDE.md` asks for before a question is written), and
[`ponytail`](https://github.com/DietrichGebert/ponytail), the less-code
discipline every change here is held to, pinned to a tag (`ref: v5.2.0`
in the settings file, `PONYTAIL_REF` in `scripts/claude-plugins.sh`, moved
together). A laptop's Claude Code installs all three from the settings file
once the workspace is trusted; a cloud session never does, so
`scripts/claude-plugins.sh install` runs them in through the `claude`
command at user scope, idempotently. The same script is `mise run
plugins:install` on a laptop, and `mise run plugins:update` refreshes the
marketplaces and moves `jud` and `typesafe` to their latest and `ponytail`
to its pin; a session that is already open sees an update after
`/reload-plugins`.
