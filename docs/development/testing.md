# Test

_Part of the [development docs](../../DEVELOPMENT.md)._

```bash
cargo nextest run --manifest-path src-tauri/Cargo.toml --features test-kdf   # Rust unit + integration tests, real temp SQLCipher DB (on Windows, in a desktop session: windows.md)
cargo test --manifest-path src-tauri/Cargo.toml --doc                        # Rust doctests (nextest doesn't run them; there are none today)
npm test                                                                     # Vitest frontend unit tests (jsdom)
npm run build && npm run test:e2e                                            # WebdriverIO E2E, driven against the built app
```

## Run part of a suite

To run part of a suite:

```bash
cargo nextest run --manifest-path src-tauri/Cargo.toml --features test-kdf -E 'test(/^firearm_lifecycle_test::/)'   # one integration-test file
cargo nextest run --manifest-path src-tauri/Cargo.toml --features test-kdf -E 'test(/^firearm_lifecycle_test::/) & test(<name>)'  # tests in it whose name contains <name>
cargo nextest run --manifest-path src-tauri/Cargo.toml --features test-kdf <name>                                   # tests anywhere whose name contains <name>
npx vitest run src/features/firearms/FirearmForm.fields.test.tsx                                                    # one Vitest file
npm run test:e2e -- --spec e2e/specs/record-firearm.e2e.ts                                                      # one E2E spec

cargo test --manifest-path src-tauri/Cargo.toml --features test-kdf --test integration firearm_lifecycle_test::  # the same without nextest
```

## How the Rust tests are laid out

Every file in `src-tauri/tests/` is a module of one test program,
`tests/integration.rs`, so the test crate links once instead of once per file
(a rebuild after a change to `src/` took 8 s with a program per file, 3 s with
one), and a test's name starts with its file's name: `firearm_lifecycle_test::`.
`autotests` is off, so **a new test file needs a `mod` line in
`tests/integration.rs`** (`test_files_test` fails when one is missing), and a
test file refers to the shared helpers as `crate::support`,
`crate::preview_support` and `crate::sheets`. The exceptions, each its own
`[[test]]` in `Cargo.toml`, are the two release-only tests below,
`performance_test` and `import_largest_test`; `macos_screen_lock_test`,
which has its own `main`; `portability_test` and `tiff_preview_test`,
which set environment variables for a test. That is sound only while no
other thread reads the environment, and `cargo test` runs a program's tests
as threads of one process; `release_upgrade_test`, which keys with the
pinned KDF (below); and `log_contents_test`, which installs the process-wide
logger in child processes it starts. A test in one of these isn't prefixed with its
file's name: select it with `-E 'binary(portability_test)'`, or
`--test portability_test` under `cargo test`.

## The `test-kdf` feature

**`--features test-kdf`** (#77 step 11) keys every test database with 1,000
PBKDF2 iterations instead of the pinned 1,000,000, which was three quarters of
the Rust tests' CPU; every other cipher setting and every code path is the
app's. The tests pass without it too, only slower. A file a `test-kdf` build
makes opens in no other build, so the feature is for the tests alone:
`db/cipher.rs` won't compile it with `custom-protocol` or `e2e` or without
debug assertions (so never in a release, E2E or shipped build), and
the app itself (`cargo run`, `tauri dev`), `human_seed` and `portable_fixture` refuse to run with it, though the tests still start the binary's helpers. `test_kdf_startup_test` checks the app's refusal; it starts the app in a sandbox, since a `test-kdf` build, like an E2E build, takes its directories only from the `HOPLODEX_E2E_*` variables (`app_dirs.rs`). `portability_test`
keys with the pinned KDF whatever the build (`cipher::test_kdf::use_real_kdf`,
which applies to its whole process), so the committed fixture and the pinned
settings are checked in every run; so does `release_upgrade_test`, for the
stored release databases. Both take `support::one_at_a_time_with_the_real_kdf()`
first in every test, so a test program that needs the pinned KDF is one of
their own, never `integration`. A test that makes two backups of one file
needs them a second apart, since a backup's name says the second it was made:
use a `ManualClock` and move it on, rather than sleeping.

## Vitest

Vitest runs every test file in its own worker thread and uses every CPU
(`pool` and `maxWorkers` in `vite.config.ts`, #77), so a run's wall time is
about the slowest file plus the rest divided by the CPU count. Keep a file
well under ten seconds: `FirearmForm`'s tests are four files by topic
(`FirearmForm.*.test.tsx`, sharing `firearmFormTestSupport.tsx`) for that
reason. Vitest's end-of-run summary shows each file's time.

## Release-only tests

The performance budgets (search 500 ms, actions 1 s, suggestions 50 ms at
10,000 records) are timed only in a release build, so `performance_test.rs`
is `#[ignore]`d and the default run skips it. **Run it before opening
any pull request that touches search, listing, persistence or the `ops` layer
(the pull request must note its impact against the budgets, per the
constitution), and before every release.** Run the tests one at a time so that
none is timed while another copies a database, and read the timings with
`--nocapture`:

```bash
CARGO_PROFILE_RELEASE_PANIC=unwind cargo test --manifest-path src-tauri/Cargo.toml \
  --release --features upgrade-test-steps --test performance_test -- --ignored --nocapture --test-threads=1
```

`panic=unwind` is needed because `cargo test --release` builds the library
for the tests with unwinding and for the binary with the profile's
`panic = "abort"`, and the two then fail to link. `upgrade-test-steps` is the
upgrade's test hooks (see [Changing the data layout](data-layout.md)),
which the upgrade's timing in this test uses; it is on in every `test-kdf`
build, but this one can't take `test-kdf`: `db::cipher` refuses it without
debug assertions, and the budgets must be timed in a release build.

The import has a release-only test of its own, `import_largest_test.rs`
(008 research.md §9, SC-003). It writes the largest
export the app can make, 10,000 firearms and 10,000 accessories with every
text field at its maximum, as one workbook and as two CSV files, then reads
each through `import_reader::read_files` and imports it into a fresh database.
Nothing may be refused and every row must import; it prints the reading time,
the import time and the process's peak memory. It takes minutes and a few
hundred MB of disk, it does not exist in a debug build, and it is `#[ignore]`d,
so the default run skips it. **Run it before a pull request that touches the
import's reading or matching (`services/import_reader/`,
`commands/import_export.rs`), and note the three figures in the pull request.**
The [release workflow](ci.md#the-release-workflow) runs the same command for
every release:

```bash
CARGO_PROFILE_RELEASE_PANIC=unwind cargo test --manifest-path src-tauri/Cargo.toml \
  --release --test import_largest_test -- --ignored --nocapture
```

`HOPLODEX_LARGEST_ROWS=500` (any count) sets the number of firearms and of
accessories, for a quick check of the test itself; its timings then mean
nothing.

This command stays on `cargo test` on purpose: nextest runs each test in its
own process, so it would repeat the 10,000-record seeding that
`performance_test.rs` does once per process under `cargo test`. If you do run
it with `cargo nextest run --release --run-ignored all`, `src-tauri/.config/nextest.toml`
puts `performance_test` in a one-at-a-time group that also holds every CPU
slot, so no other test runs beside it.
