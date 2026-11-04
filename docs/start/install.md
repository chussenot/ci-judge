---
title: Install
description: How to get ci-judge onto a CI runner, a laptop or into a job image, from a release binary with its checksum, from source, or as the container image.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, install, tutorial]
---

# Install

ci-judge is one static binary. A runner downloads it, a laptop installs
it once, a job that runs inside an image copies it in. Every route below
ends with the same bytes, which is why the checksum matters.

## A release binary

Each release publishes `ci-judge-<target>` and its `.sha256` for
`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl` and
`aarch64-apple-darwin`. The Linux binaries are statically linked and run
on any distribution, Alpine included.

```sh
v=0.1.0
t=x86_64-unknown-linux-musl
curl -fsSLO "https://github.com/chussenot/ci-judge/releases/download/v$v/ci-judge-$t"
curl -fsSLO "https://github.com/chussenot/ci-judge/releases/download/v$v/ci-judge-$t.sha256"
sha256sum -c "ci-judge-$t.sha256"
install -m 755 "ci-judge-$t" ~/.local/bin/ci-judge
ci-judge --version
```

The checksum line fails the install when the download is not the
release's; a runner that skips it trusts the network instead of the
release. The release assets also carry a GitHub build-provenance
attestation: `gh attestation verify ci-judge-$t --repo chussenot/ci-judge`.

## From source

```sh
cargo install --git https://github.com/chussenot/ci-judge --locked
```

Needs Rust 1.91 or later. `--locked` builds the dependency versions the
release was tested with.

## The container image

```sh
docker run --rm ghcr.io/chussenot/ci-judge:0.1.0 --version
```

`FROM scratch`, the release binary and a CA bundle, no shell, no `git`.
[Run in a container](../guides/container.md) says what that changes;
[Container image](../reference/container-image.md) says what is inside.

## On a runner

GitLab: the install lines go in a job's `before_script`, with the
checksum, as [Run in GitLab CI](../guides/gitlab.md) shows. GitHub: the
composite action downloads and verifies the binary for the runner's
architecture, as [Run in GitHub Actions](../guides/github.md) shows.

## Next

[First run](first-run.md): a judgment of a fixture with no key, then
with one.
