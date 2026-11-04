#!/bin/bash
# SessionStart hook for a Claude Code cloud session: make a fresh container the
# contributor's machine that `mise install` and `mise run setup` describe, so
# `mise run ci`, the git hooks and the musl build work from the first command.
# Runs only in the cloud (CLAUDE_CODE_REMOTE=true); a laptop keeps its own setup.
# Idempotent: every step is skipped or a no-op when already done, and the
# container's state is snapshotted after the hook, so later sessions pay
# nothing for what is already installed or built.
#
# What it does, and why each step exists (CONTRIBUTING.md, "Cloud sessions"):
#   1. mise from its GitHub release (mise.run is not always reachable).
#   2. MISE_ENV=cloud, so mise.cloud.toml replaces the backends the sandbox cannot
#      use with pinned release URLs; the tools go on PATH as plain binaries.
#   3. The toolchain rust-toolchain.toml names, through rustup, as CI does,
#      plus the musl target and musl-tools for `mise run build:musl`.
#   4. `mise install`: prek, cocogitto, cargo-deny, cargo-hack, release binaries.
#   5. `mise run setup`: the git hooks (prek) and `cargo fetch`.
#   6. The Claude Code plugins (scripts/claude-plugins.sh): `jud` from
#      chussenot/judgment, `typesafe` from typesafe-ai/skills and `ponytail`
#      from DietrichGebert/ponytail at its pinned tag. A cloud session never
#      installs the plugins .claude/settings.json declares (that needs the
#      trust dialog a laptop shows), so the hook does, at user scope.
#   7. `jud`, the judgment crate's command line, with cargo when
#      CI_JUDGE_SESSION_JUD=1: minutes of build, so only on request.
#   8. A warm build of every test target, so the first `cargo test` in the
#      session is incremental (CI_JUDGE_SESSION_SKIP_BUILD=1 skips it).
#   9. A probe of the hosts the work depends on; the blocked ones reach the
#      session as context, not only the log.
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

MISE_VERSION="2026.10.1"
root="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"
cd "$root"

log() { printf '[session-start] %s\n' "$*" >&2; }

export MISE_ENV=cloud
export MISE_YES=1           # never prompt (trust, install)
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

# Every later shell of the session sees the same environment.
if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
  {
    echo 'export MISE_ENV=cloud'
    echo 'export MISE_YES=1'
    echo 'export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"'
  } >>"$CLAUDE_ENV_FILE"
fi

# 1. mise, pinned, from the release tarball.
if ! command -v mise >/dev/null 2>&1; then
  log "installing mise $MISE_VERSION"
  mkdir -p "$HOME/.local/bin"
  tmp=$(mktemp -d)
  curl -fsSL --retry 3 \
    "https://github.com/jdx/mise/releases/download/v${MISE_VERSION}/mise-v${MISE_VERSION}-linux-x64.tar.gz" \
    | tar -xz -C "$tmp"
  install -m 0755 "$tmp/mise/bin/mise" "$HOME/.local/bin/mise"
  rm -rf "$tmp"
fi
mise trust mise.toml >/dev/null 2>&1 || true
mise trust mise.cloud.toml >/dev/null 2>&1 || true
log "mise $(mise --version 2>/dev/null | head -1)"

# 3. The pinned toolchain, as CI installs it; rustup reads rust-toolchain.toml.
if ! command -v rustup >/dev/null 2>&1; then
  log "rustup is missing from the image; install it before this hook can continue" ; exit 1
fi
rustup toolchain install >/dev/null 2>&1 || rustup toolchain install
rustup component add rustfmt clippy >/dev/null 2>&1 || true
rustup target add x86_64-unknown-linux-musl >/dev/null 2>&1 || true
if ! command -v musl-gcc >/dev/null 2>&1; then
  if command -v apt-get >/dev/null 2>&1 && [ "$(id -u)" = "0" ]; then
    log "installing musl-tools for the static build"
    (apt-get install -y -q musl-tools >/dev/null 2>&1 || { apt-get update -q >/dev/null 2>&1 && apt-get install -y -q musl-tools >/dev/null 2>&1; }) \
      || log "musl-tools did not install; mise run build:musl will fail until it does"
  else
    log "no apt or not root: musl-tools not installed, mise run build:musl needs it"
  fi
fi
log "$(cargo --version)"

# 4. The tools (mise.toml, overridden by mise.cloud.toml), every one a pinned
#    release binary: nothing is compiled, so a fresh container pays seconds.
log "mise install"
mise install

# The tools as plain binaries on PATH, so git hooks and scripts find them
# without mise being activated in the shell that runs them.
for tool in prek cog cargo-deny cargo-hack; do
  if path=$(mise which "$tool" 2>/dev/null) && [ -n "$path" ]; then
    ln -sfn "$path" "$HOME/.local/bin/$tool"
  fi
done
log "$(prek --version 2>/dev/null), $(cog --version 2>/dev/null), $(cargo-deny --version 2>/dev/null)"

# 5. The git hooks and a fetched registry.
mise run setup

# 6. The Claude Code plugins, through the `claude` command the image carries;
#    idempotent, and a missing `claude` is logged, not fatal.
scripts/claude-plugins.sh install

# 7. jud, on request: a cargo build of the judgment crate's cli feature.
if [ "${CI_JUDGE_SESSION_JUD:-}" = "1" ]; then
  if ! command -v jud >/dev/null 2>&1; then
    log "installing jud (cargo install judgment --features cli)"
    cargo install judgment --version 0.16.1 --features cli --locked --quiet \
      || log "jud did not install; mise run jud:check is unavailable this session"
  fi
  log "jud $(jud --version 2>/dev/null || echo 'not installed')"
else
  log "jud skipped (CI_JUDGE_SESSION_JUD=1 installs it)"
fi

# 8. A warm build, so the first test run is incremental. Every target, nothing executed.
if [ "${CI_JUDGE_SESSION_SKIP_BUILD:-}" != "1" ]; then
  log "warming the build (cargo test --all-targets --no-run)"
  cargo test --all-targets --no-run --quiet
fi

# 9. The network. The environment's egress policy decides which hosts answer,
#    and a session used to learn that a host is blocked mid-task, from a
#    failed fetch. Each host below is one the work depends on.
probe() {
  blocked=""
  for h in "$@"; do
    code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 6 "https://$h/" 2>/dev/null || true)
    case "$code" in 000|"") blocked="$blocked $h" ;; esac
  done
  printf '%s' "$blocked"
}
hosts="index.crates.io static.crates.io static.rust-lang.org docs.rs github.com release-assets.githubusercontent.com objects.githubusercontent.com ghcr.io api.typesafe.ai docs.typesafe.ai"
blocked=$(probe $hosts)
if [ -n "$blocked" ]; then
  network="blocked by the egress policy:$blocked. The tests need nothing but crates.io; a real judge run needs api.typesafe.ai or the base URL in .env; the environment's allowed domains are the fix (CONTRIBUTING.md, Cloud sessions)."
else
  network="every host the work depends on answers (crates.io, rust-lang, docs.rs, GitHub releases, ghcr.io, TypeSafe API and docs)."
fi
log "network: $network"
log "done"

# The plugins installed above load at the next session start; this asks the
# session that is starting to re-scan skills and commands as well, and hands
# it the network probe as context.
context="[session-start] network: $network Background commands write their output to the scratchpad through tail: the task output filesystem is small and a full one loses the output."
printf '{"hookSpecificOutput":{"hookEventName":"SessionStart","reloadSkills":true,"additionalContext":%s}}\n' "$(printf '%s' "$context" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))')"
