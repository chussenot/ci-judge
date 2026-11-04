---
title: Container image
description: What is inside ghcr.io/chussenot/ci-judge, the user it runs as, the paths it reads, the tags a release pushes, and how its provenance is verified.
status: current
last_reviewed: 2026-11-04
tags: [ci-judge, container, ghcr, reference]
---

# Container image

`ghcr.io/chussenot/ci-judge` is the release binary as a container image.
[Run in a container](../guides/container.md) is the procedure; this page
is what the image is.

## Contents

`FROM scratch`: the statically linked binary, a CA bundle, and a
`passwd` and `group` declaring one user. No shell, no libc, no package
manager, no `git`.

| | |
|---|---|
| binary | `/usr/local/bin/ci-judge`, the entrypoint; the arguments are ci-judge's |
| CA bundle | `/etc/ssl/certs/ca-certificates.crt`, named by `SSL_CERT_FILE`, so the backend and the platform API verify |
| user | `65532:65532`, the uid distroless images call `nonroot`; never root |
| home | `HOME=/home/ci-judge`, so the jud configuration file is `/home/ci-judge/.config/jud/config.yaml` |
| working directory | `/work`; a mount there is where relative paths resolve |
| platforms | `linux/amd64` and `linux/arm64`, one manifest list |
| labels | `org.opencontainers.image.version`, `revision`, `source`, `licenses` |

The binary is the release asset's, copied in byte for byte by the
release workflow (`scripts/image-context.sh --dist`), never built in the
`Dockerfile`; a `docker build .` from a plain checkout fails for want of
that context. `mise run image` fills it from a local musl build instead.

## Tags

Every release pushes the version (`0.1.0`), its major.minor (`0.1`) and
`latest`. A digest names one build for good:

```sh
docker image inspect ghcr.io/chussenot/ci-judge:0.1.0 --format '{{index .RepoDigests 0}}'
```

## Provenance

The image carries a build-provenance attestation signed by GitHub for the
workflow run that built it:

```sh
gh attestation verify oci://ghcr.io/chussenot/ci-judge:0.1.0 --repo chussenot/ci-judge
```

exits 0 when the image was produced by this repository's release
workflow. The release binaries carry the same attestation.

## What it cannot do

- Run the `diff` source: no `git`. Collect the diff as an artifact.
- Run as a CI job's image where the job needs a shell: copy the binary
  into a job image of your own ([Run in GitLab CI](../guides/gitlab.md)).
- Write anywhere but a mounted directory the user can write.
