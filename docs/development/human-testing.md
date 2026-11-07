# Human testing

_Part of the [development docs](../../DEVELOPMENT.md)._

To poke at the app by hand (look and feel, workflows) against a realistic
collection instead of an empty one:

```bash
scripts/human-testing.sh                # seed on first use, then launch `tauri dev`
scripts/human-testing.sh --reset        # throw the data away and reseed
scripts/human-testing.sh --extra 200    # also generate 200 plain firearms
scripts/human-testing.sh --release      # launch the E2E-profile build, not `tauri dev`
scripts/human-testing.sh --reset --pending-from 1.2.0   # reseed, with pending changes another version kept
```

`--release` is for trying what a development build doesn't have: the browser
controls (no reload, no Inspect Element, a trimmed right-click menu; 008
research.md §12-§15) are installed only in release and E2E builds. It runs
`npm run build` and `cargo build --profile e2e --features custom-protocol,e2e`,
as `npm run test:e2e` does, and launches that binary with the
`HOPLODEX_E2E_*` variables pointing into the same `.human-testing/` sandbox
(plus `XDG_*` or `HOME`, as [Test isolation](test-isolation.md) says). Because it is an E2E build, saved
passphrases are kept in `.human-testing/keyring.json` rather than the OS
keyring, the confirmation before a document opens in another app is answered
"open" without asking, and the E2E suite's WebDriver server listens on
127.0.0.1:4445 (`TAURI_WEBDRIVER_PORT`) while it runs, unauthenticated, so close
the app when you are done.

`--pending-from <version>` is passed to the seed, so it acts only when the seed
runs (with `--reset`, or on first use). The Shared collection's pending changes
are then recorded as kept by that version of HoploDex, in a form this build
can't resume (a draft form version it doesn't know, and `last_saved_version`
set to the version you gave). Open "Shared collection" to see the dialog that
names that version and offers only **Close the database** and **Discard
changes** (008 quickstart.md, US4). Without it, the
pending changes are this build's and resume.

The seed (`src-tauri/examples/human_seed.rs`) goes through the app's own
command layer, so it covers photos, documents, dispositions and every
insurance state: healthy, under-insured, uninsured (expired policy), and a
policy expiring soon. Policy dates are relative to the day it is seeded.
Some firearms carry real photos from `src-tauri/examples/seed-photos/`, all
public domain or freely licensed, whose `README.md` records each one's
source, licence and the credit line to use if they appear on a web page; one
record carries ten generated images for size and count testing. The
data lives in `.human-testing/` (git-ignored): two databases in `HoploDex/`,
"Main collection" (the full collection) and "Shared collection" (left open
by "Workshop PC", with pending changes), both opened with the passphrase the
script prints; two backups of "Main collection" in `Backups/`, its custom
backup folder; the `machine.json` listing both databases; and a set of
spreadsheets in `import-samples/` to try the top bar's Import with: clean,
conflicting and invalid rows, sheets exported before a column existed, mounts,
accessories and registrations, and one the import refuses as a whole,
`import-refused-too-many-sheets.xlsx` (20 sheets, where an import takes at
most 16), to see the refusal in the dialog's banner. The app is pointed at it, so your real collections are never opened:
on Linux through `XDG_*_HOME` and a `user-dirs.dirs` whose documents folder
is the sandbox; on macOS through a `HOME` of its own, `.human-testing/home/`,
whose `Library/Application Support` is `config/` and whose `Documents` is the
sandbox (the web view's data and caches land in it too). Linux and macOS
only. A `.human-testing/` made before the passphrase model has
no sandbox marker, and the seed refuses it: delete it and run the script
again.

**Changing the data model?** Update the seed in the same change. The seed
compiles when a new field is simply left out, so
`src-tauri/tests/human_seed_coverage_test.rs` runs it into a temporary
sandbox and fails if any column is empty in every row of both databases, if a
`CHECK ... IN` value never appears in either, or if no import sample fills a
spreadsheet column. Fix a
failure by seeding a record that uses the new field (and adding it to the
import samples), not by loosening the test.

In the [development container](dev-container.md),
run it as `scripts/dev-container.sh --gui scripts/human-testing.sh`. The
container keeps the data in `~/human-testing` in its home volume (via
`HUMAN_TESTING_DIR`), not in the checkout's `.human-testing/`.
