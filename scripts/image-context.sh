#!/usr/bin/env bash
# Fill the build context of the Dockerfile: the binaries per platform, the
# non-root user, the empty directory tree and the CA bundle.
#
#   scripts/image-context.sh --dist DIR    # from release assets under DIR:
#                                          #   ci-judge-x86_64-unknown-linux-musl, ci-judge-aarch64-unknown-linux-musl
#   scripts/image-context.sh --local       # from this checkout's own musl build (amd64 only)
#
# The Dockerfile copies bin/<TARGETOS>/<TARGETARCH>/ci-judge, so a
# `docker buildx build --platform linux/amd64,linux/arm64` needs both
# binaries; `--local` gives one and builds the host's platform only.
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
cd "$here"

mode="${1:-}"
dist="${2:-dist}"

rm -rf bin image/tree image/passwd image/group image/ca-certificates.crt
mkdir -p bin/linux/amd64 bin/linux/arm64 image

case "$mode" in
  --dist)
    for pair in "x86_64-unknown-linux-musl:amd64" "aarch64-unknown-linux-musl:arm64"; do
      target="${pair%%:*}"; arch="${pair##*:}"
      asset="$dist/ci-judge-$target"
      if [ -f "$asset" ]; then
        install -m 555 "$asset" "bin/linux/$arch/ci-judge"
        if [ -f "$asset.sha256" ]; then
          (cd "$dist" && echo "$(cut -d' ' -f1 "ci-judge-$target.sha256")  ci-judge-$target" | sha256sum -c -)
        fi
        echo "bin/linux/$arch/ci-judge from $asset"
      else
        echo "no $asset: linux/$arch is left out" >&2
        rmdir "bin/linux/$arch"
      fi
    done
    ;;
  --local)
    built="target/x86_64-unknown-linux-musl/release/ci-judge"
    [ -f "$built" ] || { echo "no $built: run \`mise run build:musl\` first" >&2; exit 2; }
    install -m 555 "$built" bin/linux/amd64/ci-judge
    rmdir bin/linux/arm64
    echo "bin/linux/amd64/ci-judge from $built (local build; arm64 left out)"
    ;;
  *)
    echo "usage: $0 --dist [DIR] | --local" >&2
    exit 2
    ;;
esac

# The empty tree with 755 directories: /etc, /etc/ssl/certs, /home/ci-judge,
# /work, /usr/local/bin. The binary writes only where a command is told to
# (a mounted /work), so the image needs no writable directory of its own.
mkdir -p image/tree/etc/ssl/certs image/tree/home/ci-judge image/tree/work image/tree/usr/local/bin
chmod 755 image/tree/home/ci-judge image/tree/work

# One non-root user, uid and gid 65532 as distroless calls "nonroot".
printf 'root:x:0:0:root:/root:/sbin/nologin\nci-judge:x:65532:65532:ci-judge:/home/ci-judge:/sbin/nologin\n' > image/passwd
printf 'root:x:0:\nci-judge:x:65532:\n' > image/group

# The runner's CA bundle.
for candidate in /etc/ssl/certs/ca-certificates.crt /etc/pki/tls/certs/ca-bundle.crt /etc/ssl/cert.pem; do
  if [ -f "$candidate" ]; then
    cp "$candidate" image/ca-certificates.crt
    echo "image/ca-certificates.crt from $candidate"
    break
  fi
done
[ -f image/ca-certificates.crt ] || { echo "no CA bundle found on this host" >&2; exit 2; }

echo "context ready: $(find bin -type f | tr '\n' ' ')"
