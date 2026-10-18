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
```

Expected:
- every test passes;
- `audit:licenses` reports the npm and Rust license checks and then
  "notices: x86_64-unknown-linux-gnu N, x86_64-pc-windows-msvc N, aarch64-apple-darwin N components; 0 failures";
- `dist/notices.json` and `dist/NOTICES.txt` exist after the build;
- the screenshot walk shows contracts/ui.md §5's shots.

The E2E journey also runs on Windows and macOS (the per-OS batches,
DEVELOPMENT.md). The macOS run is the one that chooses the menu bar item.

### Where each scenario and criterion is tested

| Scenario / criterion | Test |
|---|---|
| 010 US1-1, US1-3, US1-4, US1-6 | E2E `databases.e2e.ts`, "010 US1 - Anyone Running HoploDex Can See Its License and What It Is Built From": from the chooser, the statements, the source address (selected text equals the address), the license disclosure's text starts with "GNU GENERAL PUBLIC LICENSE", the filter "sqlcipher" leaves SQLCipher, opened, its Zetetic copyright |
| 010 US1-2, SC-005 | Same journey: open the seeded database, About from the database menu, then close; the database file's SHA-256 (`sandbox.sha256Of`) is unchanged and the dialog sent no command (Vitest: `AboutDialog.test.tsx` asserts no `invoke`) |
| 010 US1-5 | Vitest `AboutDialog.test.tsx` (a fixture document with a NOTICE text and an attribution shown word for word); `scripts/licenses/notices.test.ts` (the dictionary's `NOTICE.md` carried verbatim from the real package) |
| Windows' linked components (FR-009) | E2E, same journey, Windows only: the filter "openssl" finds OpenSSL at a 3.x version under Apache-2.0, and "webview2" finds "Microsoft WebView2 loader" under BSD-3-Clause |
| 010 US1-7 | E2E, same journey, macOS only: `chooseMenuBarItem(appPid(), "About HoploDex")` with the chooser showing and again with a database open, with no system About panel (only one window). Rust unit test: the menu's About item has id `about` and the event name |
| 010 FR-005 and the no-match edge case | Vitest `AboutDialog.test.tsx` |
| Lock while open (spec Edge Cases) | Vitest `SessionProvider.test.tsx` (a `session:closing` closes About); E2E journey: Ctrl/⌘+L with About open returns to the chooser with no dialog |
| FR-007 narrow window | Screenshot walk `NN-about-narrow-*`; Vitest checks the regions' names |
| 010 US2-1, SC-001 | `scripts/licenses/notices.test.ts` "the Linux notices list exactly the audit's shipped set" (real trees; skipped with a message without `cargo`); `audit:licenses` `--all-targets` |
| 010 US2-2, SC-002 | `scripts/licenses/notices.test.ts` with fixtures: adding a fixture crate adds its notice, removing it removes it |
| 010 US2-3, FR-010 | Fixtures: a crate with no license file fails naming it; with an exception it passes; a stale exception fails; an unplaceable file fails; a clarification whose SHA-256 changed fails |
| 010 US2-4, FR-009 | Fixtures: carrier version change, carrier missing, feature off, OpenSSL 1.1 headers, each failing with the entry's name |
| 010 FR-012 | `scripts/licenses/vitePlugin.test.ts`: a bundled module from a package outside the shipped set fails the build; `scripts/licenses/policy.test.ts`: `ALLOWED` equals `deny.toml`'s `allow` |
| 010 US2-5, FR-011 | `scripts/licenses/notices.test.ts`: `NOTICES.txt` from a fixture matches contracts/notices.md's layout; the build test finds both files in `dist/` |
| Native-code guard | Fixtures: a crate with a `cc` build dependency or a `.lib` file, unlisted, fails |
| 010 US3-1, US3-2, SC-006 | research.md §1 and §2 (reviewed in the PR) |
| 010 US3-3 | Vitest `strength.test.ts`: the estimator's dictionary has no `wikipedia` key, and the existing "password" and five-word-phrase answers hold; build check: the worker bundle lacks `wikipedia.json`'s encoded opening |
| 010 US3-4, FR-017 | DEVELOPMENT.md and `scripts/build-appimage.sh`'s header (reviewed); #19 close-out comment |
| 010 FR-019 | `scripts/licenses/policy.test.ts`: README's "License" section contains `licenses/additional-permission.txt` verbatim; Vitest: About shows it |
| SC-003 | E2E journey counts: the chooser button (1), then the disclosure (2) |
| SC-004 | E2E journey: About shows its list within 1 s of the click, and filtering shows its result within 500 ms (timed in the spec) |

## Manual checks (best effort before a release, not merge gates)

These need a release build, a person, or a screen reader.

### Setup S1 (Linux or macOS)

1. From the checkout, run `scripts/human-testing.sh --reset --release`.
2. Wait for the app to open on the database chooser, listing "Main
   collection" and "Shared collection".

### M1: A release build's notices match its OS

1. Do Setup S1.
2. In a terminal in the same checkout, run
   `node scripts/licenses/notices.mjs --check`. Note the component count it
   prints.
3. In the app, select **About HoploDex** in the chooser's top bar.
4. Expected: "Version" shows the version in `package.json`.
5. Expected: the count above the list matches step 2's count.
6. Type `webkit2gtk` in **Find a component**.
7. Expected: on Linux, the Rust bindings `webkit2gtk` and `webkit2gtk-sys`
   appear. On macOS, "No component matches “webkit2gtk”.".
8. Clear the field, then type `objc2`.
9. Expected: on macOS, the `objc2` crates appear. On Linux, no match.
10. Press Escape twice. Expected: the dialog closes and focus is on the
    **About HoploDex** button.

### M2: The bundles' plain-text notices

1. Do Setup S1.
2. Open `dist/NOTICES.txt` in a text editor.
3. Expected: it begins with "HoploDex", the version, and "Copyright (C) 2026
   exodious and the HoploDex contributors".
4. Expected: the additional permission follows the source address.
5. Expected: the last section is the full GNU General Public License,
   version 3.
6. Search for `EFF Large Wordlist`. Expected: the CC BY 4.0 attribution from
   research.md §2.

### M3: A screen reader reads About (Linux with Orca, or macOS with VoiceOver)

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
