# Quickstart: validating feature 010

How to show that the feature works. Commands run through the dev container
(CLAUDE.md, "Run tests in the dev container"). DEVELOPMENT.md's "Test" section
has the full forms, including single files.

## Automated

```sh
scripts/dev-container.sh npm test
scripts/dev-container.sh npm run audit:licenses
scripts/dev-container.sh bash -c 'npm run build && node scripts/licenses/notices.mjs --check'
scripts/dev-container.sh bash -c 'cd src-tauri && cargo test --features test-kdf'
scripts/dev-container.sh bash -c 'npm run build && npm run test:e2e -- --spec e2e/specs/databases.e2e.ts'
scripts/dev-container.sh bash -c 'npm run build && npm run screenshots'
scripts/dev-container.sh npm run bundle:linux -- --bundles appimage
```

Expected:
- every test passes;
- `audit:licenses` reports the npm and Rust license checks and then
  "notices: x86_64-unknown-linux-gnu N, x86_64-pc-windows-msvc N, aarch64-apple-darwin N components; 0 failures";
- `dist/notices.json` and `dist/NOTICES.txt` exist after the build;
- the screenshot walk shows contracts/ui.md §5's shots;
- the AppImage still builds (FR-017 keeps the ability; it isn't published).

The E2E journey and `scripts/licenses`' Vitest tests also run on Windows and
macOS (the per-OS batches, DEVELOPMENT.md), so each OS's build is checked
against its own components. The macOS run is the one that chooses the menu
bar item.

### Where each scenario and criterion is tested

| Scenario / criterion | Test |
|---|---|
| 010 US1-1, US1-3, US1-4, US1-6 | E2E `databases.e2e.ts`, "010 US1 - Anyone Running HoploDex Can See Its License and What It Is Built From": from the chooser, the statements, the version equal to `package.json`'s, the source address (selected text equals the address), the license disclosure's text starts with "GNU GENERAL PUBLIC LICENSE", the filter "sqlcipher" leaves SQLCipher, opened, its Zetetic copyright |
| 010 US1-2, SC-005 | Same journey: create and open a database, About from the database menu, then Escape; the database file's and the sandbox's `machine.json`'s SHA-256 (`sandbox.sha256Of`), taken just before About opened, are unchanged after it closed, and the dialog sent no command (Vitest: `AboutDialog.test.tsx` asserts no `invoke`) |
| 010 US1-5 | Vitest `AboutDialog.test.tsx` (a fixture document with a NOTICE text and an attribution shown word for word); `scripts/licenses/notices.test.ts` (the dictionary's `NOTICE.md` carried verbatim from the real package) |
| Each OS's own components (FR-009, SC-001) | E2E, same journey, per OS: the count above the list equals the components in the `dist/notices.json` the build embedded; on Linux `webkit2gtk` is listed and `objc2` isn't; on macOS `objc2` is listed and `webkit2gtk` isn't; on Windows "openssl" finds OpenSSL at a 3.x version under Apache-2.0, "webview2" finds "Microsoft WebView2 loader" under BSD-3-Clause, and `webkit2gtk` isn't listed |
| 010 US1-7 | E2E, same journey, macOS only: `chooseMenuBarItem(appPid(), "About HoploDex")` with the chooser showing and again with a database open, with no system About panel (only one window). Rust unit test: the menu's About item has id `about` and the event name |
| 010 FR-005 and the no-match edge case | Vitest `AboutDialog.test.tsx` |
| Lock while open (spec Edge Cases) | Vitest `SessionProvider.test.tsx` (a `session:closing` closes About); E2E journey: Ctrl/⌘+L with About open returns to the chooser with no dialog |
| FR-007 narrow window | Screenshot walk `NN-about-narrow-*`; Vitest checks the regions' names |
| 010 US2-1, SC-001 | `scripts/licenses/notices.test.ts` "the host's notices list exactly the audit's shipped set" (real trees for the host's target, run on each OS; skipped with a message without `cargo`); `audit:licenses` `--all-targets`; the E2E journey's per-OS row above |
| 010 US2-2, SC-002 | `scripts/licenses/notices.test.ts` with fixtures: adding a fixture crate adds its notice, removing it removes it |
| 010 US2-3, FR-010 | Fixtures: a crate with no license file fails naming it; with an exception it passes; a stale exception fails; an unplaceable file fails; a clarification whose SHA-256 changed fails |
| 010 US2-4, FR-009 | Fixtures: carrier version change, carrier missing, feature off, OpenSSL 1.1 headers, each failing with the entry's name |
| 010 FR-012 | `scripts/licenses/vitePlugin.test.ts`: a bundled module from a package outside the shipped set fails the build; `scripts/licenses/policy.test.ts`: `ALLOWED` equals `deny.toml`'s `allow` |
| 010 US2-5, FR-011 | `scripts/licenses/notices.test.ts`: `NOTICES.txt` from a fixture matches contracts/notices.md's layout; the real host target's `NOTICES.txt` begins with the program's name, version and copyright line, has the additional permission after the source address, the EFF CC BY 4.0 attribution from research.md §2, and ends with `LICENSE`'s text; the build test finds both files in `dist/` |
| Native-code guard | Fixtures: a crate with a `cc` build dependency or a `.lib` file, unlisted, fails |
| 010 US3-1, US3-2, SC-006 | research.md §1 and §2 (reviewed in the PR) |
| 010 US3-3 | Vitest `strength.test.ts`: the estimator's dictionary has no `wikipedia` key, and the existing "password" and five-word-phrase answers hold; build check: the worker bundle lacks `wikipedia.json`'s encoded opening |
| 010 US3-4, FR-017 | DEVELOPMENT.md and `scripts/build-appimage.sh`'s header (reviewed); the Linux gate run builds the AppImage; #19 close-out comment |
| 010 FR-019 | `scripts/licenses/policy.test.ts`: README's "License" section contains `licenses/additional-permission.txt` verbatim; Vitest: About shows it |
| SC-003 | E2E journey counts: the chooser button (1), then the disclosure (2) |
| SC-004 | E2E journey: About shows its list within 1 s of the click, filtering shows its result within 500 ms, and scrolling the dialog's body from top to bottom with the whole list shown has no `requestAnimationFrame` gap over 100 ms (timed in the spec) |

## Manual check (best effort before a release, not a merge gate)

Everything else above is automated. This one needs a person with a screen
reader.
It is deferred to #48 with the earlier features' screen reader checks, and
run with #37's list before a release.

### Setup S1 (Linux or macOS)

1. From the checkout, run `scripts/human-testing.sh --reset --release`.
2. Wait for the app to open on the database chooser, listing "Main
   collection" and "Shared collection".

### M1: A screen reader reads About (Linux with Orca, or macOS with VoiceOver)

1. Do Setup S1 with the screen reader on.
2. Select **Main collection**, type `human testing passphrase` in
   **Passphrase**, and press Enter.
3. Open the database menu (the database's name in the top bar) and choose
   **About HoploDex**.
4. Expected: the reader announces the dialog "About HoploDex" and the
   collapsed button **GNU General Public License, version 3**.
5. Press Enter. Expected: "expanded", and the text region "GNU General Public
   License text" follows.
6. Press Tab until **Find a component** is announced. Type `sqlite`.
7. Expected: the reader announces the count, "N of M components", once
   typing stops.
8. Press Escape twice. Expected: the dialog closes and the reader is back on
   the database menu's button.
