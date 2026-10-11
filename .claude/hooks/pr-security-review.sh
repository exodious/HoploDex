#!/usr/bin/env bash
# PreToolUse hook for Bash: a `gh pr create` goes ahead only once the branch has
# a saved /security-review result and the PR body carries it.
#
# The result is saved, never committed (constitution: security findings are not
# kept in documents committed to the repository), at
#   <git common dir>/security-review/<branch, "/" as "__">.md
# and its first line is `<!-- reviewed: <full commit sha> -->`.
set -euo pipefail

input=$(cat)
command=$(jq -r '.tool_input.command // ""' <<<"$input")
cwd=$(jq -r '.cwd // empty' <<<"$input")
[[ -n "$cwd" ]] && cd "$cwd"

# Only `gh` in command position (line start, or after ; & | ( or $( ), so a
# commit message or echo that mentions the command doesn't count.
grep -qE '(^|[;&|(]|\$\()[[:space:]]*gh[[:space:]]+pr[[:space:]]+create([[:space:]]|$)' <<<"$command" || exit 0
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
file="$(git rev-parse --path-format=absolute --git-common-dir)/security-review/${branch//\//__}.md"

if [[ ! -s "$file" ]]; then
  decide deny "No security review saved for branch $branch. Run /security-review, save its result to $file (first line: <!-- reviewed: <full HEAD sha> -->), and put it in the PR body under a \"## Security review\" heading."
fi

# The body is in the command (--body, or a heredoc) or in a --body-file/-F file.
body="$command"
body_file=$(grep -oE -- '(--body-file|-F)([[:space:]]+|=)("[^"]+"|'"'"'[^'"'"']+'"'"'|[^[:space:]]+)' <<<"$command" \
  | head -1 | sed -E 's/^(--body-file|-F)([[:space:]]+|=)//; s/^["'"'"']//; s/["'"'"']$//' || true)
if [[ -n "$body_file" && "$body_file" != "-" && -f "$body_file" ]]; then
  body+=$'\n'$(cat -- "$body_file")
fi
if ! grep -qiE '^[[:space:]]*##[[:space:]]+Security review' <<<"$body"; then
  decide deny "The PR body has no \"## Security review\" section. Add the review saved in $file under that heading, then open the PR."
fi

reviewed=$(head -1 "$file" | sed -nE 's/^<!-- reviewed: ([0-9a-f]{40}) -->.*/\1/p')
if [[ -z "$reviewed" ]]; then
  decide deny "$file doesn't say which commit was reviewed: its first line must be <!-- reviewed: <full sha> -->."
elif [[ "$reviewed" != "$head" ]]; then
  decide ask "The saved security review is of ${reviewed:0:7}, but HEAD is now ${head:0:7}, so the commits since then weren't reviewed. Open the PR anyway?"
fi
exit 0
