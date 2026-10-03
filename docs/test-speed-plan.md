# Test suite speed plan

Findings from timing every test layer on 2026-10-01, and a plan to cut the
full run from about 13½ minutes to about 4 without dropping coverage.

## How to use this file

Each step below stands on its own and fits in one session. To do one:

1. Read **Ground rules** and **Measuring**, then **only your step's section**.
   Its "Read" list names the files you need; you shouldn't need the rest of
   the codebase, the findings, or the other steps.
2. Do the step on this branch (`test-speed`), measure before and after as the
   step says, and fill in its **Result** line with the numbers.
3. Update the step's row in **Status**, and commit the step on its own
   (`Test speed step N: <what>`).

A session prompt that works: *"Do step N of docs/test-speed-plan.md."*

Steps 1–2 (Rust) and 3–7 (E2E) are independent tracks, and either can go
first. Within a track, do the steps in order unless a step says otherwise.

**Issue #29 (one embedded WebDriver on every platform) sits inside the E2E
track.** It replaces `tauri-driver` with a WebDriver server compiled into E2E
builds, which rewrites the driver, port and relaunch parts of step 5. The
E2E order is therefore:

1. Steps 3 and 4. Neither depends on the driver, and step 3 creates the E2E
   Cargo feature #29 reuses.
2. #29's Linux trial.
3. Steps 5, 6 and 7.

If #29's trial fails, do step 5 as written. Run `gh issue view 29` before
starting step 5 to see where it stands.

**#29's Linux trial passed** (2026-10-03, branch `embedded-webdriver`):
`tauri-driver` is gone. E2E builds carry `tauri-plugin-wdio-webdriver` behind
the `e2e` feature, and `e2e/support/app.ts` launches the app with
`TAURI_WEBDRIVER_PORT`, kills it, and relaunches it (`relaunchApp()`, used by
us4 and us7). `@wdio/tauri-service` isn't used. Full run 14 of 14 spec
files in 5 m 26 s (368 s with `tauri-driver`); us7 3 runs in a row; the
screenshot walk 2 m 32 s (4 m 00 s), its 134 images the same as
`tauri-driver`'s but for the sandbox path in text, the scrollbar thumb's
fade and anti-aliasing. Do step 5 the embedded way (its "First, check issue
#29" list).

## Status

| Step | What | Est. saving | Depends on | State |
|---|---|---|---|---|
| 1 | Performance tests out of the default run, seeded once | −90 s Rust | – | done |
| 2 | cargo-nextest | −55 s Rust | 1 | done |
| 3 | Settable idle-lock duration for E2E builds | −60 s E2E | – | done |
| 4 | Replace fixed E2E sleeps with an "app is idle" wait | −150 to −250 s E2E | – | done |
| 5 | Run E2E specs in parallel workers | E2E ≈ ÷3 | 4 recommended; #29 settled (trial passed) | done |
| 6 | Split the long E2E spec files | balance for 5 | 5 | todo |
| 7 | Faster release profile for E2E builds | −50 s per rebuild | – | done |
| 8 | Test-level policy for new features (SDD) | stops E2E growth | – | todo |
| 9 | Optional: one integration-test binary; cheaper test databases | −15 s build, CPU | 2 | todo |

## Baseline

Measured in the dev container on a 24-core, 31 GB host, branch
`006-accessory-links` at `caab90e1`, with all build caches warm. All tests
passed.

| Stage | Time | Notes |
|---|---|---|
| `cargo test` | **184 s** | 58 test binaries, run one after another |
| ↳ `performance_test.rs` | 90 s | 18 tests, each seeds 10,000 records in a debug build, one at a time |
| ↳ `human_seed_coverage_test.rs` | 17 s | 5 tests |
| ↳ the other 56 binaries | 77 s | the longest: import_export 9.6 s, passphrase_change 6.1 s, pending_changes 6.0 s |
| Rust test rebuild after touching `src/lib.rs` | 22 s | mostly linking 58 binaries |
| Vitest | 26 s | 1,060 tests; fine as it is |
| `npm run build` | 5 s | |
| E2E release rebuild after touching `src/lib.rs` | 70 s | `lto = true`, `codegen-units = 1`, `opt-level = "s"` |
| `npm run test:e2e` | **519 s** | 14 spec files, one at a time (`maxInstances: 1`) |

E2E per spec file (seconds): us12-accessories 93, us9-locking 72,
us3-value-insurance 71, us1-record-firearm 66, us11-regulated-items 38,
us7-databases 38, us8-backups 25, ui-review 23, us5-export-import 23,
us2-browse-search 16, us4-photos-documents 12, us10-cartridges-actions 11,
us7-databases-no-keyring 8, us6-identification 8.

Test counts at each feature merge (Rust integration / Vitest / E2E `it`):
before 003 282 / 228 / 64; 003 545 / 439 / 94; 004 653 / 571 / 103;
005 699 / 653 / 107; 006 (now) 972 / 968 / 120.

After steps 1–4 and 7, measured the same way on the same host with nothing
else running (2026-10-02, `ccf73ae4`): Rust 184 s → **45 s** with
`cargo nextest run` (94 s with plain `cargo test`), 957 passed and the 18 perf
tests skipped; E2E 519 s → **368 s**, 14 of 14 spec files passed; an E2E
rebuild after a backend change 70 s → about 4 s.

## Findings

- **E2E is serial and full of fixed sleeps.** About 390 s of the 519 s sits
  in gaps of 0.1–0.7 s between WebDriver commands, the pattern of
  `browser.pause()`. `e2e/support/ui.ts` has 25 of them (`SETTLE_MS = 200`
  after nearly every click and fill, 600 ms after a search), the specs have
  17 more, and the screenshot walk has 51.
- **One E2E test waits a real minute.** us9's "locks after the idle duration
  set in its settings" waits for the 1-minute idle lock (about 60 of us9's
  72 s).
- **E2E can't run in parallel yet.**
  - `e2e/wdio.conf.ts` always uses ports 4444/4445, and its
    `killProcessesOnPorts` would kill a sibling worker's tauri-driver.
  - `xvfb-run` wraps the whole run, so every worker's window would share one
    X display.
  - `cargo build` (and the seed for the screenshot walk) runs in every
    `beforeSession`.

  The sandbox itself is already per worker, since wdio workers are separate
  processes.
- **`cargo test` runs binaries one after another.** It only runs tests in
  parallel within a binary, so the 58 binaries add up.
- **The performance tests dominate Rust time and measure nothing useful in
  debug.** The `within()` budgets are tripled in debug builds. DEVELOPMENT.md
  already says the budgets are only timed in release. Each test seeds its own
  10,000 records, behind a `static Mutex` so they run one at a time.
- **Each test database costs about 0.4 s of CPU.** `kdf_iter = 1000000`
  (PBKDF2-SHA512) on every create or open. Run one at a time, nickname_test's
  12 trivial tests take 4.6 s. About 680 `TestDb::new`/`test_session` calls
  make roughly 6 CPU-minutes. That's hidden on 24 cores, but on a 4-core
  machine it would add roughly a minute and a half.
- **E2E grows by one test per acceptance scenario.** Nothing in the
  constitution requires E2E (it requires integration tests for persistence
  and a test for every behaviour change). The habit comes from `/speckit-plan`
  and `/speckit-tasks` turning each scenario into an E2E task. Edge cases such
  as us1's future-date and whole-dollar scenarios could be tested at the Rust
  `ops` or Vitest level instead.

## Ground rules (every step)

- Follow CLAUDE.md: run everything through `scripts/dev-container.sh`, never
  touch the real databases, and use `--build` after a Dockerfile change.
- Keep coverage the same. Only step 8 moves a test to another level, and it
  never deletes one without an equivalent elsewhere. Any test that passed
  before must pass after, with the same number of tests in each layer.
- Run a changed E2E spec at least **3 times** before calling it stable.
  Flakiness is the main risk of steps 4–6.
- Keep DEVELOPMENT.md's "Test" section (and CLAUDE.md, if a command it names
  changes) in step with what you change, in the same commit.

## Measuring

```bash
# Rust: total, and per binary
scripts/dev-container.sh bash -c 'cd /workspace && time cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "Running|test result"'

# E2E: total, and log for per-spec times (run npm run build first)
scripts/dev-container.sh bash -c 'cd /workspace && npm run build && time npm run test:e2e' > /tmp/e2e.log 2>&1
```

Per-spec E2E times from that log (each wdio worker's first and last
timestamp):

```bash
python3 - /tmp/e2e.log <<'EOF'
import re, sys, datetime as d
first, last, spec = {}, {}, {}
for line in open(sys.argv[1], errors="ignore"):
    m = re.match(r"\[(0-\d+)\] (\d{4}-\d\d-\d\dT[\d:.]+)Z", line)
    if m:
        t = d.datetime.fromisoformat(m.group(2)); first.setdefault(m.group(1), t); last[m.group(1)] = t
    m = re.match(r"\[(0-\d+)\] RUNNING in wry - file:///e2e/specs/(\S+)", line)
    if m: spec[m.group(1)] = m.group(2)
for s, n in sorted(((last[c] - first[c]).total_seconds(), spec.get(c, c)) for c in first)[::-1]:
    print(f"{s:6.0f}s  {n}")
EOF
```

Run long commands in the background with output in a log file (CLAUDE.md,
"Waiting on a background run").

---

## Step 1 — Performance tests out of the default run, seeded once

**Why.** 90 of `cargo test`'s 184 s. The numbers mean little in debug, and
each of the 18 tests seeds 10,000 records again.

**Read.** `src-tauri/tests/performance_test.rs`, `src-tauri/tests/support/mod.rs`
(`TestDb`), DEVELOPMENT.md "Test" (the performance paragraph).

**Do.**
1. Mark every test in `performance_test.rs` `#[ignore = "release only: see DEVELOPMENT.md"]`.
   The documented release command gains `--ignored` (or `--include-ignored`).
2. Seed once per binary: build a template database with 10,000 firearms in a
   `OnceLock`, closed, in a `TempDir` kept for the process. Each test then
   copies the file and opens the copy with the test passphrase.
   - Add a constructor to `TestDb` for this (for example,
     `TestDb::copy_of(path)`). Per the "refactor, not workaround" memory,
     change the one helper and its callers rather than adding `_with_x`
     variants.
   - Tests that seed something else stay as they are: the 10,000-accessory
     fixture around line 828, the session-based fixture around line 581, and
     any test that needs a second seeding pass. Only the plain
     `TestDb::new(); seed_10k_firearms(...)` pairs change.
   - Copy only a **closed** database. Its file holds an exclusive lock while
     open (CLAUDE.md: `locking_mode = EXCLUSIVE`).
3. Keep the `one_at_a_time()` mutex for now; step 2 replaces it.
4. Decide where the release perf run lives so the constitution's performance
   gate still runs: at least in DEVELOPMENT.md's "before a pull request"
   guidance. Ask the user if no such checklist exists.

**Verify.** `cargo test` drops by about 90 s. The documented release
command, with `--ignored`, still passes every budget and is quicker than
before (one seeding instead of 18).

**Done when.** The default run skips the perf tests, the release run passes,
and DEVELOPMENT.md shows both commands.

**Result.** Default `cargo test`: 14 unit + 972 integration → 14 unit + 940
integration passed, 18 ignored (the perf tests); the 90 s `performance_test`
binary now finishes in 0.00 s. Total 184 s → 213 s wall, but that run was a
cold build in a fresh container volume with three other implementers
building on the same machine, so it says nothing about the saving; expect
about 90 s off the warm 184 s. Release perf run (`--ignored`), all 18
budgets pass: test time 65.6 s → 57.6 s (loaded machine). The gain is small
because the seeded-once change covers only the 12 plain tests; the
session-based and 10k+10k fixtures (6 tests) still seed per test, as the
step said. The release command is in DEVELOPMENT.md "Test", with the rule to
run it before a pull request that touches search, listing, persistence or
`ops`, and before every release. `TestDb::copy_of(template)` and
`TestDb::FILE_NAME` are new; the template is removed by an `atexit` hook.

## Step 2 — cargo-nextest

**Why.** `cargo test` runs the 58 binaries one after another. Nextest runs
every test across all binaries in parallel, so the run is bounded by the
slowest single test (about 17 s, the seed coverage test) instead of the sum.

**Read.** `Dockerfile` (the `cargo install` block around line 134),
`src-tauri/tests/portability_test.rs` (its `ONE_AT_A_TIME` mutex and
`set_var`), `src-tauri/tests/performance_test.rs` (`one_at_a_time`),
DEVELOPMENT.md "Test".

**Do.**
1. Install `cargo-nextest` in the Dockerfile alongside `cargo-deny`, pinned
   by version and `--locked`, then rebuild with
   `scripts/dev-container.sh --build`. Also add it to DEVELOPMENT.md's host
   prerequisites.
2. Add `src-tauri/.config/nextest.toml`:
   - Nextest runs each test in its own process, so a `static Mutex` no longer
     serializes anything. Put `performance_test` in a `test-group` with
     `max-threads = 1` (it must not be timed while anything else runs; also
     consider the `serial` setting).
   - `portability_test` sets environment variables. Per-process isolation
     makes that safe under nextest, but keep the mutex for anyone still
     running `cargo test`.
3. Nextest doesn't run doctests. Check what `cargo test --doc` runs; if it
   runs any, keep it as a second command.
4. Switch DEVELOPMENT.md's commands to `cargo nextest run`, including the
   single-file and single-test forms (`-E 'binary(firearm_lifecycle_test)'`,
   or a name filter). Keep `cargo test` working as the fallback.

**Verify.** Every test passes under `cargo nextest run --manifest-path
src-tauri/Cargo.toml`, with the same count as `cargo test` (972 integration
+ 14 unit at baseline, minus the ignored perf tests). The release perf run
still works under the test group.

**Done when.** The documented commands use nextest, and it runs in the
container without a manual install.

**Result.** Warm `cargo test` 96 s → warm `cargo nextest run` 45 s (about
−50 s), run back to back in one container on a loaded machine (three other
implementers building); 14 unit + 940 integration passed, 18 skipped, same
counts under both. `cargo test --doc` runs 0 doctests (none exist), so nextest
loses nothing and no second command is needed; DEVELOPMENT.md lists it as a
one-line optional check. `cargo-nextest` 0.9.146 is in the Dockerfile
(`CARGO_NEXTEST_VERSION`, `--locked`). `src-tauri/.config/nextest.toml` puts
`performance_test` in a `performance` group with `max-threads = 1` and
`threads-required = "num-cpus"`, so it takes every slot and nothing runs
beside it; `cargo nextest run --run-ignored all` (debug) ran all 18
contiguously after the other tests and `show-config test-groups` reports the
group. The release perf command stays on `cargo test`: under nextest each test
is its own process, so the once-per-process 10,000-record seeding of step 1
would repeat 12 times. The two `static Mutex`es stay for `cargo test`.

## Step 3 — Settable idle-lock duration for E2E builds

**Why.** us9 waits a real minute for the idle lock.

**Read.** `src-tauri/src/session/idle.rs` (around line 93:
`now - last_input >= chrono::Duration::minutes(settings.idle_minutes)`),
`src-tauri/src/services/keyring.rs` (around lines 60–75: the pattern for an
E2E-only environment variable behind `#[cfg(feature = "mock-keyring")]`),
`e2e/specs/us9-locking.e2e.ts` (the test around line 145), `e2e/wdio.conf.ts`.

**Do.**
1. In E2E builds only, let an environment variable (for example
   `HOPLODEX_E2E_IDLE_MINUTE_SECONDS=3`) set how long the idle clock counts
   as one minute. The cfg gate must keep it out of release builds.
   - **Decision for the user:** `mock-keyring` is the only E2E-only feature
     today. Reusing it for this is a misnomer, so the clean choice is a new
     `e2e` feature that implies `mock-keyring`. Then update the build in
     `wdio.conf.ts` and `runSeed`, and the feature list in DEVELOPMENT.md.
   - Issue #29 also needs an E2E-only feature, to gate its embedded WebDriver
     server, which must never reach a shipped build. Make this the one
     feature both use: name it for E2E builds generally, not for the idle
     clock. If #29 has already landed, reuse its feature instead of adding
     another.
2. Set the variable for us9 in `wdio.conf.ts`'s `beforeSession` (only when the
   spec is us9, like the `-no-keyring` rule), and cut the test's 90 s
   `waitUntil` and 150 s `.timeout()` to fit.
3. The notice still reads "after 1 minute without use". That's correct,
   because the setting is still 1 minute.
4. Add a Rust test for the scaling, in the style of the existing idle tests.

**Verify.** us9 drops from about 72 s to about 15 s, and still passes 3 runs
in a row.

**Result.** us9 72 s → 14 s of test time (5 passing in 13.5, 13.6, 13.7 s on
three runs in a row; 17–18 s wall with the binary already built; the first
run's 95 s included a cold build). Measured with the machine loaded by two
other implementers' builds and tests. New `e2e` Cargo feature (implies
`mock-keyring`; meant for #29's embedded WebDriver server too).
`HOPLODEX_E2E_IDLE_MINUTE_SECONDS=3` is set for us9 only. Unit tests in
`session/idle.rs` run with `--features e2e --lib`.

## Step 4 — Replace fixed E2E sleeps with an "app is idle" wait

**Why.** The largest single cost of the E2E run: about 390 s of gaps between
WebDriver commands, mostly fixed `browser.pause()` calls. Condition-based
waiting is also what makes step 5 safe, because fixed sleeps that are just
long enough on an idle machine become too short under parallel load.

**Read.** `src/services/tauriClient.ts` (`invoke`, around line 51),
`src/features/browse/searchHooks.ts` (the search debounce),
`e2e/support/ui.ts` (`SETTLE_MS` and the 25 `browser.pause` calls).

**Do.** Split this over two sessions if needed: 4a covers points 1–3, 4b
covers point 4.

1. Add a tiny busy counter in `src/lib/` (for example `busy.ts`:
   `begin()`/`end()`, and `window.__hoplodexBusy` exposed as a number).
   - `invoke()` wraps each call in it.
   - The search debounce counts as busy from the keystroke until its search
     resolves. Other debounces or timers that the E2E specs wait out need
     the same.
   - The counter is harmless in production builds, so keep it unconditional
     unless the user prefers a build flag.
   - Give it a Vitest test.
2. In `e2e/support/ui.ts`, add `settle()`, which waits until three things
   hold:
   - the busy counter is 0;
   - no **finite** animation is running:
     `document.getAnimations().filter(a => a.effect?.getTiming().iterations !== Infinity && a.playState === "running")`
     is empty. The chooser's catalogue plate cycles forever, so an unfiltered
     check never settles;
   - two `requestAnimationFrame`s have passed, so React has committed and
     painted.
3. Replace each `browser.pause(N)` in `ui.ts` with `settle()`, one helper at a
   time, running the specs that use it.
   - Where a pause covers something the counter can't see (a Radix
     open/close, focus moving after a dialog closes), wait for that specific
     condition instead.
   - Keep a pause only where it is the point of the test (for example "nothing
     happens for N ms"), with a comment saying so.
4. Do the same for the 17 pauses in `e2e/specs/` and the 51 in
   `e2e/screenshots/screens.e2e.ts`. The screenshot walk isn't part of
   `test:e2e`, so it's lower priority.

**Verify.** The full E2E run passes 3 times in a row, and the total drops.
Compare per-spec times against the baseline with the script in **Measuring**.

**Result.** 4a (the helpers in `ui.ts`; no `browser.pause`
is left there): full run 519 s (baseline) → 433 s (7 m 13 s, three passing
runs in a row, each with all 14 specs green), measured on a loaded machine
(three other worktrees building and testing), so the figures are indicative.
us9's one-minute wait is step 3's and is still in this number (72 s). Per
spec, wdio's own times, before → after, from back-to-back runs on that
machine: us3-value-insurance 70 → 43 s, us1-record-firearm 65 → 45,
us5-export-import 21 → 12, us2-browse-search 15 → 10, ui-review 22 → 15,
us4-photos-documents 11 → 8, the other specs unchanged (us11 37 → 36, us12
91 → 88, us7-databases 34 → 33, us8-backups 24 → 24, us10 9 → 9). The
13 specs other than us9 sum to 405 s → 336 s. What is left is mostly in the
specs' own pauses (4b), the real-key-press specs (us11, us12: about 200 ms a
key through `x11-input.py`) and `settle()` itself, about 50 ms a call at best
(two frames) and longer while a backend call such as a key derivation runs.
`settle()` skips animations longer than 1 s (the firearm drawing's draw-in,
the form section highlight): the highlight made us1's Scenario 15 fail,
because `settle()` waited out the 1.8 s highlight the test then looked for.
Screenshot walk: all 51 `browser.pause` calls in `screens.e2e.ts` are gone, and
so are the three in `screenshots.ts`'s `shot()` and `chooseTheme()`, which
the walk also paid for on each of its 132 shots. The walk's screens spec took
4 m 07 s → 2 m 42 s (the whole `npm run screenshots` 4 m 25 s → 2 m 57 s, build
excluded; three passing runs in a row). No pause is kept. `shot()` now ends in
`settleForShot()` (`settle()` plus waiting for every finite animation,
decorative ones included), and `resizeWindow()` waits for the window to change
size. Compared image by image with the walk before: 132 files, of which the
only differences are the per-run sandbox path and clock times in text (14, 16,
17, 19, 21, 22, 27), the overlay scrollbar thumb's fade shade (02, 30, 32; it
also varies between two runs of the new walk), and 08-add-firearm, where the
list photos behind the dialog are now loaded.

4b, specs (the 17 `browser.pause` calls in `e2e/specs/`, none left; the
screenshot walk is the other half): 16 became `settle()` and one a
`waitUntil` on the app having quit (us7's discard-and-quit, which polls until
WebDriver can no longer reach it); us1's Scenario 15 `isFieldInView` is now a
`waitUntil` too. us9's draft-staging pause became a dispatched window `blur`
(which stages the draft at once, the way `usePendingDraft` does) plus
`settle()`. No pause was kept, since none was "nothing happens for N ms".
Counting the draft's 250 ms debounce in `busy.ts` was tried and dropped: it
made every `fill` in every form wait it out, and us1 went from 45 s to 74 s.
Full run, three in a row, 14 of 14 spec files green each time, on the
combined branch (steps 3, 4a, 7 and 4b): 6 m 09 s, 6 m 01 s, 6 m 02 s (the
binary already built; the first run after a code change adds the cargo
build). Per spec, wdio wall time of the last run: us12 89 s, us1 44, us3 38,
us11 36, us7-databases 32, us8 23, us9 15, ui-review 14, us5 13, us2 10, us10
10, us4 8, us7-no-keyring 8, us6 7; their sum is 347 s, against 4a's 433 s
total and the 519 s baseline. The 4b edits themselves are worth only about
10 to 15 s (us3 43 → 38, us4, us8 and us2 a second or so each); most of the
drop from 4a's figure is a quieter machine, so compare the baseline and this
line, not 4a's. Remaining time is us12's real key presses, `settle()`
(about 50 ms a call at best) and each spec's own work.

## Step 5 — Run E2E specs in parallel workers

**Why.** 14 spec files run one at a time. With N workers, the run is bounded
by roughly max(longest spec, total ÷ N).

**Read.** `e2e/wdio.conf.ts`, `e2e/support/app.ts` (the app's port is
`WEBDRIVER_PORT` there), `e2e/run-e2e.mjs`, `e2e/support/realInput.ts`
and `e2e/scripts/x11-input.py` (how real input finds the display and
window), DEVELOPMENT.md "Test" and "Test isolation".

**First, check issue #29** (`gh issue view 29`). This step is written for
`tauri-driver`. If #29 has replaced it with the embedded WebDriver server:
- **Point 2 becomes simpler.** Each worker launches its own app with its own
  `TAURI_WEBDRIVER_PORT` and points `config.port` at it. There is no
  tauri-driver, no native port and no `findNativeDriver`.
- **Relaunching moves to the harness.** Relaunches (`reloadSession()` in us4
  and us7) go through whatever #29 settled on, `@wdio/tauri-service` or a
  harness helper, and that relaunch must reuse the worker's port and display.
- **The rest still applies:** one build in `onPrepare`, a display per worker
  (the app still renders under X, and `realInput.ts` still needs it),
  `maxInstances`, and the isolation check.

If #29 is still open and untried, ask the user whether to wait for it or go
ahead with `tauri-driver`, which means rewriting point 2 later.

**Do.**
1. **One build, before any worker.** Move `cargo build --release ...` out of
   `beforeSession` into `onPrepare`. Also build the `human_seed` example
   there (`cargo build --release --features ... --example human_seed`) so
   that workers seeding the screenshot walk don't fight over the cargo lock.
2. **Ports per worker.**
   - In `beforeSession`, take the worker number from `cid` (`"0-5"` → 5) and
     give tauri-driver `--port 4444+2n` and `--native-port 4445+2n`.
   - Set `config.port` to match. **Verify** that wdio honours a `config.port`
     changed in `beforeSession`: the log line "Connecting to existing driver
     at http://127.0.0.1:PORT" must show the worker's port. If it doesn't,
     set the port on the capability instead.
   - `killProcessesOnPorts` then only clears the worker's own two ports.
     Clearing every port in the range up front belongs in `onPrepare`.
3. **A display per worker.** Stop wrapping the whole run in `xvfb-run` in
   `run-e2e.mjs`.
   - In `beforeSession`, start `Xvfb -displayfd <fd> -screen 0 1920x4200x24`
     (or `xvfb-run -a` around tauri-driver), set `DISPLAY` in
     `process.env` before spawning tauri-driver, and kill the display in
     `afterSession`.
   - `realInput.ts` and `x11-input.py` must use the worker's `DISPLAY`;
     check that they read it from the environment.
   - Keep `GDK_BACKEND=x11` and the `WAYLAND_DISPLAY` removal.
4. **`maxInstances`.** Make it configurable (`HOPLODEX_E2E_WORKERS`, defaulting
   to about half the cores, capped at 4–6). Each WebKitGTK app uses a few
   hundred MB. `--spec` runs and the screenshot walk keep working unchanged.
5. **Isolation.** The sandbox is already per worker. Confirm no two workers
   share anything else: the keyring file and `HOPLODEX_E2E_DOCUMENTS` live
   in each worker's sandbox, so check that nothing writes to a fixed path
   such as `e2e/screenshots-out` from two workers at once.
6. Update DEVELOPMENT.md: how parallel runs work, the variable, and how to
   run serially (`HOPLODEX_E2E_WORKERS=1`) when debugging.

**Verify.** The full run passes 3 times in a row with 4 workers and once with
1 worker. The total is close to max(longest spec, total ÷ 4).

**Result.** Full run 326 s (one worker, after #29) → **98–99 s** with 4
workers, 14 of 14 spec files passing on three runs in a row (wdio's own
total; 100 s wall each, binary already built), measured in the dev container
on the 24-core host with nothing else running. With `HOPLODEX_E2E_WORKERS=1`
the same suite passed in 323 s, so running one at a time costs nothing
extra. Per spec under 4 workers: us12 88 s, us1 39, us11 36, us3 31,
us7-databases 29, us8 21, us9 14, ui-review 12, us5 12, us10 9, us2 9, us4 8,
us7-no-keyring 8, us6 6; their sum is 322 s, so parallel load slows a spec
little. The run is now us12 plus the ~10 s before a worker frees up for it:
max(88, 322 ÷ 4) = 88 against 99, which step 6 attacks. Screenshot walk
(`npm run screenshots`, its two specs now side by side) 2 m 32 s → 2 m 20 s,
134 images as before. How it's done: `onPrepare` builds the app and the
`human_seed` example once (a failed build ends the run), and the screenshot
seeding runs the built example instead of `cargo run`. Each worker takes port
4445 + its worker number from `cid` (`assignWorkerPort` in
`e2e/support/app.ts`); WebdriverIO does honour a `config.port` set in
`beforeSession`, since it connects with that same config object (the log
shows "Connecting to existing driver at http://127.0.0.1:4446/" for worker
`0-1`). Each worker starts its own `Xvfb -displayfd` and sets `DISPLAY`
(`e2e/support/display.ts`); `run-e2e.mjs` no longer wraps the run in
`xvfb-run`, and `autoXvfb: false` stops WebdriverIO wrapping workers in its
own when `DISPLAY` is unset. `x11-input.py` opens `$DISPLAY`, so real input
follows the worker. `maxInstances` is `HOPLODEX_E2E_WORKERS`, by default half
the CPUs, at most 4. Isolation: the sandbox, keyring file and documents folder
were already per worker; us5's export folders are `mkdtemp`s; the two
screenshot specs write different file names into the shared output folder.

## Step 6 — Split the long E2E spec files

**Why.** wdio parallelizes by file, so after step 5 the longest file sets the
floor. At baseline the four longest were 93, 72, 71 and 66 s.

**Read.** Only the spec being split, plus `e2e/support/ui.ts` as needed.

**Do.**
- Split along the existing `describe` blocks. us12-accessories already has
  four, one per user story, so it splits into
  `us12-accessories-<story>.e2e.ts`.
- Each new file needs its own setup (`createDatabase()`, seeded firearms),
  because each file is a fresh app and sandbox. Move shared fixtures into
  `e2e/support/`.
- Keep the user-story prefix so the files still map to `spec.md`.
- us1 and us3 have 17 and 16 `it`s in one `describe`. Split them only where
  the tests don't depend on earlier ones' data; read the setup first.

**Verify.** The same number of `it`s before and after
(`grep -cE '^\s*it\(' e2e/specs/*.ts`). All pass 3 times.

**Result.** _(before → after)_

## Step 7 — Faster release profile for E2E builds

**Why.** Every backend change makes the E2E build relink with fat LTO and one
codegen unit: 70 s.

**Read.** `src-tauri/Cargo.toml` (`[profile.release]`), `e2e/wdio.conf.ts`
(the build, and `application`'s path), `scripts/human-testing.sh` if it shares
the build.

**Do.**
1. Add a profile with `inherits = "release"`, `lto = false`,
   `codegen-units = 16` and `incremental = true`, named `e2e` or similar.
   Keep `panic = "abort"`, so behaviour on a panic matches the shipped app.
2. Build E2E and the seed with `--profile <name>`, and point `application`
   at `target/<name>/hoplodex`.
3. **Trade-off to state in DEVELOPMENT.md:** E2E then doesn't drive a
   byte-identical shipping binary. It already doesn't, because of
   `mock-keyring`. Before a release, run E2E once against `--release`
   (consider a `HOPLODEX_E2E_PROFILE=release` switch).

**Verify.** Touch `src-tauri/src/lib.rs` and time the E2E build (baseline
70 s). The full E2E run still passes.

**Result.** E2E rebuild after touching `lib.rs`: 70 s → 2–4 s (a comment-only
edit 2.3 s, a new function 3.9–4.1 s). Release relink measured the same way
in the same loaded session: 117 s. Full E2E run passed (14 of 14 spec files)
in 475 s against a 519 s baseline; the saving there is mostly step 3, since
this step only shortens rebuilds. `HOPLODEX_E2E_PROFILE=release` checked with
us6 (passes; builds `target/release`). Profile named `e2e`.

## Step 8 — Test-level policy for new features (SDD)

**Why.** E2E grows by about one test per acceptance scenario, because
`/speckit-plan` and `/speckit-tasks` turn scenarios into E2E tasks. Every
scenario still needs a test (constitution II), but most can be tested
somewhere far cheaper.

**Read.** `.specify/memory/constitution.md` (the Testing Standards
principle), `.specify/templates/plan-template.md`,
`.specify/templates/tasks-template.md`.

**Do.**
1. Draft the rule:
   - **E2E per user story:** one journey through the real app (keyboard-first,
     where the story calls for it), plus only what needs the real app: IPC
     wiring, relaunch, locking, sleep and shutdown, native file drops, focus
     and real-input behaviour, and the screenshot walk.
   - **Validation, duplicate, rounding and other edge-case scenarios:** tested
     at the Rust `ops` level (persistence) or in Vitest (UI behaviour), and
     cited by scenario ID in the test name or a comment, so traceability
     stays.
2. **Decision for the user:** put the rule in the plan template's testing
   section (light), or amend the constitution (a MINOR version bump with a
   Sync Impact Report, via `/speckit-constitution`).
3. Optional, one spec per session: list the existing E2E tests that only check
   validation or edge cases (us1 scenarios 7, 9, 10, 13–14 and 16 are
   candidates). For each, confirm a Rust or Vitest test covers the same rule,
   add one if not, then remove the E2E test. Record each move in the commit
   message.

**Result.** _(rule placed where; E2E tests moved)_

## Step 9 — Optional: one integration-test binary; cheaper test databases

Do these only if the earlier steps leave Rust build or test time a problem.

- **One binary.** Move `src-tauri/tests/*.rs` into
  `src-tauri/tests/integration/main.rs` with a `mod` per file. That's one link
  instead of 58 (baseline: a 22 s incremental test rebuild). It changes how
  you run one file (`--test x` becomes a name filter), so update
  DEVELOPMENT.md and the `tasks.md` commands in the active spec.
  `#[path]`-included modules (`human_seed_coverage_test` includes
  `examples/human_seed.rs`) need checking.
- **Cheaper test databases.** About 0.4 s of CPU per `TestDb` goes to key
  derivation. Copying a pre-made template file doesn't help, because opening
  derives the key again. The only real fix is opening with a raw derived key
  in tests, which changes security-sensitive code (`db/mod.rs`,
  `db/cipher.rs`, "never change a cipher setting"). Don't do it without the
  user's explicit agreement, and only if a slower machine makes it matter.
