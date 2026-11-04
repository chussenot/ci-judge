---
title: Changelog
description: Every notable change to ci-judge, by version, in the Keep a Changelog format.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, changelog]
---

# Changelog

All notable changes to ci-judge are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- The documentation under `docs/`, built by mkdocs (`mise run docs:check`,
  `mise run docs:serve`): install and first run, guides for GitLab,
  GitHub, rubrics, shadow mode and the container, reference pages for
  the command line, configuration, the manifest, sources and reducers,
  the verdict file, the gate, the bundled rubrics and the image, the
  concepts behind the design, and the project pages; front matter on
  every Markdown file, held by `scripts/check-frontmatter.sh` and the
  `docs` CI job.

### Changed

- A finding's description no longer ends with `(p=…, threshold …)`: the
  gates fingerprint `path:line:description`, and the number moved between
  runs, so every finding showed as new. A `finding` template that wants
  the number uses `{p}` and `{threshold}`; the probability stays in
  `ci_judge.answers`
  ([ADR 0001](docs/project/decisions/0001-fit-the-judge-gate-block.md)).
- The README states why the binary exists and points at the docs; what
  it held moved to the pages above. `CONTRIBUTING.md` points at
  `docs/project/`.

### Fixed

- The `mr` source reads the whole description from the merge request API
  when GitLab cut `CI_MERGE_REQUEST_DESCRIPTION` at 2700 characters
  (`CI_MERGE_REQUEST_DESCRIPTION_IS_TRUNCATED`); a failed read is a
  collection failure, not a judgment of a cut text.
- `examples/gitlab-ci.yml` and the GitLab guide swallow `ci-judge run`'s
  status, since a failing `before_script` skipped the block's script, and
  restate the block's artifact paths, which `extends` had replaced with
  `recordings/`.

## [0.1.0] - 2026-11-04

### Added

- `ci-judge run`: collect a merge request's evidence (Terraform plan, JUnit
  reports, the diff, the request's own fields, failed job traces on GitLab,
  files, variables), redact secrets, shape one request per evidence item and
  one global request, ask a `.jud` rubric through the `judgment` crate, and
  write `judge-gate.verdict.json` in the contract the `judge-gate` block
  validates.
- `ci-judge gate`: the decision step for GitHub workflows and anywhere the
  GitLab block is not available: shadow, advise, warn, block and route
  modes, the override label, a pull request comment or merge request note
  updated in place, GitHub annotations and step summary, `GITHUB_OUTPUT`,
  a Code Quality report and a dotenv, exit statuses 0/1/2/3.
- `ci-judge collect` and `ci-judge explain`: see what the model would see,
  with no call.
- `ci-judge check` and `ci-judge eval`: hold a rubric, its manifest, cases
  and recordings together; grade recordings against labels with the crate's
  accuracy, Brier and calibration metrics; exit 3 below a promotion bar.
- Shadow mode: `--record DIR` keeps every answer as a `.jud` Recording,
  `--cases-out FILE` grows an unlabelled Cases document for reviewers.
- Three bundled rubrics with manifests and synthetic cases: `infra-plan`,
  `mr-hygiene`, `failed-job-triage`.
- A composite GitHub Action (`action.yml`) and a release workflow that
  publishes static `x86_64` and `aarch64` musl binaries and a macOS arm64
  one, with checksums and build provenance.
- The container image `ghcr.io/chussenot/ci-judge`: `FROM scratch`, the
  release binary byte for byte, uid 65532, `linux/amd64` and `linux/arm64`,
  tags for the version, its major.minor and `latest`, provenance attested;
  built by the release workflow from `Dockerfile` and
  `scripts/image-context.sh`, or locally with `mise run image`.
- The Claude Code harness under `.claude/`: six agents (`contract-reviewer`,
  `rubric-author`, `test-writer`, `docs-writer`, `refactor-scout`,
  `pr-shepherd`), three skills (`pr-ready`, `steward`, `rubric`), the
  format-after-edit and Bash guard hooks, and a cloud-session bootstrap
  (`session-start.sh` with `mise.cloud.toml`); `CLAUDE.md` and
  `CONTRIBUTING.md`; prek hooks (`.pre-commit-config.yaml`) and the CI jobs
  that run them and `cog check`.
- `mise.toml` with the pinned toolchain and tools (rust 1.97.0, cargo-deny,
  cargo-hack, cocogitto, the `jud` command line) and the gates as tasks;
  `deny.toml` for licences, advisories, bans and sources; `cog.toml` for
  Conventional Commits and the release bump; Dependabot for crates and
  actions.
