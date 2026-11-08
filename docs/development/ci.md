# Continuous integration

_Part of the [development docs](../../DEVELOPMENT.md)._

Five workflows run on GitHub: the [CI workflow](#the-ci-workflow)
(`.github/workflows/ci.yml`) and CodeQL (`.github/workflows/codeql.yml`) on
every pull request to `main` and on pushes to it (CodeQL weekly too), the
[dependency audit](dependency-audit.md) (`.github/workflows/audit.yml`), the
check of a pull request description's required sections
(`.github/workflows/pr-body.yml`), and the [release workflow](#the-release-workflow)
(`.github/workflows/release.yml`) on a `v*` tag. In all of them, third-party actions are
pinned to a commit (#72) and GitHub's own to a release tag. Dependabot
(`.github/dependabot.yml`) opens a weekly pull request to update them, and
another for the [vcpkg pin](dependency-audit.md#the-pinned-openssl).

On a pull request, the audit skips its work when the change can't affect it:
when the pull request changes only Markdown, or only `codeql.yml`,
`.github/codeql/` and `pr-body.yml`, which nothing the audit runs reads. The job
diffs the pull request's merge commit against `main` (`--no-renames`, so a
renamed file counts under both names) and skips its remaining steps, not the
job or the workflow, so the check still reports a result. A fault in deciding
fails the job rather than skipping the audit. The scheduled and manual runs
always audit.

CodeQL and the PR description check always run. The description check checks
the description, not the files. CodeQL can't skip because of the `Default`
ruleset's code scanning rule: a pull request whose head has no CodeQL analysis
is blocked from merging, whether the analysis was skipped by a `paths` filter,
a job-level `if` or a skipped step. #142 showed it: with the analysis steps
skipped, its head got no `CodeQL` code scanning check and stayed `BLOCKED`, and
once the next head was analyzed it was `CLEAN`. Skipping CodeQL on a docs-only
pull request would therefore need the ruleset changed, or a placeholder
analysis uploaded.

## The CI workflow

`.github/workflows/ci.yml` (#25) runs on every pull request to `main` and on
every push to it. It is a second net, not the first: run the tests in your own
Linux, macOS and Windows environments before you open a pull request, and say
in it which ran where. The jobs run natively on pinned images (`ubuntu-26.04`,
`windows-2025`, `macos-26`), not in the dev container:

| Job | What it runs |
|---|---|
| Checks (Linux) | `npm run format:check`, `cargo fmt --check`, `npm run lint`, both clippys ([Linting & formatting](lint.md)), `npm test`, `cargo nextest run --features test-kdf --no-fail-fast`, the doctests, `npm run build` |
| Checks (Windows, macOS) | both clippys, `npm test`, nextest as above |
| E2E (Linux, Windows, macOS) | `npm run build`, `npm run test:e2e`, then the quit-cleanup script for that OS |
| Packages (Linux) | On `ubuntu-24.04`, which sets the packages' glibc floor: `npm run bundle:linux -- --bundles deb,rpm`, which then runs `scripts/check-no-webdriver.mjs` on the binary, then `scripts/check-package.mjs` checks the `.deb` installs `/usr/bin/hoplodex`, `NOTICES.txt` and apport's ignore entry for it (#166), and the `.rpm` the first two; no AppImage, since Linux ships the `.deb`, `.rpm` and Flatpak (010 FR-017) |
| Packages (Windows) | `npm run tauri build -- --config src-tauri/tauri.bundle.conf.json --bundles nsis` (no MSI, #19), then `scripts/check-no-webdriver.mjs` on `hoplodex.exe` and `scripts/check-package.mjs` on the installer (7-Zip lists `hoplodex.exe` and `NOTICES.txt`) |
| Packages (macOS) | `npm run tauri build -- --config src-tauri/tauri.bundle.conf.json --bundles app,dmg`, then `scripts/check-no-webdriver.mjs` on the binary and `scripts/check-package.mjs` on the `.dmg`: the `.app` in it has `NOTICES.txt` and a valid ad-hoc signature (`codesign --verify`) |
| Flatpak | `npm run bundle:flatpak`, whose manifest runs `scripts/check-no-webdriver.mjs` on the release binary (the check's variant carries the driver on purpose); then, only when the pull request changes one of the paths [The Flatpak check](flatpak.md#the-flatpak-check) lists, the check's stand-ins and `npm run build && npm run test:flatpak` |

The packages are those a release publishes, built the same way, but a pull
request's run keeps them nowhere; the jobs show they build and pass their
checks. `performance_test` and `import_largest_test` don't run
here; [Test](testing.md) says when to run them.

- **One app at a time on Windows and macOS.** E2E there runs one worker
  (`wdio.conf.ts`'s default off Linux), since workers would share a desktop
  and another worker's window can close an open menu ([E2E](e2e.md)); Linux
  runs half the runner's CPUs, each worker on a display of its own.
- **Optional: E2E on macOS.** The runner has 3 CPUs and 7 GB, which one worker
  fills, so the job runs the suite on every pull
  request but passes whatever it finds. A failure, or a hang that the step's
  timeout (40 minutes, 5 for quit-cleanup) ends, appears in the run's summary
  and as a warning naming each failed spec file; record it in an issue so it
  is tracked.
- **Docs-only pull requests** skip each job's steps, as the audit does: a
  pull request that changes only Markdown (`*.md`) passes without them
  (`.github/actions/scope`). A change to any other file, the other workflows'
  included, runs them. Vitest reads some Markdown (the spec citations,
  `README.md`), so a docs-only change that breaks it shows on the next pull
  request that runs it.
- **Caches** (npm, one Rust cache per OS and profile, and the Windows
  OpenSSL) are saved only by pushes to `main` and restored by every other run
  (`.github/actions/setup`), about 3 GB of the repository's 10 GB. The release
  builds and the Flatpak installation (6.3 GB, fetched in about a minute) have
  none.
- **What each runner needs** is set up by its job: on Linux, every Ubuntu
  package it uses, named rather than left to the image, from the sets in
  `.github/actions/apt` (`build` is the Debian/Ubuntu list in
  [Prerequisites](prerequisites.md); `tests`, `e2e`, `flatpak` and
  `flatpak-check` add what those steps run); on Windows, `TEMP` on the D: disk with a Defender call for the
  tests, which makes nextest three times faster there, and the desktop at
  1920×1080 with animations on for E2E; on macOS, Reduce Motion off and the
  screen at 1920×1080 (`displayplacer`) for E2E. The release workflow's surface
  checks set the same screen size on both, since the default 1024×768 is shorter
  than their window.

**Timing budgets under CI.** Shared runners' speed varies too much for a
timing budget to mean anything, so under CI, when the `CI` variable is `true`
(GitHub sets it), the budget assertions are skipped and the tests still run
and make every other check. Rust tests write one as
`support::assert_budget!(took < budget, ...)` (`src-tauri/tests/support/mod.rs`),
and E2E specs as `if (budgetsChecked()) expect(took).toBeLessThan(budget)`
(`e2e/support/budgets.ts`). Both look at `CI` the same way; everywhere else
(your machine, the dev container, the VMs) the budgets are enforced. A budget
is an upper bound on how fast the code must be, such as an SC's (`007 SC-001`,
`010 SC-004`, `006 SC-001`, `005 SC-001`, `011 SC-002`, `011 SC-006`) or a
named one (`import_reader_test`'s refusal and stop budgets). A bound that
tells waiting out a time-out from not waiting (`portal_opener_test`), and a
lower bound, are behavior, and stay checked. Put a new budget through the
helper, so CI stays quiet about it and your own runs don't.

**Reproducing a job locally.** Each step is a command from these documents, so run
it in the dev container (Linux), the macOS VM or on Windows. To see a test
as CI does, with its budgets skipped, set `CI=true` for that run
(`CI=true cargo nextest run ...`). The Windows jobs' OpenSSL is the pinned
one: [Windows](windows.md) says how to build against it.

**The vcpkg pin.** `scripts/windows/vcpkg/vcpkg.json` pins the vcpkg commit
(`builtin-baseline`) whose OpenSSL every Windows job links statically
(`openssl:x64-windows-static-md`). CI fetches vcpkg at that commit, builds
OpenSSL into the fixed folder `C:\hoplodex-openssl` (fixed, because
libsqlite3-sys doesn't rebuild SQLCipher when `OPENSSL_DIR` moves) and caches
it under a key from the manifest, so it is built again only when the pin
moves. Dependabot moves the pin weekly; `npm run audit:openssl` checks the
OpenSSL it resolves to ([The pinned OpenSSL](dependency-audit.md#the-pinned-openssl)).

## The release workflow

`.github/workflows/release.yml` (#19) runs on a `v*` tag and opens a draft
GitHub release; [RELEASING.md](../../RELEASING.md) is the process around it.
It calls this CI workflow (`workflow_call`) with its `release` input set, which
changes only this: E2E runs against the shipping profile
(`HOPLODEX_E2E_PROFILE=release`), macOS E2E counts, the Flatpak job runs
`test:flatpak` and then `test:flatpak -- --fresh-install`, and the package
jobs keep what they built as artifacts. Its own jobs check the tag and the
release's AI security review, run `npm run audit`, `import_largest_test` and
the [surface checks](surface-checks.md) on each OS, then draft the release.
Run by hand (`workflow_dispatch`), it is a dry run of every job but the draft.
Keep the job names in `ci.yml` as they are: the `Default` ruleset requires
them by name.
