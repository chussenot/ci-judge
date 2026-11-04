#!/usr/bin/env bash
# Every Markdown file in the repository opens with YAML front matter holding
# title, description, status, last_reviewed and tags, so a page's line in
# the index and its review date are in the page itself. Claude Code's own
# agent and skill files have their own front matter (name, description) and
# are checked for that instead.
#
#   scripts/check-frontmatter.sh            # the whole repository
#   scripts/check-frontmatter.sh docs       # one tree
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
status=0
while IFS= read -r file; do
  case "$file" in
    ./target/*|./.git/*|./node_modules/*) continue ;;
  esac
  if [ "$(head -n1 "$file")" != "---" ]; then
    echo "$file: no front matter"; status=1; continue
  fi
  # The block between the first and second `---`.
  block=$(awk 'NR==1{next} /^---$/{exit} {print}' "$file")
  case "$file" in
    ./.claude/agents/*|./.claude/skills/*) keys="name description" ;;
    *) keys="title description status last_reviewed tags" ;;
  esac
  for key in $keys; do
    if ! printf '%s\n' "$block" | grep -Eq "^$key:[[:space:]]*[^[:space:]]"; then
      echo "$file: front matter lacks $key"; status=1
    fi
  done
  if printf '%s\n' "$block" | grep -Eq '^last_reviewed:' \
     && ! printf '%s\n' "$block" | grep -Eq '^last_reviewed:[[:space:]]*[0-9]{4}-[0-9]{2}-[0-9]{2}$'; then
    echo "$file: last_reviewed is not an ISO date"; status=1
  fi
done < <(git ls-files -co --exclude-standard -- "${1:-.}" | grep '\.md$' | sed 's|^|./|' | sort)
[ "$status" -eq 0 ] && echo "front matter ok"
exit "$status"
