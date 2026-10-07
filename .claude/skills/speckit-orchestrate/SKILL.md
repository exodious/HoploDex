---
name: "speckit-orchestrate"
description: "Lead a feature's tasks.md to completion by delegating batches of tasks to speckit-implementer subagents (Sonnet, high effort), reviewing and committing their work, running speckit-converge at the end, and reporting only decisions and blockers."
argument-hint: "Optional: phases to cover, concurrency limit, or when to stop (e.g. 'stop before the PR')"
compatibility: "Requires spec-kit project structure with .specify/ directory and the speckit-implementer agent"
user-invocable: true
disable-model-invocation: true
---

# Orchestrate a feature's implementation

You are the **project lead**. You don't implement tasks yourself; you hand batches of
tasks to `speckit-implementer` subagents, review what comes back, commit it, and keep the
user informed. Your context is the scarce resource that has to last the whole feature, so
keep file dumps and transcripts out of it.

## User input

```text
$ARGUMENTS
```

Honour it (phases to cover, a stop point, a different concurrency limit). Defaults:
all phases, at most **3** subagents at once, **stop before creating the pull request**.

## 0. Preconditions

1. `.specify/feature.json` names the feature; `tasks.md` exists and is complete.
2. The `speckit-implementer` agent type is available (`.claude/agents/speckit-implementer.md`
   sets `model` and `effort: high`; the Agent tool can't set effort per spawn). If the
   Agent tool says the type is not found, the session must be reloaded before it appears.
   Stop and tell the user rather than falling back silently.
3. Read `tasks.md`'s phase list, "Dependencies & Execution Order" (especially the
   same-file sequences) and "Parallel Opportunities". Read only those sections, not the
   whole spec.
4. Check the branch: never commit to `main`.

## 1. Plan batches

- **Size**: give each subagent a meaningful slice, **4–8 related tasks** or half a phase,
  not one or two. Smaller only when a task is large (a whole page, a full E2E run).
- **Split by file ownership**: agents running at once must not edit the same files. Use
  the same-file sequences in tasks.md to decide what must be one agent's in order.
- **Per user story**, the shape that worked:
  1. Failing tests: backend tests | frontend and E2E tests (two agents in parallel).
  2. Implementation: backend | frontend (two agents; the frontend agent does the
     plumbing/types task **first** and the other frontend agent waits for those types).
  3. The story's E2E run, alone (see §3).
- Start independent later work (another story's tests, docs-only tasks) in a free slot
  when its files don't overlap with what's running.
- Spawn a **new** subagent for every batch. Resume one (SendMessage) only to finish
  **its own** assigned tasks, for example after a rate limit or to pass it news it needs.

## 2. The subagent prompt

Every prompt has these parts. Keep it self-contained: the subagent starts cold.

```text
You are implementing part of feature NNN (specs/NNN-name/) in <repo>. Use the
speckit-implement skill, restricted to these tasks from specs/NNN-name/tasks.md:

**T0xx, T0yy, …** (one-line gist of each)

Context: <what is committed (hash); what the tests encode; the exact API/props the
test-writing agents said they assumed, one line each; decisions already made that
affect these tasks; known gaps to settle>

Ground rules:
- Read CLAUDE.md, DEVELOPMENT.md and the relevant spec docs first.
- Running at the same time: <who, which files>. Touch only <your files>. If a shared
  file must be edited (a contract), re-read it right before each edit and change only
  your rows.
- Do NOT git commit or push. Mark completed tasks [X] in tasks.md (only your lines).
- Run checks via <test wrapper, e.g. scripts/dev-container.sh>, long runs in the
  background with logs in your scratchpad and a summary line. <Don't run app builds/E2E
  if another agent is.>
- The contract/spec wins over a test; if they conflict, fix the test and say so.
- Final report: under 150 words — only decisions I might want to revisit, deviations,
  test changes, and anything left red. Don't summarise the tasks.
```

For **test-writing** batches, ask instead for "the API your tests assume, one line each,
then ambiguities", and pass that list to the implementer of the same story.

## 2a. Per-OS batches on other machines' sessions

The lead runs on whichever machine the user leads from (usually Linux, the
fastest; sometimes another). Tasks that need a different OS (code behind a
`#[cfg(target_os = ...)]`, that OS's tests, E2E, screenshots or surface check)
go to a Claude session on a machine with that OS, over `SendMessage`. Each
runs under Remote Control in a HoploDex checkout and is named
`<host>-hoplodex`; find the exact names with `ListAgents`. As of feature 007:
`nous-hoplodex` (Linux, usually the lead), `apollo-hoplodex` (a Mac, which
drives the macOS 26 VM with `scripts/tart-vm.sh`), and `win-<host>-hoplodex`
(the Windows test VM, kept running by `scripts/windows/claude-remote-control.ps1`;
DEVELOPMENT.md "Windows").

- **The batch:** write it like a subagent prompt (§2), plus the lead's
  session name to reply to and "use the speckit-os-batch skill". That skill
  makes the other session a **sub-lead**: it always hands the tasks to new
  `speckit-implementer` subagents, never implements them itself, and keeps
  its own context small, since it takes batch after batch for the whole
  feature. It pulls, commits by path, pushes and reports the hash. Don't ask
  the user to type a command there.
- **Acknowledge every report:** messages between machines carry no delivery
  receipt in either direction, so answer each report at once with a one-line
  `SendMessage`, "Received <hash>", before anything else. The session waits for
  it rather than repeating its report to the user.
- **Same permission mode:** a session in a different permission mode from
  yours holds your messages for the user's approval. The sessions run in auto
  mode; if a message from one shows `from-mode="prompting"` and batches stall,
  tell the user.
- **Pulling:** pull before reviewing its commits, and never while one of your
  own agents would have files changed under it mid-run.
- **Why sessions, not SSH:** subagents driving another machine over SSH were
  considered and set aside. Windows' `sshd` ends every process when the
  connection drops, an SSH session has no desktop for E2E, screenshots or
  real input, and a key-authenticated session likely can't use Credential
  Manager. The sessions run in the machine's desktop session instead.

## 3. Shared build environment

When agents share one checkout's build state (on Linux, the dev container's volumes; on
macOS or Windows, the checkout itself: one `target/`, one `node_modules`, one `dist/`),
cargo serialises on its lock but app builds don't:

- Run **anything that builds or launches the app** (E2E, screenshots, the full gates, a
  release performance run) **one at a time**, with nothing else building the app.
- Agents may run unit tests in parallel; a red test caused by another agent's
  in-progress file is expected. Tell agents to wait and retry rather than fix it.
- Don't change the container script or test harness mid-feature to work around this.

## 4. Review and commit (the lead's job)

When a subagent reports:

1. **Verify claims that matter** with a quick look at the code (`git diff`, `grep`), not
   the whole transcript. Never read a subagent's output file.
2. **Check deviations against the spec.** Revert an unrequested rule, or keep it and flag
   it (say which and why). Don't let an agent edit a spec's Source Request or other
   recorded decisions; add a dated amendment note instead.
3. **Commit by path**, leaving out files a still-running agent is editing; push. One
   commit per batch, message naming the task IDs. Follow CLAUDE.md's commit rules.
4. Note follow-ups that belong to a later batch, and put them in that batch's prompt.

## 5. Interruptions

- **Rate limit**: agents fail with a 429. Once it resets, resume each one with
  SendMessage ("you were cut off; resume <its tasks>; check git status/diff first; who
  else is resuming"). Don't respawn: their partial edits and context are worth keeping.
- **Agent stuck or looping**: check `ps` (and on Linux `podman ps` for its container) before assuming;
  report to the user.

## 6. Reporting to the user

Keep updates short. After each batch: the task IDs done, the commit hash, then **only**
decisions they might want to revisit, deviations from the spec, and blockers. No recap of
what tasks do; tasks.md says that. Report each phase when it completes. Mention usage
limits when a long phase is about to start near one.

Manual tasks (a screen reader, eyeballing drawings) and pull request close-out tasks go
to the user; don't attempt them, and don't open the PR until the user says so.

## 7. Converge

When every implementable task is done:

1. Split the assessment across **two read-only subagents** by slice (for example US1–US2
   plus their FRs and contracts | the remaining stories, plan decisions and the
   constitution). Each follows speckit-converge Steps 2–6 but **writes nothing** and
   returns: a findings table (ID, gap type, severity, source-ref, evidence, remaining
   work as an imperative), a "needs a decision" list with options, and counts.
   List the decisions already accepted so they aren't re-reported.
2. **Verify** the HIGH findings yourself (a quick read of the cited lines).
3. **Stop for decisions**: put each genuine product question to the user (AskUserQuestion,
   recommended option first) before appending anything. Explain unfamiliar risks plainly
   and concretely when asked.
4. Append one `## Phase N: Convergence` section yourself (append-only, next IDs, HIGH
   first, each task citing its source-ref and gap type, the user's decisions recorded at
   the top). Commit it.
5. Implement it with the same batching, then run the remaining E2E and the full gates
   once more, alone.

## 8. Lessons that cost time before

- Unit tests that mock the IPC layer miss argument-shape mismatches between frontend and
  backend. Ask the frontend implementer to pin `invoke` arguments for every new command,
  and get at least one E2E per story through the real IPC early.
- Agents fill contract gaps on their own. Ask for those choices in the report, and
  bring the user-visible ones (wording, layout, rules) to the user.
- Test-writing agents make reasonable but wrong assumptions (wrong ARIA role, reading
  `textContent` of inputs, tokens that never appear). Implementers may fix the test, but
  must say so.
- A pre-existing fixture (a portability database, a seed) may need regenerating when the
  schema is edited in place. Check how earlier features did it before accepting.
