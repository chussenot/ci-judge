---
title: Run in a container
description: How to run ci-judge as ghcr.io/chussenot/ci-judge, how a checkout, an artifact and a rubric reach it, what the non-root user without a shell changes, and why the diff source needs an artifact inside it.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, container, docker, how-to]
---

# Run in a container

**You will** judge a directory with nothing installed but Docker.
**Prerequisites:** Docker or another OCI runtime; a backend.

```sh
docker run --rm -v "$PWD:/work" -e TYPESAFE_API_KEY \
  ghcr.io/chussenot/ci-judge:0.1.0 run --rubric infra-plan --platform local
```

That line is the whole model: `-v "$PWD:/work"` puts the directory
where the binary looks for files, `-e TYPESAFE_API_KEY` copies the key
from your shell without putting it on a command line, and everything
after the image name is ci-judge's own command line.

## Files reach it through `/work`

The working directory is `/work`. Relative paths in a manifest
(`tfplan.json`, `docs/runbook.md`) resolve there. Mount read-only for
`check`, `explain`, `collect` and `gate --platform local`; `run` writes
the verdict file, and with `--record` the recordings, so it needs a
writable mount.

```sh
docker run --rm -v "$PWD/rubrics:/work:ro" ghcr.io/chussenot/ci-judge:0.1.0 check infra-plan
docker run --rm -v "$PWD:/work:ro" ghcr.io/chussenot/ci-judge:0.1.0 explain --rubric my-rubric --platform local
```

The binary runs as uid 65532. A mounted file must be readable by that
user (644 is the usual and needs nothing); a directory it writes to must
be writable by it, or run the container as yourself
(`--user "$(id -u):$(id -g)"`).

## No `git`

The image is `FROM scratch`: the binary, a CA bundle, a `passwd`. No
shell, no `git`. The `diff` source runs `git`, so inside the image it
fails. Two ways round it:

- Collect the diff in the job that has `git` and hand it over as an
  artifact: `git diff --merge-base "$BASE" HEAD > changes.diff`, and in
  the manifest `{ key: diff, source: artifact, path: changes.diff, reduce: diffhunks }`.
- Run the release binary, not the image, in a job that has `git`
  ([Install](../start/install.md)).

## Reaching a backend

The settings are the `jud` command's, inside the container as outside:
the environment, then `/home/ci-judge/.config/jud/config.yaml`, then
the defaults ([Configuration](../reference/configuration.md)).

- The hosted API: `-e TYPESAFE_API_KEY`.
- Another server: `-e TYPESAFE_BASE_URL=http://... -e TYPESAFE_API_KEY=any`.
- A model on the host: `--network host` and a mounted configuration
  file naming it, `-v "$PWD/config.yaml:/home/ci-judge/.config/jud/config.yaml:ro"`.
- No server: `--replay recordings` answers from a mounted directory.

## In CI

A CI system that runs a job *inside* an image needs a shell the scratch
image lacks. Build a job image that copies the binary out of this one
([Run in GitLab CI](gitlab.md)). On GitHub the action downloads the
binary instead; the image is for a shell, a cron or a Kubernetes job.

## Pin it

```sh
docker image inspect ghcr.io/chussenot/ci-judge:0.1.0 --format '{{index .RepoDigests 0}}'
gh attestation verify oci://ghcr.io/chussenot/ci-judge:0.1.0 --repo chussenot/ci-judge
```
