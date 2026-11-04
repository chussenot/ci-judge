# Changelog

All notable changes to ci-judge are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

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
