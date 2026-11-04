# ci-judge as a container image: ghcr.io/chussenot/ci-judge.
#
#   docker run --rm -v "$PWD:/work" -e TYPESAFE_API_KEY ghcr.io/chussenot/ci-judge run --rubric infra-plan
#
# FROM scratch: the image is the statically linked musl binary and nothing
# else, no shell, no libc, no package manager, so there is no base image to
# patch and nothing to escalate into. The binary is not built here: it is
# the one the release workflow built on a native runner, ran and checksummed
# as a release asset, copied in from a build context the workflow fills from
# those assets, one binary per platform under bin/<TARGETOS>/<TARGETARCH>/.
# So the image carries, byte for byte, the binary a runner downloads, and a
# `docker build .` from a plain checkout fails for want of that context:
# scripts/image-context.sh fills it from a release or a local build.
#
# A run needs no network for `check`, `explain`, `collect`, `gate` on a
# local platform, or `--replay`; a run against a backend reads
# TYPESAFE_API_KEY and TYPESAFE_BASE_URL from the environment, as outside a
# container (`-e TYPESAFE_API_KEY`). The jud configuration file is read from
# /home/ci-judge/.config/jud/config.yaml, so a host file is mounted there.
# The `diff` source runs `git`, which the image does not carry: collect the
# diff as an artifact (`git diff ... > changes.diff`, reduce: diffhunks) or
# run the binary from the release asset in a job that has git.
FROM scratch

ARG TARGETOS
ARG TARGETARCH

# The directories first, with their own modes: a COPY --chmod into a
# directory that does not exist yet creates it with the file's mode, and
# 444 on /etc or /etc/ssl/certs has no execute bit, so a non-root user can
# read nothing under them. image/tree holds the empty tree with 755
# directories (scripts/image-context.sh).
COPY --chmod=755 image/tree/ /

# A user that is not root, declared here since there is no /etc to add one
# to: uid and gid 65532, the one distroless images use for "nonroot".
COPY --chmod=444 image/passwd /etc/passwd
COPY --chmod=444 image/group /etc/group
COPY --chmod=555 bin/${TARGETOS}/${TARGETARCH}/ci-judge /usr/local/bin/ci-judge

# TLS roots for the model backend and the GitLab or GitHub API: reqwest's
# rustls feature verifies through the platform store, which reads
# /etc/ssl/certs/ca-certificates.crt. The bundle is the one the workflow's
# runner carries (Ubuntu's ca-certificates), copied into the context by
# scripts/image-context.sh, and SSL_CERT_FILE names it explicitly.
COPY --chmod=444 image/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt

# HOME lets the jud configuration file be found; /work is where a checkout,
# an artifact and a rubric are mounted.
ENV HOME=/home/ci-judge
WORKDIR /work
USER 65532:65532

ENTRYPOINT ["/usr/local/bin/ci-judge"]
