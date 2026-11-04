#!/usr/bin/env sh
# PostToolUse (Bash): the Edit and Write hook formats a Rust file after an
# edit, but most edits in a long session go through Bash (a python heredoc,
# sed -i, cat >), which that hook never sees. This hook asks git what the
# command left modified and runs rustfmt on every modified Rust file.
# Unmerged files are left alone (rustfmt would choke on conflict markers),
# and so is everything outside a git checkout. Cost when nothing changed:
# one `git status`.
set -eu
root=${CLAUDE_PROJECT_DIR:-.}
cd "$root" 2>/dev/null || exit 0
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || exit 0

status=$(git status --porcelain --untracked-files=all -- '*.rs' 2>/dev/null || true)
[ -n "$status" ] || exit 0

paths=$(printf '%s\n' "$status" | grep -v '^UU\|^AA\|^DD\|^.D\|^D' | sed 's/^...//; s/^.* -> //')
rs=$(printf '%s\n' "$paths" | grep '\.rs$' || true)
if [ -n "$rs" ] && command -v rustfmt >/dev/null 2>&1; then
  printf '%s\n' "$rs" | while IFS= read -r f; do
    [ -f "$f" ] && rustfmt --edition 2024 "$f" >/dev/null 2>&1 || true
  done
fi
exit 0
