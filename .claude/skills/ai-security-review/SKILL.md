---
name: "ai-security-review"
description: "HoploDex's AI-assisted security review: a pull request's diff (pr mode, before `gh pr create` or after new pushes), or the whole codebase before a release (release mode). Reviews against docs/attack-surface.md, uploads findings to GitHub code scanning with stable fingerprints, carries earlier alerts forward, proposes the attack-surface list's update, and checks the release gate."
argument-hint: "pr [<number>] | release [<commit>] [--after <reviewed commit>]"
user-invocable: true
---

# AI security review

The constitution ("Development Workflow & Quality Gates", 1.5.0) requires a
whole-codebase AI review before every release, and makes a per-PR one
optional; `.claude/hooks/pr-security-review.sh` requires the per-PR one for a
pull request opened through Claude, and `.github/workflows/pr-body.yml` checks
its lines in every PR body. This skill runs both. You are the **lead**: you
run the script, brief `security-reviewer` subagents, and decide nothing the
operator should.

## User input

```text
$ARGUMENTS
```

`pr` reviews the current branch's HEAD against its base; with `<number>`, the
pull request already exists. `release` reviews `<commit>` (default
`origin/main`'s head); `--after <reviewed commit>` is the final release commit
after fixes made in response to a review of `<reviewed commit>`.

## Rules

- **Findings go to code scanning, never into a committed file, a PR body, a
  PR comment or an issue** (the one exception is below). The PR body carries
  only the script's three lines: the reviewed commit, the counts, and the
  attack surface verdict.
- **A pull request upload carries only what the pull request adds or
  changes.** A weakness already at the merge base, or a copy of one, is
  *pre-existing*: tell the operator, and never upload it against the pull
  request. Before 0.1.0 (no `v*` tag) it may become an ordinary issue, or wait
  for the release review; after, it is a draft private security advisory,
  whose text you print for the operator to file. The script refuses a pr-mode
  finding without `"introduced": true`.
- Reviewers and verifiers are `security-reviewer` subagents: read-only, no
  shell, no network. Reviewing sends source code to the AI vendor, never
  collection data (constitution, Principle V). Never run the app or open a
  database.
- **Never** dismiss an alert on your own, create an advisory, change a
  ruleset, or edit `TOOL`, `CATEGORY` or `BANDS` in the script (a new tool
  name or category orphans every alert). The operator dismisses; you offer.
- Surface ids in `docs/attack-surface.md` are permanent: alert rule ids start
  with them. An entry names what to review, never an unfixed weakness ("X
  lacks Y" is a finding).
- Keep scratch files (diffs, findings, alert lists) in your scratchpad, not
  the tree. The script keeps its SARIF and record in the git common dir.

## The script

`node scripts/ai-security-review.mjs` does everything deterministic (its
header lists the commands): `touched` maps a diff to surfaces, `open` lists
the alerts to carry, `build` validates findings and writes the SARIF and the
record, `upload` sends it and checks the result, `gate` is the release gate,
`carry-dismissal` re-applies a dismissal after a move. Never write SARIF,
fingerprints or API calls yourself.

A finding, as reviewers return it and `build` takes it (a JSON array):

```json
{"surface": "import-export", "cwe": 1236, "path": "src-tauri/src/services/spreadsheet.rs",
 "symbol": "protect_csv_cell", "region": {"startLine": 626, "endLine": 626},
 "severity": "high", "title": "≤255 chars", "message": "what and why",
 "help_markdown": "how to verify and fix", "introduced": true,
 "rule_id": null}
```

`rule_id` is set only on a carried alert (or a new finding the verifier says
*is* a carried or dismissed alert): copy that alert's `rule_id`. Otherwise
`build` mints `<surface>/cwe-<n>/<path>#<symbol>` (adding `~2`… when that is
taken by a different finding), and the fingerprint follows from it, so a line
move or new wording keeps the same alert. A file move, symbol rename or new
surface or CWE is a new alert: treat it as a move (below).

## Steps shared by both modes

1. **Clean tree at the commit.** `git status --porcelain` must be empty and
   the working tree must be the commit under review, since reviewers read the
   tree (release mode: `git switch --detach <commit>`; return afterwards).
   Run `node scripts/attack-surface.mjs`; if it fails, the list is stale:
   fix it first (pr mode) or stop and ask for a list PR (release mode).
2. **Carry.** `open --ref <ref> > <scratch>/open.json` lists open and
   dismissed alerts. Open ones are carried: each goes to its surface's
   reviewer (its `rule_id` starts with the surface id). Dismissed ones are
   context only: a dismissal survives absence and return, so they need no
   upload, but a new finding that *is* one takes its `rule_id` and stays
   dismissed.
3. **Review.** One `security-reviewer` per surface in scope, in parallel (at
   most 4 at once). Brief each with: the entry's text from
   `docs/attack-surface.md`, the commit, the diff file (pr mode, `--after`),
   its carried alerts, the finding schema above, and what to return (new
   findings, plus a verdict per carried alert: present / fixed / moved /
   unsure).
4. **Verify.** A fresh `security-reviewer` per new finding (refute it, check
   the band, answer "same as carried or dismissed #N?", and in pr mode
   "introduced?"), and per `fixed` or `moved` verdict. Unconfirmed findings
   are dropped; an unconfirmed fix stays carried as `unsure`.
5. **Assemble** `<scratch>/findings.json`: confirmed new findings, plus every
   carried open alert that is present or unsure, with its `rule_id` (update
   its region and message to the reviewer's). Leave out only verified fixes.
   A confirmed **move** is a new finding at the new place; once uploaded, if
   the old alert was dismissed, ask the operator, then `carry-dismissal <new>
   <old>`.

## pr mode

1. **Base.** With a number: `gh pr view <n> --json baseRefName,headRefOid`;
   else the base is `main`. `git fetch origin <base>`, and
   `base=$(git merge-base origin/<base> HEAD)`.
2. **Surface verdict first**, because a new entry must exist at the reviewed
   commit before a finding can name it. `touched --base $base` lists the
   surfaces the diff touches and the files no entry claims. Write
   `git diff $base HEAD` to a scratch file, and brief one `security-reviewer`
   with it and `docs/attack-surface.md`: does the diff add, change or remove
   surface no entry covers (a command or its arguments, a capability, CSP or
   Flatpak permission, a path, process or helper boundary, a parser of
   untrusted input or a file format, D-Bus, portal or keyring use, key or
   passphrase handling, a web view or navigation, a dependency that parses
   input or runs native code), or make an entry stale (a path moved or gone,
   an Ask that no longer fits)? It returns `unchanged`, or `changed` with the
   edit to the list.
   - `changed`: apply the edit to `docs/attack-surface.md` (new ids are
     permanent from now on), run `node scripts/attack-surface.mjs`, show the
     operator the edit, and commit it on the branch (CLAUDE.md, "Commit and
     push"). Rerun `touched` for the new HEAD.
3. **Review** (shared steps 2–5) the touched surfaces. Carry
   `open --ref refs/pull/<n>/head > <scratch>/open.json` when the PR exists
   (else there's nothing to carry), and give reviewers
   `open --ref refs/heads/main > <scratch>/main.json` as context: a finding
   that is already one of `main`'s alerts is pre-existing. Each reviewer
   judges `introduced` against the merge base. Pre-existing ones go to the
   operator (Rules), not into `findings.json`.
4. **Build.** `build <scratch>/findings.json --mode pr --commit HEAD --base
   $base --surface unchanged|changed --known
   <scratch>/open.json,<scratch>/main.json`. It prints the
   `## Security review` lines and writes the record the hook reads.
5. **Upload.** It needs the PR's number, so:
   - **No PR yet:** push the branch, write the PR body with the section and
     its three lines (add your own text around them, describing no finding),
     run `gh pr create` (the hook checks the lines and the commit), then at
     once `upload --pr <n>`.
   - **PR exists:** push, `upload --pr <n>`, then replace the body's section
     (`gh pr edit <n> --body-file …`); `pr-body.yml` checks it again.
   Upload even with no findings: an empty upload closes the PR's alerts that
   were fixed and shows the check ran. The check "HoploDex AI security
   review" on the PR is public, as are its annotations (each rule's title and
   message), which is why only introduced findings go there. Its annotation
   threads must be resolved before merge (the `Default` ruleset).
6. **Report** (terminal only): the counts and alert numbers, each carried
   alert's transition, the list edit, and the pre-existing findings with their
   routing.

## release mode

1. **Target.** `commit` defaults to `git rev-parse origin/main`; `ref` is
   `refs/heads/main`, or `refs/heads/release/<x>` if the commit is cut from a
   release branch. Never a tag: an alert on a tag can never close.
2. **The list must describe the code at the commit** (constitution). Besides
   step 1's check, brief one `security-reviewer` to look across the codebase
   for surface no entry covers. If the list is stale, stop: a list PR comes
   first.
3. **Carry**: `open --ref <ref> --seed-prs`, which adds alerts still open on
   pull requests merged since the ref's last analysis (a PR finding merged
   unfixed).
4. **Scope.**
   - **Whole review**: every entry except `not-surface`.
   - **`--after <reviewed>`** (fixes made in response to a review): the
     reviewed commit must be an ancestor (`git merge-base --is-ancestor`) and
     have an analysis on the ref (`gh api
     "repos/exodious/HoploDex/code-scanning/analyses?tool_name=HoploDex%20AI%20security%20review&ref=<ref>"`
     lists its `commit_sha`). Re-verify every carried alert, and review the
     diff `<reviewed>..<commit>` on the surfaces `touched --base <reviewed>`
     names, with its surface check as in pr mode. Not a second whole review
     (constitution 1.5.0).
5. **Review** (shared steps 2–5); `introduced` doesn't apply. After 0.1.0,
   mark each finding whose code is in the latest release (`git show
   <tag>:<path>` has it) *advisory required*: its fix goes through a draft
   private advisory's temporary fork, not a public PR, and you print what the
   advisory needs (affected and patched versions, the description).
6. **Build and upload.** `build … --mode release --commit <commit> --known
   <scratch>/open.json`, then `upload --release --ref <ref>`.
7. **Gate.** `gate` (with `--ref` for a release branch) counts open critical
   and high code scanning alerts from every tool, Dependabot critical and high
   alerts, open secret scanning alerts, and draft or triage advisories. The
   release proceeds only on PASS; each blocking alert is fixed, or dismissed
   by the operator with its reason (a dismissed dependency alert must match a
   recorded exception).
8. **Report** (terminal only): the target, the list's blob
   (`git rev-parse <commit>:docs/attack-surface.md`), the surfaces reviewed,
   one row per alert (number, rule id, band, and new / still open / fixed
   (verified) / dismissed / moved #a → #b / unsure), the upload (SARIF id,
   analysis id, results against expected), the advisory-required list, and
   the gate's verdict with its counts.

## When something fails

`upload` exits non-zero and lists problems when the analysis's result count,
tool or category is off, an uploaded rule has no alert, or an open alert was
left out. Don't re-upload blindly: an upload without a finding closes it as
fixed. Show the operator the problems and the alert list.
