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
| 010 US1-1, SC-001, SC-003, SC-004 | E2E `databases.e2e.ts`, `describe("010 US1 - Anyone Running HoploDex Can See Its License and What It Is Built From")`, "opens from the chooser in one action, with the statements, the version, the source address and every component": from the chooser's button, the list within 1 s, the statements, the version equal to `package.json`'s, the additional permission, no link in the dialog, and every component of the build's `dist/notices.json` listed under the count. Vitest `AboutDialog.test.tsx` ("AboutDialog's program block") for the statements before the fetch resolves; `DatabaseChooser.test.tsx` ("DatabaseChooser's About HoploDex button") |
| 010 US1-3, US1-4, SC-003 | Same describe, "shows the license in a second action, and SQLCipher's notice with Zetetic's copyright": the license disclosure's text starts with "GNU GENERAL PUBLIC LICENSE", the filter "sqlcipher" leaves SQLCipher, opened, its Zetetic copyright, "zzzz" shows the no-match line |
| 010 US1-6 | Same describe, first test: the source address is its own text, and selecting it gives the address. On Linux a real pointer press (`realClick`) selects it; on macOS and Windows, with no real-input helper, the check is the computed `user-select: all` and that selecting the node's children gives the address. Vitest `AboutDialog.test.tsx` ("gives the source address as selectable text in a monospace span, not a link") |
| 010 US1-2, SC-005 | Same describe, "opens from the database menu without reading or writing the database or machine.json, and Ctrl/⌘+L locks with it open": create and open a database, About from the database menu, then Escape; the database file's and the sandbox's `machine.json`'s SHA-256 (`sha256Of`), taken just before About opened, are unchanged after it closed. Vitest `AboutDialog.test.tsx` ("AboutDialog sends no command") asserts no `invoke`, rendering, filtering, opening rows or failing to load |
| 010 US1-5 | Vitest `AboutDialog.test.tsx` ("shows a NOTICE after the license text, word for word", "shows an attribution after the license text, word for word", on a fixture document); `scripts/licenses/notices.test.ts` ("carries the OpenSubtitles NOTICE.md word for word", from the real package) |
| Each OS's own components (FR-009, SC-001) | E2E, same describe, "lists the components this OS ships, and not another's", and the count above the list equal to the components in the `dist/notices.json` the build embedded (first test): on Linux `webkit2gtk` is listed and `objc2` isn't; on macOS `objc2` is listed and `webkit2gtk` isn't; on Windows "openssl" finds OpenSSL at a 3.x version under Apache-2.0, "webview2" finds "Microsoft WebView2 loader" under BSD-3-Clause, and `webkit2gtk` isn't listed |
| 010 US1-7 | E2E, same describe, "opens from the macOS menu bar, from the chooser and with a database open, in HoploDex's own window", macOS only: `chooseMenuBarItem(appPid(), "About HoploDex")` with the chooser showing and again, after unlocking, with a database open, with no system About panel (one window handle). Rust unit tests in `app_menu.rs`: `about_item_and_event_names_are_the_contracts`, `only_the_about_item_raises_the_event` and `the_frontend_listens_for_the_same_event`; Vitest `SessionProvider.test.tsx` ("SessionProvider and About HoploDex", the event in the starting, chooser and open phases) |
| 010 FR-001 (the three ways in) | Vitest `DatabaseChooser.test.tsx` ("DatabaseChooser's About HoploDex button"), `DatabaseMenu.test.tsx` (the menu lists "About HoploDex" after "About databases and security"), `SessionProvider.test.tsx` (the one dialog; a second call leaves it as it is) |
| 010 FR-005 and the no-match edge case | Vitest `AboutDialog.test.tsx` ("AboutDialog's components": the filter, the count, "shows the no-match line in place of the list") |
| Keyboard (010 FR-007; contracts/ui.md §3) | Vitest `AboutDialog.test.tsx` ("AboutDialog's keyboard": focus on the license disclosure's button once it loads, Escape clearing the filter and then closing, no Enter action, Enter and Space on a disclosure); E2E same describe, "clears the filter with the first Escape, closes with the second, and returns focus to the button" |
| Lock while open (spec Edge Cases) | Vitest `SessionProvider.test.tsx` ("closes, without asking, on a lock, which shows the closing screen", "closes, without asking, when a database opens", "stays open for a closing event of another session"); E2E same describe, the database-menu test above: Ctrl/⌘+L with About open returns to the chooser with no dialog |
| FR-007 narrow window | Screenshot walk `102-about-narrow-*` (`describe("Screenshots: About HoploDex")`); Vitest `AboutDialog.test.tsx` checks the regions' names |
| 010 US2-1, SC-001 | `scripts/licenses/notices.test.ts`: `describe("010 SC-001 - the host's notices against the real trees")`, "lists exactly the audit's shipped set" (real trees for the host's target, run on each OS; skipped with a message without `cargo`), and `describe("010 US2-1 - the components are exactly what the audit counts as shipped")` on fixtures; `audit:licenses` `--all-targets`; the E2E per-OS row above |
| 010 US2-2, SC-002 | `scripts/licenses/notices.test.ts` with fixtures (`describe("010 US2-1 …")`): "adding a crate to the tree adds its notice, and nothing else changes", "removing a crate from the tree removes its notice, and nothing else changes" |
| 010 US2-3, FR-010 | `licenseFiles.test.ts` ("fail closed: no text, no guess, no generic fallback": a crate with no license file fails naming it; an unplaceable file fails), `records.test.ts` ("notice exceptions": it passes with an exception, fails when the component now ships a license file, or its declared license changed; "clarifications": a changed SHA-256 fails), `notices.test.ts` ("a failed check names the component and every failure is collected") |
| 010 US2-4, FR-009 | `records.test.ts` ("hand-recorded components" and "the OpenSSL version comes from the build": carrier version change, carrier missing, feature off, OpenSSL 1.1 headers) and `notices.test.ts`, each failing with the entry's name |
| 010 FR-012 | `scripts/licenses/vitePlugin.test.ts` ("the bundle may hold only packages the audit counts as shipped": a bundled module from a package outside the shipped set fails the build, from either bundle); `scripts/licenses/policy.test.ts` ("the allow list is in step with deny.toml": `ALLOWED` equals `deny.toml`'s `allow`) |
| 010 US2-5, FR-011 | `scripts/licenses/notices.test.ts` ("010 US2-5 - NOTICES.txt": from a fixture it matches contracts/notices.md's layout; "writes a real NOTICES.txt": the real host target's begins with the program's name, version and copyright line, has the additional permission after the source address, the EFF CC BY 4.0 attribution from research.md §2, and ends with `LICENSE`'s text); `vitePlugin.test.ts` ("the build writes the notices": the plugin emits both files at the bundle's root, byte for byte what `notices.mjs --out` writes); `databases.e2e.ts` reads the `dist/notices.json` the build wrote |
| Native-code guard | `records.test.ts` ("native code": a crate with a `cc` build dependency or a prebuilt library, unlisted, fails) |
| 010 US3-1, US3-2, SC-006 | research.md §1 and §2 (reviewed in the PR) |
| 010 US3-3 | Vitest `src/components/strength/strength.test.ts` ("the strength hint's word lists": the estimator's dictionary has no `wikipedia` key and keeps every other list), with the existing "password" and five-word-phrase answers; build check: `vitePlugin.test.ts` ("withdrawn data never reaches a chunk") and `policy.test.ts` ("the withdrawn data files") fail a build whose worker holds `wikipedia.json`'s encoded opening |
| 010 US3-4, FR-017 | DEVELOPMENT.md and `scripts/build-appimage.sh`'s header (reviewed); the Linux gate run builds the AppImage; #19 close-out comment |
| 010 FR-019 | `scripts/licenses/policy.test.ts` ("the README's license notice": the "License" section contains `licenses/additional-permission.txt` verbatim); Vitest `AboutDialog.test.tsx` ("shows the additional permission after the statements once loaded") |
| SC-003 | E2E journey: the chooser button opens About (1 action; the statements and source address show), then the disclosure shows the full license (2) |
| SC-004 | E2E journey: About shows its list within 1 s of the click (first test), filtering shows its result within 500 ms (`filterBy`), and "scrolls the whole list with no frame over 100 ms" (no `requestAnimationFrame` gap over 100 ms, timed in the spec) |

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
