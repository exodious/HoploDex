#!/usr/bin/env bash
# PreToolUse hook for Bash: a `gh pr create` (or `gh-axi pr create`) goes ahead only once the branch's
# HEAD has a pull request run of /ai-security-review and the PR body carries its lines.
#
# The run's record is saved, never committed (constitution: security findings
# are not kept in documents committed to the repository), by
# `node scripts/ai-security-review.mjs build --mode pr`, at
#   <git common dir>/ai-security-review/<branch, "/" as "__">.json
# with the reviewed `commit` and the `body` lines for "## Security review".
# Its upload to code scanning needs the PR's number, so it follows the PR's
# creation (`upload --pr <n>`); the skill does that, and this hook can't.
set -euo pipefail

input=$(cat)
command=$(jq -r '.tool_input.command // ""' <<<"$input")
cwd=$(jq -r '.cwd // empty' <<<"$input")
[[ -n "$cwd" ]] && cd "$cwd"

# Only `gh` or `gh-axi` in command position (line start, or after ; & | ( or
# $( ), so a commit message or echo that mentions the command doesn't count.
# An early warning only: .github/workflows/pr-body.yml holds whoever opens the PR.
grep -qE '(^|[;&|(]|\$\()[[:space:]]*gh(-axi)?[[:space:]]+pr[[:space:]]+create([[:space:]]|$)' <<<"$command" || exit 0
grep -qE '(^|[[:space:]])(-h|--help)([[:space:]]|$)' <<<"$command" && exit 0

decide() { # decision, reason
  jq -n --arg d "$1" --arg r "$2" '{
    systemMessage: $r,
    hookSpecificOutput: {hookEventName: "PreToolUse", permissionDecision: $d, permissionDecisionReason: $r}
  }'
  exit 0
}

branch=$(git rev-parse --abbrev-ref HEAD)
head=$(git rev-parse HEAD)
file="$(git rev-parse --path-format=absolute --git-common-dir)/ai-security-review/${branch//\//__}.json"

if [[ ! -s "$file" ]]; then
  decide deny "No AI security review recorded for branch $branch. Run /ai-security-review pr (it writes $file), and put the lines it prints in the PR body under a \"## Security review\" heading."
fi
reviewed=$(jq -r 'if .mode == "pr" then .commit // "" else "" end' "$file" 2>/dev/null || true)
if [[ ! "$reviewed" =~ ^[0-9a-f]{40}$ ]]; then
  decide deny "$file isn't a pull request review record. Run /ai-security-review pr again."
fi

# The body is in the command (--body, or a heredoc) or in a --body-file/-F file.
body="$command"
body_file=$(grep -oE -- '(--body-file|-F)([[:space:]]+|=)("[^"]+"|'"'"'[^'"'"']+'"'"'|[^[:space:]]+)' <<<"$command" \
  | head -1 | sed -E 's/^(--body-file|-F)([[:space:]]+|=)//; s/^["'"'"']//; s/["'"'"']$//' || true)
if [[ -n "$body_file" && "$body_file" != "-" && -f "$body_file" ]]; then
  body+=$'\n'$(cat -- "$body_file")
fi
# The section runs from its heading to the next # or ## heading; each line
# loses its indent, list marker and trailing space before it's compared.
normalize() { sed -E 's/^[[:space:]]*([-*][[:space:]]+)?//; s/[[:space:]]+$//'; }
section=$(tr -d '\r' <<<"$body" | awk '
  tolower($0) ~ /^[ \t]*##[ \t]+security review([^a-z0-9]|$)/ { s = 1; found = 1; next }
  s && /^[ \t]*##?[ \t]/ { s = 0 }
  s { print }
  END { if (!found) print "\001none" }')
if [[ "$section" == $'\001none' ]]; then
  decide deny "The PR body has no \"## Security review\" section. Put the lines recorded in $file under that heading, then open the PR."
fi
section=$(normalize <<<"$section")
missing=""
while IFS= read -r line; do
  [[ -z "$line" ]] && continue
  want=$(normalize <<<"$line")
  grep -qxF -- "$want" <<<"$section" || missing+=$'\n'"$line"
done < <(jq -r '.body[]?' "$file")
if [[ -n "$missing" ]]; then
  decide deny "The PR body's \"## Security review\" section lacks the review's lines (from $file):$missing"
fi

if [[ "$reviewed" != "$head" ]]; then
  decide ask "The AI security review is of ${reviewed:0:7}, but HEAD is now ${head:0:7}, so the commits since then weren't reviewed. Open the PR anyway?"
fi
exit 0
