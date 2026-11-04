#!/usr/bin/env bash
# The Claude Code plugins .claude/settings.json enables, installed through the
# `claude` command at user scope: a laptop's Claude Code installs them from
# the settings file once the workspace is trusted, a cloud session never
# does, so .claude/hooks/session-start.sh runs this. `install` is idempotent;
# `update` refreshes the marketplaces and moves jud and typesafe to their
# latest and ponytail to its pin.
#
#   scripts/claude-plugins.sh install      # mise run plugins:install
#   scripts/claude-plugins.sh update       # mise run plugins:update
set -euo pipefail

PONYTAIL_REF="v5.2.0"   # moved together with .claude/settings.json

log() { printf '[claude-plugins] %s\n' "$*" >&2; }

if ! command -v claude >/dev/null 2>&1; then
  log "no \`claude\` on PATH; the plugins load from .claude/settings.json on a trusted laptop"
  exit 0
fi

marketplace() {
  # $1 name, $2 source (owner/repo or path), $3 ref (optional)
  local name="$1" source="$2" ref="${3:-}"
  if claude plugin marketplace list 2>/dev/null | grep -q "^$name\b\|\"$name\""; then
    return 0
  fi
  if [ -n "$ref" ]; then
    claude plugin marketplace add "$source" --name "$name" --ref "$ref" >/dev/null 2>&1 \
      || claude plugin marketplace add "$source" >/dev/null 2>&1 \
      || log "could not add marketplace $name ($source)"
  else
    claude plugin marketplace add "$source" >/dev/null 2>&1 \
      || log "could not add marketplace $name ($source)"
  fi
}

install_plugin() {
  # $1 plugin@marketplace
  claude plugin install "$1" --scope user >/dev/null 2>&1 \
    || claude plugin install "$1" >/dev/null 2>&1 \
    || log "could not install $1 (already installed, or the marketplace is unreachable)"
}

case "${1:-install}" in
  install)
    marketplace judgment chussenot/judgment
    marketplace typesafe-ai typesafe-ai/skills
    marketplace ponytail DietrichGebert/ponytail "$PONYTAIL_REF"
    install_plugin jud@judgment
    install_plugin typesafe@typesafe-ai
    install_plugin ponytail@ponytail
    log "$(claude plugin list 2>/dev/null | grep -E 'jud|typesafe|ponytail' | tr '\n' ';' || echo 'plugin list unavailable')"
    ;;
  update)
    claude plugin marketplace update >/dev/null 2>&1 || log "marketplace update failed"
    claude plugin update jud@judgment >/dev/null 2>&1 || log "jud not updated"
    claude plugin update typesafe@typesafe-ai >/dev/null 2>&1 || log "typesafe not updated"
    claude plugin install "ponytail@ponytail" --scope user >/dev/null 2>&1 || true
    log "updated; a session that is already open sees it after /reload-plugins"
    ;;
  *)
    echo "usage: $0 install|update" >&2
    exit 2
    ;;
esac
