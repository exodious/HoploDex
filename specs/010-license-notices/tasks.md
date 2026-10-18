---

description: "Task list for feature 010: license notices in the app and the license audit's last items"
---

# Tasks: License Notices in the App and the License Audit's Last Items

**Input**: Design documents from `/specs/010-license-notices/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ (notices.md, ui.md, tauri-commands.md), quickstart.md (all present)

**Tests**: The project constitution's Testing Standards principle is NON-NEGOTIABLE: every user story below includes test tasks written first, expected to fail, then made to pass by the implementation, each at the level plan.md's Test Levels gives it. The notices generator is tested in Vitest against fixtures (a fake lockfile and `node_modules`, fake crate folders, recorded `cargo tree` and `cargo metadata` output), with one red fixture per failure rule, and once against the real trees (skipped, with a message, where `cargo` isn't on the PATH). About HoploDex is tested in Vitest, with one E2E journey in `databases.e2e.ts`. No test reads real data.

**Organization**: Tasks are grouped by user story (spec.md's P1–P3). **The phases run in dependency order, not priority order**: About's list (US1, P1) is the generator's output (US2, P2), and plan.md's "First tasks" put the generator, its records and a real `notices.json` before the interface. So Phase 3 is US2 and Phase 4 is US1. US1's Vitest tests and components use fixture documents and can start alongside US2 (different files); only its E2E journey and screenshots wait for US2's build. This feature is mostly new files (`scripts/licenses/`, `licenses/`, `AboutDialog`); no database, setting, command or capability changes.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: Which user story this task belongs to (US1–US3)
- Exact file paths are given per plan.md's Project Structure
- In this file, bare IDs (FR-001, US1, T012) mean this feature's. In code, comments and test names, cite them with the feature's number: `010 FR-010`, `010 US2-3`, `010 T012`, `010 research.md §7` (DEVELOPMENT.md, "Spec citations"); `npm test` fails on a bare one
- Work that must be built, tested or run on a specific platform gets one task per platform, starting with the platform's name

## Path Conventions

Existing Tauri desktop app: React/TypeScript frontend in `src/`, Rust backend in `src-tauri/` (touched only for the macOS menu), Node build tooling in `scripts/`, WebdriverIO E2E suite in `e2e/`. Committed license records go in a new top-level `licenses/`. Run tests, lint, the audit, the build and screenshots through `scripts/dev-container.sh` (CLAUDE.md; commands in quickstart.md). macOS work runs in the macOS 26 VM and Windows work on the Windows test machine (DEVELOPMENT.md's macOS and Windows sections). Run `npm run build` before `test:e2e` or `screenshots`: the E2E build embeds `dist/`, and `dist/notices.json` is now part of it. Never open the real databases, `machine.json` or keyring entries (CLAUDE.md, "Never touch the real databases"): About reads none of them, and the E2E journey uses a database it creates in its sandbox.

## How the stories divide the shared pieces

- **Foundational** owns the one license policy (`scripts/licenses/policy.mjs`, `spdx.mjs`, `shipped.mjs`'s `npmShipped`), moved out of `check-npm-licenses.mjs` (plan.md "First tasks" 1); dropping `wikipedia.json` from the strength hint (First task 2, since what ships decides what the notices list; it settles part of US3's FR-015); and the two facts that could stop the work: the WebView2 SDK's license and the fonts' own copyright strings (First task 4).
- **US2** owns the generator and everything that makes it match the build: `crateShipped`, `licenseFiles.mjs`, `records.mjs`, `notices.mjs` and its CLI, `vitePlugin.mjs` (the `__HOPLODEX_PROGRAM__` constant, the bundled-package check, the withdrawn-data check, emitting `notices.json` and `NOTICES.txt`, the development server's `/notices.json`), the committed records under `licenses/` (hand-recorded, exceptions, clarifications, native code, texts), `audit:licenses --all-targets`, and DEVELOPMENT.md's new "License notices" section.
- **US1** owns everything the user sees: `AboutDialog`, `aboutNotices.ts`, `about.css`, `showAbout()` and the dialog's life in `SessionProvider`, `sessionService.onAboutRequested`, the chooser's button, the database menu's item, the macOS application menu (`app_menu.rs`, `main.rs`) and its `app:about-requested` event, the E2E journey and the screenshot walk.
- **US3** owns the record and the rules around it: research.md §1–§4 rerun and dated, README's "License" section with the §7 permission and its test, `scripts/build-appimage.sh`'s header, DEVELOPMENT.md's "License audit" rewrite, and the amendments to 003.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Empty modules and the committed-records folder, so later tasks in different files can proceed in parallel.

- [X] T001 Create the build tooling's skeleton, each module with a header comment naming its research.md section and exporting nothing yet, and confirm Vitest, `npm run lint` and `npm run format:check` pick them up:
  - `scripts/licenses/policy.mjs` (research.md §5, §7), `spdx.mjs` (§5), `shipped.mjs` (§5, §6), `licenseFiles.mjs` (§5), `records.mjs` (§7, §8), `notices.mjs` (§5, §10; with a `#!/usr/bin/env node` CLI entry that exits 0), `vitePlugin.mjs` (§6, §10);
  - an empty `scripts/licenses/fixtures/` with a `README.md` saying each fixture folder is one case and naming the rule it shows;
  - `licenses/hand-recorded.json`, `licenses/exceptions.json`, `licenses/clarify.json` and `licenses/native-code.json`, each `[]`;
  - `licenses/additional-permission.txt` holding research.md §15's text exactly (the heading line "Additional permission under GNU GPL version 3 section 7", a blank line, then the paragraph), LF line ends, a final newline;
  - an empty `licenses/texts/` (with a `.gitkeep` until T009 and T021 fill it).

  Committed texts are `.txt` so Prettier leaves them as taken; if `npm run format:check` would rewrite `licenses/*.json` in a way the records' sort check (T018) rejects, settle it here (Prettier's JSON output is fine; the sort is by entries, not by keys)
- [X] T002 [P] Create the interface's and the menu's empty, compiling modules: `src/features/app/AboutDialog.tsx` (exports a component rendering `null`), `src/features/app/aboutNotices.ts`, `src/features/app/about.css` (a header comment naming contracts/ui.md §2), each with a header comment naming contracts/ui.md; and `src-tauri/src/app_menu.rs` with a module doc comment citing `010 research.md §11` and contracts/tauri-commands.md, declared as `mod app_menu;` in `src-tauri/src/main.rs`. `npm run lint` and `cargo build --manifest-path src-tauri/Cargo.toml` must pass in the dev container

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: One license policy shared by the audit and the notices, the strength hint's dictionary reduced to the lists that ship, and the facts that could stop the work (plan.md "First tasks" 1, 2 and 4).

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### First task 1: one policy for the audit and the notices (research.md §5, §6, §9)

- [X] T003 Write the policy's tests first; they fail until T004:
  - `scripts/licenses/spdx.test.ts`: `satisfies()` keeps today's behaviour (OR, AND, WITH, parentheses, an unbalanced or trailing token throws); `choose(expression, isAllowed)` resolves a choice (`OR`) to the first allowed license in `PREFERENCE` order ("MIT OR Apache-2.0" → MIT, "Apache-2.0 OR MIT" → MIT, "Zlib OR Apache-2.0 OR MIT" → MIT), keeps every part of a conjunction (`AND`), keeps a `WITH` exception whole ("Apache-2.0 WITH LLVM-exception"), and returns nothing when no choice is allowed;
  - `scripts/licenses/policy.test.ts`: `ALLOWED` equals the `allow` list in `src-tauri/deny.toml`'s `[licenses]` (parse the TOML array with a small reader in the test; no new dependency), both ways, naming any difference (research.md §9); `PREFERENCE` is exactly research.md §5's order (MIT, Apache-2.0, BSD-3-Clause, BSD-2-Clause, ISC, Zlib, Unicode-3.0, BSL-1.0, 0BSD, MIT-0, CC0-1.0, MPL-2.0, Apache-2.0 WITH LLVM-exception) and each is in `ALLOWED`; OFL-1.1 is allowed for `@fontsource/…` and `@fontsource-variable/…` packages and for nothing else; `blessing`, `CC-BY-4.0`, `ODC-By-1.0` and `LicenseRef-public-domain` are allowed only for the hand-recorded entries research.md §7 names, never for a package or crate (constitution, "Licensing");
  - `scripts/licenses/shipped.test.ts`: `npmShipped(lock)` over a small fixture lockfile returns the packages with neither `dev` nor `devOptional` (research.md §6), leaves out the root (`""`), and names a nested package by its last `node_modules/` segment, as `check-npm-licenses.mjs` does today

  Cite `010 FR-008`, `010 FR-012` and research.md sections in the test names
- [X] T004 Move the policy out of `scripts/check-npm-licenses.mjs` (a refactor, not a copy; CLAUDE.md memory "Refactor, not workaround"):
  - `scripts/licenses/policy.mjs`: `ALLOWED` (the script's set, with its "keep in step with deny.toml" comment now pointing to `policy.test.ts`), `EXCEPTIONS` scoped to packages (`packages: RegExp`) or to hand-recorded entries (`entries: string[]`), and `PREFERENCE`;
  - `scripts/licenses/spdx.mjs`: `satisfies()` moved unchanged, and `choose()`;
  - `scripts/licenses/shipped.mjs`: `npmShipped(lock)`, where `devOptional` now counts as development.

  `check-npm-licenses.mjs` imports all three and keeps its output format. Run it before and after (save both outputs in the scratchpad) and confirm the only change is that `@types/react`, `@types/react-dom`, `@types/prop-types` and `csstype` leave the shipped count (research.md §6); any other difference is a bug. T003's tests must now pass
- [X] T005 On Linux, run `scripts/dev-container.sh bash -c 'npx vitest run scripts/licenses && npm run audit:licenses && npm run lint'`; all must pass. Keep T004's before/after outputs for the PR notes

### First task 2: the strength hint without `wikipedia.json` (research.md §2)

- [X] T006 Write the strength tests first in `src/components/strength/strength.test.ts`; they fail until T007:
  - the estimator's dictionary (exported from `estimate.ts` for the test, e.g. `strengthDictionary()`) has no key containing `wikipedia` (`010 US3-3`, `010 FR-015`);
  - it has exactly the keys it has today less `wikipedia-en`: `commonWords-en`, `firstnames-en`, `lastnames-en`, the 14 `wordSequences` keys, `passwords-common`, `diceware-common` (read today's key list from `@zxcvbn-ts/language-en`'s and `-common`'s `dist/index.mjs` and write it out in the test, so a key that silently disappears fails);
  - the existing answers hold: "password" scores 0 with a suggestion, the five-word phrase scores 4 (003 FR-003)
- [X] T007 Change `src/components/strength/estimate.ts` to build the dictionary from the packages' per-list modules (`@zxcvbn-ts/language-en/dist/<list>.json.mjs` and `@zxcvbn-ts/language-common/dist/<list>.json.mjs`; confirm the exact file names and the `wordSequences` module's shape with `ls node_modules/@zxcvbn-ts/language-*/dist`), with the same keys as today less `wikipedia-en`, and keep `translations` and `adjacencyGraphs` from the entry modules (research.md §2: spreading `dictionary` reads every key, so leaving one out after the spread would not drop the list). Add an ambient `declare module` for the per-list modules in `src/vite-env.d.ts` (they ship without declarations). Keep the module comment about the worker (#102). T006's tests must now pass
- [X] T008 On Linux, run `scripts/dev-container.sh bash -c 'npx vitest run src/components/strength && npm run build'`, then confirm by hand that the built strength worker chunk in `dist/assets/` no longer contains `wikipedia.json`'s opening entries as the bundle encodes them (find them first in a build from `main`, and save that marker string for T015 and T016, which turn this into an automated check)

### First task 4: the facts the records rest on (research.md §3, §12)

- [X] T009 [P] Find the `Microsoft.Web.WebView2` SDK version whose `WebView2LoaderStatic.lib` `webview2-com-sys` 0.38.2 carries (the `webview2-rs` repository at that crate's tag: its build or update script, or its README), and take that package version's `LICENSE.txt`. **If it is not BSD-3-Clause, stop and tell the owner before going on** (plan.md "First tasks" 4; FR-019 and research.md §3, §15 rest on it). Otherwise commit it as `licenses/texts/webview2-loader/LICENSE.txt`, its first line `Source: <the URL of that file at that version>`, and add the version and the URL to research.md §3 with an `_Amended 2026-MM-DD (010 T009):_` note only if it changes what §3 says
- [X] T010 [P] Read the copyright string (`name` table, ID 0) from each of the seven `woff2` files a build emits into `dist/assets/` (Atkinson Hyperlegible Mono and Next, latin and latin-ext; Big Shoulders Display, latin, latin-ext and vietnamese), with a throwaway script in the scratchpad using only `node:zlib`'s Brotli (no new dependency). Write the seven strings into research.md §12. Big Shoulders' notice will use the font's own string and the upstream `OFL.txt` (T022's clarification); if either Atkinson family's string disagrees with its package's `LICENSE` copyright line, record that it gets the same treatment

**Checkpoint**: one policy; the strength hint ships only the lists that will be listed; the WebView2 SDK's license is confirmed.

---

## Phase 3: User Story 2 - Every Build Carries Notices That Match What It Ships (Priority: P2)

**Goal**: every `npm run build` writes `dist/notices.json` and `dist/NOTICES.txt` for its target, listing exactly the audit's shipped set and the hand-recorded components, and fails, naming the component, whenever a notice can't be found or a record has gone stale. Built before US1 because About's list is this story's output.

**Independent Test**: `npm run build` writes both files; `node scripts/licenses/notices.mjs --check --all-targets` passes; the fixtures show each failure rule failing with the component's name; the real-tree test matches the audit's count.

### Tests for User Story 2 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T011 [US2] Build the fixtures in `scripts/licenses/fixtures/`, one folder per case, so no fixture test needs `cargo`, the network or the real `node_modules`. The generator's functions take every path and command as parameters (the repository root, the lockfile, `node_modules`, the `licenses/` folder, the `LICENSE` file, `OPENSSL_DIR`, and a `cargo` runner that the fixtures replace with recorded output), so the fixtures pass them in. Include:
  - a small lockfile and `node_modules` with shipped, `dev` and `devOptional` packages, a package with a choice of licenses, one with a `NOTICE` file, one with no license file, one with only `LICENSE.spdx`, and an `@fontsource-variable/` package;
  - crate folders (each with a `Cargo.toml` naming its `license`) and recorded `cargo tree` (per triple) and `cargo metadata` outputs covering: a single-license crate; a "MIT OR Apache-2.0" crate with `LICENSE-MIT` and `LICENSE-APACHE`; one whose two licenses are in one file (as `chrono`); one with a pointer-only `COPYING` (as `siphasher`'s 281-byte file); one with texts under `LICENSES/`; one with a `NOTICE`; one with no license file; one with an unplaceable file; one with a `cc` build dependency; one carrying a `.lib`; a non-`hoplodex` path package; a crate shipped only on one triple;
  - a carrier crate standing for `libsqlite3-sys` with `sqlcipher/LICENSE`, with and without its `bundled-sqlcipher` feature in the recorded tree, and at two versions;
  - `include/openssl/opensslv.h` headers at `3.5.4` and at `1.1.1w`;
  - `licenses/` folders for the good case and for each red case: an unsorted file, an unknown field, a missing text file, a stale exception (the package now ships `LICENSE`), an exception whose declared license changed, a clarification whose file's SHA-256 changed, an unlisted native-code crate, a native-code entry no target ships
- [X] T012 [P] [US2] Write `scripts/licenses/licenseFiles.test.ts` against T011's crate and package folders (`010 US2-3`, `010 FR-010`, research.md §5):
  - files that count: `LICENSE*`, `LICENCE*`, `COPYING*`, `UNLICENSE*`, `NOTICE*`, `COPYRIGHT*`, `AUTHORS*` (the last three only beside a license file), in the root and in `LICENSES/` or `license/`;
  - a single-license crate gets all of them; a crate with a choice gets only the chosen license's files, placed first by name (`LICENSE-MIT`, `LICENSE-APACHE`, `LICENSE.Apache-2.0`, `LICENSE-ZLIB` and the like), then by content (MIT's "Permission is hereby granted, free of charge", Apache's "Apache License" with "Version 2.0", BSD's "Redistribution and use in source and binary forms", Zlib's "This software is provided 'as-is'", Unicode's "UNICODE LICENSE"); a file holding two licenses counts for both;
  - a NOTICE file is always kept, after the license text;
  - a pointer-only file isn't a text;
  - an unplaceable file in a crate with a choice is a failure naming the crate; no text for the chosen license is a failure naming the crate; there is never a generic fallback text
- [X] T013 [P] [US2] Write `scripts/licenses/records.test.ts` against T011's `licenses/` cases (`010 US2-4`, `010 FR-009`, `010 FR-010`, data-model.md), each failure asserted by its message, word for word as contracts/notices.md "Failure messages" gives it (component, version and triple where it shows them, and what to do):
  - **every record file**: sorted by its first field, no unknown field, else a failure naming the file and the entry;
  - **hand-recorded** (`[{ name, version, license, kind, os[], carrier: { crate|npm, version, feature? }, texts[], reason }]`; `os` one or more of `linux`, `windows`, `macos`; `kind` one of `compiled-in`, `linked`, `data`): fails, naming the entry, when the carrier isn't shipped on a listed OS, the carrier's version differs from the recorded one, the carrier feature isn't enabled, or a text file (`licenses/texts/…` or `package:<path>` in the carrier's folder) is missing;
  - **OpenSSL** (`version: "from-build"`): on a single `x86_64-pc-windows-msvc` check, the version is `OPENSSL_VERSION_STR` from `$OPENSSL_DIR/include/openssl/opensslv.h`; `1.1.1w` fails with "the Windows build links OpenSSL 1.1.1w; HoploDex needs OpenSSL 3 …"; `3.5.4` passes and becomes the component's version; a Windows check with no `OPENSSL_DIR` fails naming `OPENSSL_DIR`, since the Windows build needs it anyway (DEVELOPMENT.md, Windows); under `--all-targets` the version isn't read, and the component reads "3.x (the version the Windows build links)" (contracts/notices.md);
  - **exceptions** (`[{ component, checkedVersion, declared, texts[], from, reason }]`, `component` `crate:<name>` or `npm:<name>`): fail when the declared expression changes, when the component now ships a usable license file of its own ("remove the exception"), or a text file is missing; pass on a version bump alone (research.md §8); warn when this target doesn't ship the component; fail under `--all-targets` when no target does;
  - **clarifications** (`[{ component, version, files: [{ path, sha256 } | { text }], reason }]`): fail when a listed file's SHA-256 differs or a listed file is missing; otherwise replace what the matcher finds;
  - **native code** (`[{ crate, what, note }]`, `what` one of `hand-recorded:<entry name>`, `own-code`, `import-libraries`, `system-library`): a shipped crate with an enabled `cc` or `cmake` build dependency (`cargo tree -e build` for the target) or carrying a `*.lib`, `*.a`, `*.dll`, `*.dylib`, `*.so` or `*.o` file, and not listed, fails naming the crate and triple; a listed crate no target ships warns, and fails under `--all-targets`;
  - committed texts' first line `Source: <url>` is stripped from the text and required
- [X] T014 [P] [US2] Write `scripts/licenses/notices.test.ts` (`010 US2-1`, `010 US2-2`, `010 US2-5`, `010 FR-008`, `010 FR-011`, `010 SC-001`, `010 SC-002`; contracts/notices.md):
  - **with fixtures**: the components are exactly the npm shipped set plus the crates in the triple's recorded `cargo tree` (less `hoplodex`) plus the hand-recorded entries for that OS; no `dev` or `devOptional` package; a non-`hoplodex` path package fails; a crate shipped only on one triple appears only in that triple's document; adding a fixture crate to the tree adds its notice and removing it removes it, with no other change to the document;
  - **`notices.json`**: `format: 1`; keys in contracts/notices.md's order; two-space indent and a final newline; components ordered by `name` in lower case, then `version`; each text stored once in `texts` and referenced by id; license text first, then NOTICE and attribution texts in the record's order; `declared` present only for a choice; `source` one of `package`, `exception`, `clarified`, `hand-recorded`; `carrier` only on hand-recorded components; `kind` `font` for `@fontsource*`; two runs over the same inputs are byte-identical;
  - **the program block**: `name` "HoploDex"; `version` from `package.json`; `copyrightHolder` "exodious and the HoploDex contributors"; `sourceUrl` from `repository.url` without `git+` and `.git` (`https://github.com/exodious/HoploDex`); `copyrightYears` "2026" for a 2026 commit date, "2026–2028" for 2028, and the build's own year when there's no git (research.md §13);
  - **`NOTICES.txt`**: contracts/notices.md's layout line for line (the header, the GPL statement, the source address, the additional permission, "This copy was built for <triple>.", the third-party section with each component's heading line and "used under <license> of <declared>" for a choice, every text repeated per component, the full `LICENSE` last), UTF-8 with LF line ends;
  - **the CLI**: `--check` writes nothing and exits 1 on any failure; every failure is printed, one per line, before exiting; `--out <dir>` writes both files; `--target` overrides `TAURI_ENV_TARGET_TRIPLE`, which overrides the host's triple; `--all-targets` checks the three triples of contracts/notices.md and prints "notices: x86_64-unknown-linux-gnu N, x86_64-pc-windows-msvc N, aarch64-apple-darwin N components; 0 failures";
  - **against the real trees, for the host's target** (`targetTriple()`, so each OS's test run checks its own notices; skipped with a message where `cargo` isn't on the PATH):
    - "the host's notices list exactly the audit's shipped set": the npm components equal `npmShipped(package-lock.json)`, the crates equal `cargo tree` for the host's triple with `custom-protocol`, and the hand-recorded entries are those for the host's OS (`010 SC-001`);
    - `@zxcvbn-ts/language-en`'s `NOTICE.md` appears word for word in the OpenSubtitles entry's texts (`010 US1-5`, `010 FR-016`);
    - the real `NOTICES.txt` begins with "HoploDex", the `package.json` version and "Copyright (C) 2026 exodious and the HoploDex contributors" (or the year range); `licenses/additional-permission.txt`'s text follows the source address; research.md §2's EFF CC BY 4.0 attribution is there word for word; and it ends with `LICENSE`'s text (`010 US2-5`, `010 FR-011`, `010 FR-019`)
- [X] T015 [P] [US2] Write `scripts/licenses/vitePlugin.test.ts`, running Vite's `build()` on a tiny fixture app (one entry, one module worker, packages from T011's `node_modules`) with `hoplodexNotices()` given T011's inputs (`010 FR-008`, `010 FR-011`, `010 FR-012`; contracts/notices.md "The Vite plugin"):
  - the build emits `notices.json` and `NOTICES.txt` at the bundle's root, equal to `notices.mjs --out`'s for the same inputs;
  - a module bundled from a package outside the npm shipped set (marked `dev` in the fixture lockfile), in the main bundle and, separately, only in the worker's, fails the build with the "bundle: package qux is bundled into the app but isn't in the audit's shipped set …" message;
  - any generator failure fails the build with the whole failure list;
  - a withdrawn data file's marker (policy.mjs's withdrawn list, T016) in any emitted chunk fails the build naming the file;
  - `__HOPLODEX_PROGRAM__` is defined with the program block in build and serve;
  - the development server serves `GET /notices.json` with `development: true`, reports a generator failure as a warning and still serves what it found, and recomputes after the lockfile changes

### Implementation for User Story 2

- [X] T016 [US2] In `scripts/licenses/policy.mjs`, add the entry-scoped exceptions (`blessing` for SQLite, `CC-BY-4.0` for the EFF list, `ODC-By-1.0` for the OpenSubtitles data, `LicenseRef-public-domain` for the Moby, Census and Burnett data; research.md §7) and a withdrawn-data list `[{ file: "@zxcvbn-ts/language-en/dist/wikipedia.json", marker: <T008's string>, reason: "CC BY-SA 3.0 / GFDL, 010 research.md §2" }]`. Then implement `crateShipped(triple, { cargo, root })` in `scripts/licenses/shipped.mjs` (research.md §5): run `cargo fetch --locked --target <triple>` first, then `cargo tree --locked --manifest-path src-tauri/Cargo.toml -e normal,no-proc-macro --target <triple> --features custom-protocol --prefix none -f '{p}'`, deduplicate, drop `hoplodex`, fail on any other path package; find each crate's folder from `cargo metadata --locked --format-version 1`'s `manifest_path` (never its resolve, which over-counts); read enabled `cc`/`cmake` build dependencies from `cargo tree -e build` for the target; and `targetTriple()` (`--target`, else `TAURI_ENV_TARGET_TRIPLE`, else `rustc -vV`'s `host`). `check-npm-licenses.mjs` is unchanged by this task: it uses only `npmShipped`
- [X] T017 [US2] Implement `scripts/licenses/licenseFiles.mjs` (research.md §5) to T012's rules: `findTexts(folder, chosenLicense, { declared })` returning the ordered texts or a failure naming the component; no generic fallback. T012 must pass
- [X] T018 [US2] Implement `scripts/licenses/records.mjs` (research.md §7, §8; data-model.md): readers for the four record files with the sort and unknown-field checks; `checkHandRecorded`, `checkExceptions`, `checkClarifications` and `checkNativeCode` for one target, and their cross-target forms for `--all-targets`; the OpenSSL version reader over `opensslv.h`; committed texts read with their `Source:` line stripped. Messages exactly as contracts/notices.md's. T013 must pass
- [X] T019 [US2] Implement `scripts/licenses/notices.mjs` (research.md §5, §6, §10, §13; contracts/notices.md): `programBlock()`; `buildNotices({ target, … })` assembling the components (npm from `npmShipped` with texts from `node_modules`, crates from `crateShipped` with texts from their folders, then exceptions, clarifications and hand-recorded entries applied), resolving choices with `spdx.choose()` and checking each against `policy.mjs` ("license: crate quux 2.0.0 is under GPL-2.0-only, which isn't allowed."), collecting every failure; `renderJson()` and `renderText()` to contracts/notices.md's formats; and the CLI (`--check`, `--target`, `--out`, `--all-targets`). T014's fixture tests must pass (the real-tree ones wait for T021)
- [X] T020 [US2] Implement `scripts/licenses/vitePlugin.mjs`'s `hoplodexNotices(options)` (contracts/notices.md "The Vite plugin"): one collector shared by a main-build plugin and a worker-build plugin (returned together, e.g. `{ main, worker }`); `config` adds `define: { __HOPLODEX_PROGRAM__ }` without needing `cargo`, so Vitest gets it too; during the build, the `node_modules` package of every module id, in both builds; in the main build's `generateBundle`, the bundled-package check, the withdrawn-data check over every emitted chunk (worker chunks included), the generator for `targetTriple()`, and `this.emitFile` of `notices.json` and `NOTICES.txt`, or `this.error` with the failure list; under `vite` serve, `GET /notices.json` computed on first request, cached until `package-lock.json` changes, failures as warnings, `development: true`. Wire it into `vite.config.ts` (`plugins` and `worker.plugins`), and declare `__HOPLODEX_PROGRAM__` (`{ name, version, copyrightYears, copyrightHolder, sourceUrl }`) in `src/vite-env.d.ts`. T015 must pass
- [X] T021 [US2] Write the real records until `node scripts/licenses/notices.mjs --check --all-targets` passes on Linux, rerunning the check after each file:
  - `licenses/hand-recorded.json`: research.md §7's nine entries (SQLCipher 4.14.0 community on `libsqlite3-sys` 0.38.1 with `bundled-sqlcipher`, text `package:sqlcipher/LICENSE`; SQLite 3.51.3, `licenses/texts/sqlite/blessing.txt` from the blessing at `sqlcipher/sqlite3.c:64`; OpenSSL `from-build`, Windows, OpenSSL 3's `LICENSE.txt` at the version `scripts/windows/setup-system.ps1` installs; the WebView2 loader, Windows, T009's text; the OpenSubtitles data, `package:NOTICE.md` and `package:THIRD_PARTY_LICENSES.md`; Moby and random-name, with random-name's `LICENSE` from its repository; the 1990 Census surnames; Mark Burnett's passwords; the EFF Large Wordlist, with research.md §2's CC BY 4.0 attribution word for word), each with its `reason`;
  - `licenses/exceptions.json`: research.md §8's 19 crates and 2 npm packages, texts committed under `licenses/texts/<component>/` from the source repository at the checked version's tag (one shared text for each family that shares a repository and license file, the `objc2` and `unic` crates), each `from` the file's URL at that tag;
  - `licenses/clarify.json`: Big Shoulders Display with T010's copyright string and the upstream `OFL.txt` (research.md §12), the Atkinson families too if T010 found a mismatch, and any crate the matcher can't place;
  - `licenses/native-code.json`: research.md §7's initial list (`libsqlite3-sys` → `hand-recorded:SQLCipher`, `webview2-com-sys` → `hand-recorded:Microsoft WebView2 loader`, `objc2-exception-helper` → `own-code`, the `windows_*_msvc`/`windows_*_gnu` crates → `import-libraries`, and each `-sys` crate that builds C or carries files → `system-library`).

  Where the real trees differ from research.md §1, §7 or §8 (a count, an entry), correct the research in place: it is this feature's own artifact. Then T014's real-tree tests must pass
- [X] T022 [US2] Add `&& node scripts/licenses/notices.mjs --check --all-targets` to `audit:licenses` in `package.json`, and update the header comment of `.github/workflows/audit.yml` to say the audit now checks the notices for all three OSes (its `npm ci --ignore-scripts` and the Rust toolchain are all it needs; `cargo fetch` reaches the registry as `cargo deny` does)
- [X] T023 [US2] Add a "License notices" section to `DEVELOPMENT.md`, after "License audit" (FR-018): what `npm run build` writes and where (`dist/notices.json`, `dist/NOTICES.txt`, for the build's target); the CLI's four forms; what "shipped" means (the one definition in `shipped.mjs`); and how to answer each failure message: add a hand-recorded entry, an exception (with its text committed and its `Source:` line), a clarification or a native-code entry, with one example of each from today's records; that a version bump fails a hand-recorded entry but not an exception, and why (research.md §8). Mention the files in "Build"
- [ ] T024 [US2] On Linux, run `scripts/dev-container.sh bash -c 'npm test && npm run lint && npm run format:check && npm run audit:licenses && npm run build'`; all must pass, `dist/notices.json` and `dist/NOTICES.txt` must exist, and a second build must leave both byte-identical. Time `npm run build` on this branch and on `main` (three runs each) and keep the figures for the PR's performance note (plan.md Technical Context)
- [ ] T025 [P] [US2] On Windows, on the Windows test machine, with `OPENSSL_DIR` set as `scripts/windows/setup-system.ps1` sets it: `npm run build` passes and `dist/notices.json`'s OpenSSL component carries the 3.x version from `opensslv.h`, and the WebView2 loader is listed; `node scripts/licenses/notices.mjs --check` passes; then point `OPENSSL_DIR` at a scratch copy of the headers whose `opensslv.h` says `1.1.1w` and confirm `--check` fails with the OpenSSL message (never edit the real vcpkg tree); and run `npx vitest run scripts/licenses` with the real `OPENSSL_DIR`, whose real-tree tests now check the Windows target
- [X] T026 [P] [US2] On macOS, in the macOS 26 VM, run `npm run build` (target `aarch64-apple-darwin`) and `node scripts/licenses/notices.mjs --check`; both must pass, with the `objc2` family's exceptions used; and run `npx vitest run scripts/licenses`, whose real-tree tests now check the macOS target

**Checkpoint**: every build carries notices that match it; the audit checks all three OSes.

---

## Phase 4: User Story 1 - Anyone Running HoploDex Can See Its License and What It Is Built From (Priority: P1) 🎯 MVP

**Goal**: one About HoploDex dialog, from the chooser's top bar, the database menu and the macOS application menu, showing the program block, the full GPL-3.0, and every component with its notice texts, with no command, database or network.

**Independent Test**: in a build, open About from the chooser and from the database menu; the statements, the source address, the license text and a sample of components (a frontend package, a crate, SQLCipher, a font, the dictionary's attribution) are there; the database file is unchanged and no command was sent.

### Tests for User Story 1 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T027 [P] [US1] Write `src/features/app/AboutDialog.test.tsx` against fixture documents (a small `notices.json` with a choice, a NOTICE text, an attribution and a hand-recorded component), with `fetch` stubbed, citing each scenario:
  - `010 US1-1`, `010 FR-002`: name, "Version 0.1.0" (from `__HOPLODEX_PROGRAM__`), "Copyright © 2026 exodious and the HoploDex contributors", the free-software and no-warranty sentences exactly as contracts/ui.md §2 words them, shown before the fetch resolves; the additional permission after it, its heading line as a `strong` run-in (`010 FR-019`);
  - `010 US1-6`, `010 FR-006`: the source address is text in a `user-select: all` monospace span, not a link (no `<a>`, nothing opened);
  - `010 US1-3`, `010 FR-003`: the license disclosure, titled "GNU General Public License, version 3", opens to the document's `gpl` in a `pre` that is a region named "GNU General Public License text" with `tabIndex` 0;
  - `010 US1-4`, `010 FR-005`: the count "N components"; typing narrows by case-insensitive substring of the name and the count reads "n of N components" in a polite live region; a row's summary is "version · license", plus ", used under MIT" for a choice; opening a row shows its texts in order in regions named "<component> license text"; a hand-recorded row adds "Built into libsqlite3-sys 0.38.1"; a filter with no match shows "No component matches “zzzz”." in place of the list;
  - `010 US1-5`: the NOTICE and the attribution are shown word for word;
  - keyboard (contracts/ui.md §3): focus opens on the license disclosure's button; Escape in a filter with text clears it, and a second Escape closes; Enter in the filter does nothing (the dialog isn't a `<form>`);
  - loading and failure (contracts/ui.md §2 items 4, 5): "Loading the license notices…" only after `SHOW_AFTER_MS` (fake timers; nothing at 900 ms); a rejected fetch, a non-OK response and `format: 2` each show "The license notices couldn't be loaded. They are also in the NOTICES.txt file that came with HoploDex." with the program block still shown;
  - `010 SC-005`: rendering, filtering and opening rows never call `invoke` (assert the `@tauri-apps/api/core` mock untouched)
- [X] T028 [P] [US1] Amend `src/features/databases/DatabaseChooser.test.tsx` (the top bar has an "About HoploDex" ghost button before the theme toggle, which calls `showAbout`; `010 US1-1`, `010 FR-001`) and `src/features/databases/DatabaseMenu.test.tsx` (an "About HoploDex" item directly after "About databases and security", which calls `showAbout`; `010 US1-2`)
- [X] T029 [P] [US1] Amend `src/features/session/SessionProvider.test.tsx` (`010 FR-001`; spec Edge Cases; contracts/ui.md §1, §4): `showAbout()` opens the one dialog in the chooser phase and in the open phase; a second call leaves it as it is; the `onAboutRequested` handler (mock `sessionService.onAboutRequested` beside `onQuitRequested`) is subscribed from the starting phase and opens it in each phase; About closes, without asking, on an open, a lock (`session:closing` for the open `sessionId`), a close, a database switch and the closing screen; closing returns focus to the control that opened it
- [X] T030 [P] [US1] Write the Rust unit test in `src-tauri/src/app_menu.rs`'s `#[cfg(test)]` module, compiled on every OS (the constants and the id-to-event mapping aren't macOS-only; only building the menu is): `ABOUT_MENU_ID` is `"about"`; `ABOUT_REQUESTED` is `"app:about-requested"`; the id maps to that event and any other id to none; and `src/features/session/sessionService.ts` contains the same event name (read it with `include_str!`), so the two sides can't drift (`010 US1-7`; contracts/tauri-commands.md)
- [ ] T031 [US1] Write the E2E journey in `e2e/specs/databases.e2e.ts` as `describe("010 US1 - Anyone Running HoploDex Can See Its License and What It Is Built From")`, following plan.md's Test Levels row (depends on T020's build output in `dist/`). The file's sandbox starts with no database, so the journey makes its own with `createDatabase()` (`e2e/support/ui.ts`); it relies on no other test's database:
  - from the chooser: **About HoploDex** (1 action, `010 SC-003`), the program block's statements, "Version" with `package.json`'s version, the source address selected equals `https://github.com/exodious/HoploDex`; the count above the list equals the number of components in the repository's `dist/notices.json`, which the E2E build embedded (`010 SC-001`); the license disclosure (2 actions) whose text starts with "GNU GENERAL PUBLIC LICENSE"; filter "sqlcipher", open SQLCipher, its "Zetetic LLC" copyright; filter "zzzz", the no-match line; clear the filter;
  - per OS, the components only that OS ships (`010 SC-001`, `010 FR-009`): on Linux "webkit2gtk" finds `webkit2gtk` and "objc2" finds none; on macOS "objc2" finds the `objc2` crates and "webkit2gtk" finds none; on Windows "openssl" finds OpenSSL at a 3.x version under Apache-2.0, "webview2" finds "Microsoft WebView2 loader" under BSD-3-Clause, and "webkit2gtk" finds none;
  - Escape twice, focus back on the button;
  - then `createDatabase({ name: "About" })`; take the SHA-256 (`sandbox.sha256Of`) of its file and of the sandbox's `machine.json`; **About HoploDex** from the database menu, Escape; both SHA-256s are unchanged (`010 US1-2`, `010 SC-005`; taken just around About, since opening and locking a database write both files themselves); then open About again from the menu and Ctrl/⌘+L returns to the chooser with no dialog;
  - timed (`010 SC-004`): the list shows within 1 s of the click, a filter's result within 500 ms of typing, and, with the filter empty and every row shown, scrolling `.hd-dialog__body` from top to bottom one viewport height per `requestAnimationFrame` records no gap between frames over 100 ms (measured in the page with `performance.now()`);
  - macOS only: `chooseMenuBarItem(appPid(), "About HoploDex")` opens it from the chooser and again with the database open, and the app has only its own window (no system About panel) (`010 US1-7`)

### Implementation for User Story 1

- [X] T032 [P] [US1] Implement `src/features/app/aboutNotices.ts` (data-model.md "Notices document", "The About dialog's states"): the document's types; `loadNotices()` fetching `notices.json` relative to the page's own origin (no IPC, no other origin) and refusing a non-OK response, a `format` other than `1` or a malformed shape; `filterComponents(components, query)` (case-insensitive substring of `name`)
- [X] T033 [P] [US1] Write `src/features/app/about.css`: the program block and `.hd-about__text` (monospace, wrapping, `max-height: 60vh`, its own scroll), from `src/styles/tokens.css`'s tokens at WCAG 2.1 AA in both themes, and one column at the `max-width: 900px` breakpoint (FR-007); the source address reuses export's `.hd-paths dd` treatment (`user-select: all`)
- [X] T034 [US1] Implement `src/features/app/AboutDialog.tsx` to contracts/ui.md §2 and §3, from the shared `Dialog` (`size="lg"`, title "About HoploDex", no footer), `Disclosure` (`headingLevel` 3 for the license, 4 per component), `TextField` (`type="search"`, label "Find a component") and the closing screen's `SHOW_AFTER_MS`; texts render only when their row opens; filtering on every keystroke, no debounce. T027 must pass
- [X] T035 [US1] Add `onAboutRequested(handler)` to `src/features/session/sessionService.ts`, wrapping `listen("app:about-requested", …)` beside `onQuitRequested` (contracts/tauri-commands.md)
- [X] T036 [US1] In `src/features/session/SessionProvider.tsx`: the About state, `showAbout()` in the session context (a second call does nothing while open), `AboutDialog` rendered once above both the chooser and the open database's tree, closed on every phase change, and `onAboutRequested` subscribed for the provider's whole life, the starting phase included (research.md §11). T029 must pass
- [X] T037 [P] [US1] Add the "About HoploDex" button (`Button variant="ghost"`) to `src/features/databases/DatabaseChooser.tsx`'s top bar, before the theme toggle, calling `showAbout()`
- [X] T038 [P] [US1] Add the "About HoploDex" `MenuItem` to `src/features/databases/DatabaseMenu.tsx`, after "About databases and security", calling `showAbout()`. T028 must pass
- [X] T039 [US1] Implement the macOS application menu in `src-tauri/src/app_menu.rs` and `src-tauri/src/main.rs` (research.md §11; contracts/tauri-commands.md): `ABOUT_MENU_ID`, `ABOUT_REQUESTED` and the id-to-event mapping (all OSes); on macOS only, `setup()` sets `Menu::default(app)` with the application submenu's predefined About item replaced, at the same position, by `MenuItem::with_id(app, ABOUT_MENU_ID, "About HoploDex", true, None)`, every other item untouched; `on_menu_event` emits `ABOUT_REQUESTED` with `()` for that id, logging a failed emit as `request_quit` does. Linux and Windows set no menu, as today. No command, ACL or capability changes (`core:default` grants `listen`). T030 must pass
- [ ] T040 [US1] Add contracts/ui.md §5's shots to `e2e/screenshots/screens.e2e.ts`, numbered from the walk's next free number, each in both themes: about-chooser, about-license, about-component (filter "sqlcipher", its row opened), about-no-match (filter "zzzz"), about-narrow (the narrow width, from the database menu)
- [X] T041 [US1] In `specs/003-database-protection-management/spec.md`, beside FR-030, add `_Amended by [spec 010](../010-license-notices/spec.md): About HoploDex (010 FR-001) is a separate screen; "About databases and security" stays the guide._`, and add `AboutDialog` to CLAUDE.md's list of forms and dialogs that share patterns ("UI consistency is a constitution principle")
- [ ] T042 [US1] On Linux, run `scripts/dev-container.sh bash -c 'npm test && npm run lint && npm run build && npm run test:e2e -- --spec e2e/specs/databases.e2e.ts && npm run screenshots'`; all must pass. Look at every new shot in both themes for layout, contrast and the narrow width, and keep them for the PR (with "before" shots of the chooser's top bar and the database menu from `main`)
- [ ] T043 [P] [US1] On macOS, in the macOS 26 VM, run `cargo test --manifest-path src-tauri/Cargo.toml --features test-kdf app_menu` and `cargo clippy --all-targets -- -D warnings` (the menu code is compiled only here), then `npm run build && npm run test:e2e -- --spec e2e/specs/databases.e2e.ts`, which chooses the menu bar's **About HoploDex**; all must pass
- [ ] T044 [P] [US1] On Windows, on the Windows test machine, run `npm run build && npm run test:e2e -- --spec e2e/specs/databases.e2e.ts`, with its Windows-only OpenSSL and WebView2 loader steps; it must pass

**Checkpoint**: About HoploDex works from all three ways in, on every OS, with notices that match the build.

---

## Phase 5: User Story 3 - The License Audit Is Recorded and Its Open Questions Are Closed (Priority: P3)

**Goal**: the audit record is current and dated, the word lists are settled (done in T006–T007 and T021), the §7 permission is stated where the license notice is, the published Linux bundles carry no LGPL libraries, and DEVELOPMENT.md lists only the manual checks that remain.

**Independent Test**: research.md §1–§4 cover every part of FR-013 with dated results; README's "License" section matches `licenses/additional-permission.txt`; DEVELOPMENT.md and `build-appimage.sh` state FR-017; the strength hint still rates passphrases.

### Tests for User Story 3 (mandatory per constitution)

- [X] T045 [P] [US3] Add to `scripts/licenses/policy.test.ts` (`010 FR-019`; research.md §15): `README.md` has a "License" section, and it contains `licenses/additional-permission.txt`'s text verbatim (compared after joining wrapped lines, if the README wraps it), the copyright line "Copyright (C) 2026 exodious and the HoploDex contributors" and the GPL-3.0-only notice; it fails until T046

### Implementation for User Story 3

- [X] T046 [US3] Add a "License" section to `README.md` (research.md §15): the copyright notice, the GPL-3.0-only notice with the no-warranty statement and the pointer to `LICENSE`, the additional permission verbatim (pointing to `licenses/additional-permission.txt` as the copy the notices read), that contributions are taken under the license with this permission, and that the notices for third-party components are in each build's `NOTICES.txt` and About HoploDex. `LICENSE` stays unmodified and the SPDX identifier stays `GPL-3.0-only`. T045 must pass
- [X] T047 [P] [US3] Add to `scripts/build-appimage.sh`'s header comment that AppImages are built, not published with a release, because they carry LGPL libraries (WebKitGTK, GTK and their stack), and that publishing one first needs a decision on how it offers their source (`010 FR-017`; research.md §4)
- [X] T048 [US3] Rewrite `DEVELOPMENT.md`'s "License audit" section (FR-018; after T023, same file): the audit now runs `notices.mjs --check --all-targets` too; the strength hint's dictionary paragraph says which lists ship and on which ground, that `wikipedia.json` doesn't (research.md §2), and that the attributions are hand-recorded entries; remove the closed manual checks (the word lists; "C code built inside a crate", now the native-code guard; the OpenSSL version, now a build check), and keep or add only what remains: a new MPL-2.0 crate's Exhibit B, a new asset's or data list's source and license, rechecking exceptions whose recorded version is behind before a release, and the system-libraries rule restated as FR-017's (published Linux bundles are the Flatpak, `.deb` and `.rpm`; no AppImage is published); replace the closing "cargo about can generate one" paragraph with a pointer to "License notices"; and point to research.md §1 as the audit's record
- [ ] T049 [US3] Rerun research.md §1's checks with this branch's release build figures and rewrite the table's counts and date (the implementation's last audit task, research.md's preamble): the npm shipped and bundled counts after `devOptional` (T004), the shipped crate counts per OS from `--all-targets` (T021), the MPL-2.0 Exhibit B grep over every MPL-2.0 crate, the native-code list, the fonts (T010); confirm §2's package versions and their sha512 still match `package-lock.json`; and add the WebView2 SDK version (T009) to §3
- [X] T050 [US3] Amend 003 with one-line italic notes (CLAUDE.md "Spec Kit workflow"), leaving the old text as it is: beside the dictionary exception in `specs/003-database-protection-management/spec.md`'s clarification of 2026-09-26 and in `research.md` §18, `_Amended by [spec 010](../010-license-notices/research.md): the other word lists are settled and wikipedia.json no longer ships (010 research.md §2); the attribution is shown in About HoploDex and NOTICES.txt (010 FR-009)._`
- [ ] T051 [US3] On Linux, run `scripts/dev-container.sh bash -c 'npx vitest run scripts/licenses src/components/strength && npm run audit:licenses && npm run format:check'`; all must pass

**Checkpoint**: the audit's record is complete and dated; no manual check #20 names is left open (`010 SC-006`).

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: The architecture notes, the gates on every OS, the one manual check, and the pull request with its close-out.

### Docs and checks

- [ ] T052 Update `CLAUDE.md`'s architecture: `scripts/licenses/` (the one definition of shipped, the policy shared with the audit, the generator and the Vite plugin, fail closed) and `licenses/` (the four record files and committed texts, the §7 permission's one copy); `app:about-requested` and the macOS application menu in `main.rs`/`app_menu.rs`; `AboutDialog` in `features/app/`, rendered by `SessionProvider`, reading `notices.json` with no command; and that `npm run build` writes `dist/notices.json` and `dist/NOTICES.txt` (after T041, same file)
- [ ] T053 Check quickstart.md end to end: every row of "Where each scenario and criterion is tested" names a test that exists under that name; update the map, and contracts/notices.md, contracts/ui.md and data-model.md, where the implementation changed a name, a message or a shape (they are this feature's own artifacts)

### Gates on every OS

- [ ] T054 On Linux, run the whole suite in the dev container: `cd src-tauri && cargo test --features test-kdf`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `npm test` (citations included), `npm run lint`, `npm run format:check`, `npm run audit`, `npm run build`, every E2E spec file, the screenshot walk, and `npm run bundle:linux -- --bundles appimage`, which must still build an AppImage with the notices step in the build (`010 FR-017` keeps the ability; it isn't published); fix every failure and record the result
- [ ] T055 [P] On macOS, in the macOS 26 VM, run the build, `cargo test --features test-kdf`, `npm test`, lint, the audits, every E2E spec file and the screenshot walk; fix every failure and record the result
- [ ] T056 [P] On Windows, on the Windows test machine, run the build, `cargo test --features test-kdf`, `npm test`, lint, the audits, every E2E spec file and the screenshot walk; fix every failure and record the result
- [ ] T057 Run `/security-review` and save its result under `.git` (CLAUDE.md memory "Security review before PR"); draft the PR notes: before/after screenshots (T042); security and data handling from research.md §16 (no command, capability or database access; one data-less event; no network; the build's one fetch is `cargo fetch --locked`); the performance note (SC-004's timings and longest scroll frame from T031, the build's added time from T024); the audit's output change (T004); and the owner's decisions recorded at planning (plan.md "Design choices worth a look")

### Manual check (best effort before a release, not a merge gate)

Everything else is automated: a build's notices matching its OS in T014 (run on each OS) and T031, and `NOTICES.txt`'s content in T014. Only the screen reader needs a person. That check is deferred to #48 (below).

- [X] T058 [P] On Linux, do quickstart.md's manual check **M1** with Orca (on "Main collection", passphrase `human testing passphrase`), and record the result for the PR
  - **Deferred** (2026-10-18) to #48, with 004's to 009's screen reader checks, and listed in #37's pre-release checks: not run before the PR. The dialog's name, the focus on open and on close, the text regions' names and the polite count are covered by `AboutDialog.test.tsx` (T027) and `SessionProvider.test.tsx` (T029)
- [X] T059 [P] On macOS, do quickstart.md's manual check **M1** with VoiceOver (on "Main collection", passphrase `human testing passphrase`), and record the result for the PR
  - **Deferred** (2026-10-18) to #48, with 004's to 009's screen reader checks, and listed in #37's pre-release checks: not run before the PR. The dialog's name, the focus on open and on close, the text regions' names and the polite count are covered by `AboutDialog.test.tsx` (T027) and `SessionProvider.test.tsx` (T029)

### Close-out (at pull request time)

Done when the pull request is opened, not before (CLAUDE.md; plan.md "Close-out (at pull request time)"). Check each issue's state first (`gh issue view <n> --json state`) and don't comment on a closed one unless the owner asks.

- [ ] T060 Write the pull request body with "Fixes #18, fixes #20" (one keyword per issue), `/security-review`'s result (T057), research.md §16's data-handling notes, the performance note and the before/after screenshots (the repository's hook requires the review); after opening it, check `closingIssuesReferences` names both issues
- [ ] T061 [P] On issue **#18**, comment item by item against its checklist: the Rust notices by our own script, not `cargo about`, and why (research.md §5); the npm notices from the audit's shipped set, checked against the bundle; SQLCipher, OpenSSL 3, and also the WebView2 loader and the word lists' attributions as hand-recorded entries with carrier checks; one generated `notices.json` fetched by the app and produced by every `npm run build`; `NOTICES.txt` ready for #19; the About screen and its shots in the walk
- [ ] T062 [P] On issue **#20**, comment: the audit record is 010 research.md §1; the word lists' verdicts (§2), with `wikipedia.json` dropped; the WebView2 loader's two-way settlement and the §7 permission (§3, §15); the LGPL decision (§4, FR-017); the native-code guard and the OpenSSL build check, which replace two manual checks
- [ ] T063 [P] On issue **#19**, comment what the written release process must take from this feature: each bundle carries `dist/NOTICES.txt` beside the program; published Linux bundles are the Flatpak, `.deb` and `.rpm`, never an AppImage without a decision on its LGPL source; the Windows build must have `OPENSSL_DIR` at OpenSSL 3, now enforced by the build; the release review's manual license checks are those in DEVELOPMENT.md, including rechecking exceptions whose recorded version is behind
- [ ] T064 [P] On issue **#42**, comment that the Flatpak's LGPL libraries come from its Flathub runtime, which Flathub distributes with source (010 FR-017), and that the bundle should carry `NOTICES.txt` too

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none.
- **Foundational (Phase 2)**: depends on Setup; BLOCKS every story. T003 (red) → T004 → T005; T006 (red) → T007 → T008; T009 and T010 in parallel with both. If T009 finds the WebView2 SDK isn't BSD-3-Clause, everything waits on the owner.
- **US2 (Phase 3)**: depends on Foundational. T011 → T012–T015 (red) → T016 → T017, T018 → T019 → T020 → T021 → T022, T023 → T024 → T025, T026.
- **US1 (Phase 4)**: its tests (T027–T030) and components (T032–T038) need only Setup and the format contract, so they can run alongside US2. T039 needs T030. T031 (E2E), T040 (shots) and T042–T044 need US2's T020 and T021, since the E2E build embeds a real `dist/notices.json`. The MVP is US2 + US1 together: About's list is US2's output.
- **US3 (Phase 5)**: T045 → T046; T047 any time after Setup; T048 after T023 (same file); T049 after T021 and T025–T026 (the real counts); T050 after T007.
- **Polish (Phase 6)**: depends on every story; the close-out waits for the pull request.

### Same-file order across stories

- `scripts/licenses/policy.mjs`: T004 → T016.
- `scripts/licenses/policy.test.ts`: T003 → T045.
- `scripts/licenses/shipped.mjs`: T004 → T016.
- `scripts/check-npm-licenses.mjs`: T004 only.
- `src/vite-env.d.ts`: T007 → T020.
- `vite.config.ts`: T020 only.
- `package.json`: T022 only.
- `licenses/*.json`, `licenses/texts/`: T001 → T009 (WebView2 text) → T021.
- `DEVELOPMENT.md`: T023 (US2) → T048 (US3).
- `CLAUDE.md`: T041 (US1) → T052.
- `specs/010-license-notices/research.md`: T009, T010 → T021 → T049.
- `src-tauri/src/main.rs`: T002 → T039.
- `src-tauri/src/app_menu.rs`: T002 → T030 → T039.
- `src/features/session/SessionProvider.tsx`: T036 only; `SessionProvider.test.tsx`: T029 only.
- `e2e/specs/databases.e2e.ts`: T031 only.

### Within Each User Story

- Tests are written first and must fail before the implementation.
- Fixtures before the tests that use them; the policy before the generator; the generator before the plugin; the plugin before the real records are checked through a build.
- Interface: data loading and styles before the dialog; the dialog and `showAbout()` before the entry points; the menu's constants (and their test) before the menu.
- Each story ends with a Linux run of what it touches; the other OS run at the story's end where only they can show it (OpenSSL, the macOS menu) and again in Polish.

### Parallel Opportunities

- T002 alongside T001; T009 and T010 alongside T003–T008.
- US2: T012–T015 together once T011 is done; T017 and T018 together; T025 and T026 together.
- US1: T027–T030 together; T032, T033, T037 and T038 together; T043 and T044 together. US1's tests and components alongside US2's implementation.
- US3: T045 and T047 together, and alongside US1.
- Per-OS runs (macOS, Windows) can go to their own sessions in parallel with Linux work (CLAUDE.md memory "Per-OS sessions").

---

## Parallel Example: User Story 2

```bash
# After the fixtures (T011), the red tests together:
Task: "T012 scripts/licenses/licenseFiles.test.ts"
Task: "T013 scripts/licenses/records.test.ts"
Task: "T014 scripts/licenses/notices.test.ts"
Task: "T015 scripts/licenses/vitePlugin.test.ts"

# Then the two independent modules:
Task: "T017 scripts/licenses/licenseFiles.mjs"
Task: "T018 scripts/licenses/records.mjs"
```

## Parallel Example: User Story 1

```bash
# Tests, together (against fixture documents; no build needed):
Task: "T027 src/features/app/AboutDialog.test.tsx"
Task: "T028 DatabaseChooser.test.tsx and DatabaseMenu.test.tsx"
Task: "T029 src/features/session/SessionProvider.test.tsx"
Task: "T030 src-tauri/src/app_menu.rs unit test"

# Building blocks, together:
Task: "T032 src/features/app/aboutNotices.ts"
Task: "T033 src/features/app/about.css"
Task: "T037 DatabaseChooser.tsx top bar button"
Task: "T038 DatabaseMenu.tsx menu item"
```

## Parallel Example: User Story 3

```bash
Task: "T045 policy.test.ts README check"
Task: "T047 scripts/build-appimage.sh header"
```

---

## Implementation Strategy

### MVP First (User Stories 2 and 1)

1. Complete Phase 1: Setup.
2. Complete Phase 2: Foundational (one policy, the strength hint's lists, the WebView2 license confirmed).
3. Complete Phase 3: User Story 2, so a build writes notices that match it.
4. Complete Phase 4: User Story 1, the screen that shows them.
5. **STOP and VALIDATE**: About HoploDex opens from every way in on every OS, and its list is the build's.

### Incremental Delivery

1. Setup + Foundational → the audit and the notices share one policy; `wikipedia.json` is gone.
2. US2 → every build carries `notices.json` and `NOTICES.txt`; the audit checks all three OSes (#18).
3. US1 → About HoploDex (#18).
4. US3 → the audit's record, the §7 permission in the README, the LGPL rule, DEVELOPMENT.md (#20).
5. Polish → gates on every OS, the manual checks, the PR and its close-out.

All three ship in one pull request, before 0.1.0 is tagged: the first release can't go out without any of them.

---

## Notes

- [P] tasks = different files, no dependency on an incomplete task.
- [Story] maps a task to its user story for traceability.
- Verify each test fails before implementing it.
- Commit after each task or logical group (CLAUDE.md "Commit and push").
- The build stays offline once Cargo's sources are fetched: no license text is fetched at build time; every exception's text is committed.
- Never open the real databases; every run goes through the sandboxed tooling.
