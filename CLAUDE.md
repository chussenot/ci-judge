---
title: Claude Code instructions
description: What a Claude Code session needs to know to work in this repository; the start, the rules that are easy to get wrong, and the harness.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, claude-code]
---

# ci-judge

Merge-request evidence to a calibrated verdict file for a CI gate, as a
Rust binary on the `judgment` crate: collectors and reducers, one request
per evidence item plus one global request, a `.jud` rubric asked through a
System One backend, the verdict file a gate validates and decides on.
The README says why; `docs/` says how (`docs/index.md` is the map,
`docs/project/contributing.md` how to work here).

## Start here

- `mise install` once, then `mise run setup` (git hooks, `cargo fetch`) and
  `mise run ci` (the local gates, CI order; CI also runs `deny`, `msrv` and
  the musl build, which a dependency or workflow change needs too).
  `mise tasks` lists the rest.
- The three layers: the `judgment` crate answers typed questions
  (https://github.com/chussenot/judgment, `.jud` format in
  `docs/reference/jud-format.md` there); this binary shapes evidence into
  requests and writes `judge-gate.verdict.json`; a gate (a GitLab
  `judge-gate` block, or `ci-judge gate` and `action.yml` on GitHub) turns
  the file into exit 0/1/2/3.
- `CHANGELOG.md` is Keep a Changelog written by hand, its section headed
  with the version before `mise run release`, which refuses without it;
  0.x means a minor release may break.

## Rules that are easy to get wrong

- Load the `typesafe:typesafe-ai` skill before writing or rewording a
  question: the primitives, their `criteria` and their limits are
  TypeSafe's. The `jud` plugin's `/jud:rubric`, `/jud:cases` and
  `/jud:check` know the format; the `rubric` skill knows this repository's
  lifecycle around it.
- The `.jud` documents under `rubrics/` are read by another tool. Never
  add a field to one: the reader refuses unknown fields by path.
  Everything CI-specific (sources, request shapes, severities, finding
  templates, promotion bars) lives in `manifest.yml` beside the rubric
  (`src/manifest.rs`).
- The granularity rule: one request per evidence item, the item under one
  key, the question gated with `when: <item key>`; a blocker is always per
  item, so its finding has a file and a line. A global question carries
  no path and may not block. `requests::build` adds a key named after a
  global shape so `when: <request name>` works; item requests carry no
  item source at all.
- The verdict file's first four fields are the gate's contract
  (`verdict::validate_loose` is the gate's own validation); everything
  else sits under `ci_judge`. `run` writes no file when it cannot judge,
  so the gate sees a gate error, never a pass; the GitLab job swallows
  `run`'s status (`|| echo`, `examples/gitlab-ci.yml`) so the block's
  script runs at all. Exit statuses are `src/error.rs`: 0/1/2/3 with
  the meanings the README states. A finding's description carries no
  number unless its template asks (`{p}`): the gates fingerprint on it.
  A decision that changes the seam gets an ADR under
  `docs/project/decisions/`.
- Nothing here generates text. A finding is a template filled from the
  item and the probability; a summary is counts. A System One backend
  returns probabilities; thresholds and bars are policy in the rubric and
  the manifest.
- Only the bundled catalog (`rubrics/`, compiled into the binary) may hold
  `critical` and `blocker` severities; a repo-local manifest is refused
  above `major` unless `--allow-local-blockers`.
- `run` refuses a model other than the rubric's `tuning.model` unless
  `--allow-untuned`; a rubric without a `tuning` block runs with a note.
  Never write a `tuning` block by hand: `jud tune` on recordings does.
- Tests never call a real API: `Fake` and recordings for the model
  (`tests/end_to_end.rs`), fixtures on disk for collectors and reducers;
  the platform clients have no tests yet and get a local mock when they
  do, never the real API.
- The crate is generic. No organisation, registry, hostname, secret store
  or block implementation is named anywhere: the GitLab decision layer is
  "a `judge-gate` block". `grep -rniE 'believe|blv-|vault|shd\.' .` must
  find nothing. Secrets live in `.env` (gitignored, loaded by mise); never
  commit one.
- Backend settings are the `jud` command's: `TYPESAFE_API_KEY`,
  `TYPESAFE_BASE_URL`, then `~/.config/jud/config.yaml`, then the defaults
  (`src/config.rs`); the key is never printed. The image is `FROM scratch`
  and carries no `git`, so the `diff` source inside it needs an artifact.
- Documentation lives under `docs/` (Diátaxis: `start/`, `guides/`,
  `reference/`, `concepts/`, `project/`), every Markdown file in the
  repository carries front matter (`title`, `description`, `status`,
  `last_reviewed`, `tags`; `scripts/check-frontmatter.sh`), a new page
  goes in `mkdocs.yml`'s nav, and `mise run docs:check` builds the site
  strictly. The README is for deciding whether to use the binary;
  everything after that decision is a docs page.
- Commits are Conventional Commits (`cog verify` runs on every message
  through prek; CI checks a pull request's commits). A release is `cog
  bump --auto` on `main`, never a hand-edited version, and the tag push
  runs `release.yml`, which builds the binaries and pushes the image; the
  Bash guard denies pushing a tag, `docker push`, `gh release` and `cargo
  publish`, all the release owner's.

## Harness

- Subagents in `.claude/agents/`: `contract-reviewer` (the verdict file
  against the gate's validation, the `.jud` documents against the reader,
  the platform variables), `rubric-author` (a rubric in its three-file
  shape), `test-writer` (tests in this crate's Fake and fixture style),
  `docs-writer` (this documentation, the why as well as the how),
  `refactor-scout` (dead code and duplication, ranked), `pr-shepherd`
  (open the PR, read its CI checks). Reviewers report, writers edit; run
  the reviewers on a diff before opening a pull request.
- Skills in `.claude/skills/`: `pr-ready` picks the reviewers a diff needs,
  runs the gate and hands the branch to `pr-shepherd`; `steward` holds the
  rules for an open pull request (a merge commit to bring `main` in,
  Dependabot batches, no checks meaning a conflict); `rubric` is the
  lifecycle of a rubric from draft to a blocking gate.
- Plugins (`.claude/settings.json`): `jud` from `chussenot/judgment`,
  `typesafe` from `typesafe-ai/skills`, `ponytail` from
  `DietrichGebert/ponytail` pinned to `v5.2.0`. Ponytail's discipline
  applies to every change: no code the task does not need, the standard
  library and what the crate already has before a new dependency or
  abstraction, the checks kept whole. A deliberate shortcut is marked
  `ponytail:` in a comment.
- Hooks in `.claude/hooks/`: Rust files are formatted after every edit,
  whether through Edit and Write or through a Bash command
  (`tidy-after-bash.sh` asks git what changed); a Bash guard denies pushes
  to `main`, committing `.env`, pushing a tag, `docker push`, `gh release`
  and `cargo publish`. In a cloud session `session-start.sh` installs the
  tools (`mise.cloud.toml`, pinned release URLs), the musl target and
  musl-tools, the git hooks, the three plugins (`scripts/claude-plugins.sh`,
  since a cloud session never installs what the settings file declares),
  `jud` on request (`CI_JUDGE_SESSION_JUD=1`), warms the build and probes
  the hosts the work depends on, handing the session the list of blocked
  ones. A background command writes its output to the scratchpad through
  `tail`: the task output filesystem is small.
