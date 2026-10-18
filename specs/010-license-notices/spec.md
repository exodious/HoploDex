# Feature Specification: License Notices in the App and the License Audit's Last Items

**Feature Branch**: `010-license-notices`

**Created**: 2026-10-18

**Status**: Draft

**Input**: User description: "issues 18 and 20". Two GitHub issues, specified together because both stand between HoploDex and its first release under GPL-3.0-only: #18, the third-party license notices and the GPL's own notices shown in the app, so every copy carries them; and #20, the license audit's remaining work: recording its result, finding the source and license of the strength hint's unlabelled word lists, and deciding how a release bundle that carries LGPL libraries makes their source available.

## Clarifications

### Session 2026-10-18

- Q: Whose name goes in the copyright notice? → A: "exodious and the HoploDex contributors": the main author by handle, staying accurate once others contribute (FR-002). Asked alongside: US copyright policy (the Copyright Office's 2023 guidance and 2025 report; *Thaler v. Perlmutter*, D.C. Cir. 2025) protects only human authorship, so the notice covers what is protectable: the owner's specification, selection, arrangement and changes, which the specs, clarifications, issues and history record. Nothing else in this feature changes: the third-party notices are owed whatever HoploDex's own protection is, and a notice in place now holds if the law changes.
- Q: How does a release bundle that carries LGPL libraries make their source available? → A: No published bundle carries them. Linux releases are Flatpak (#42), whose runtime Flathub distributes with its source, and .deb and .rpm, which use the system's libraries. An AppImage can still be built, but isn't published with a release; publishing one later reopens this question first (FR-017).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Anyone Running HoploDex Can See Its License and What It Is Built From (Priority: P1)

A collector, or anyone they pass a copy of HoploDex to, opens "About HoploDex" from the database chooser or from an open database's menu. It names the program and its version, gives its copyright notice, says that HoploDex is free software under the GNU General Public License version 3, that it comes with no warranty, and where its source code is. The full license text is a step away. Below that is every third-party component this copy of HoploDex is built from, each with its version, its license and the copyright and license text that license requires. None of it needs a database, a passphrase or a network connection.

**Why this priority**: It is the part a release can't ship without. MIT, BSD, Apache-2.0, ISC, OFL-1.1 and ODC-BY all require their notices to travel with the program, and neither the minified interface nor the stripped program carries them; GPL-3.0 §0 and §5(d) also ask an interactive program to show its "Appropriate Legal Notices" (#18). One screen serves both.

**Independent Test**: In a build of the app, open About HoploDex from the chooser with nothing open, then again from the database menu with a database open. Confirm the version, copyright, license statement, no-warranty statement and source address are shown; the full GPL-3.0 text opens and reads in full; and a sample of components (one frontend package, one backend component, SQLCipher, a font, the strength hint's dictionary) each appear with their license text. Confirm nothing was sent over the network and no database was read.

**Acceptance Scenarios**:

1. **Given** the database chooser with nothing open, **When** the user chooses About HoploDex, **Then** it opens with the program's name, version, copyright notice, the statement that it is free software under GPL-3.0-only that may be shared and changed under that license, the statement that there is no warranty to the extent the law allows, and the address of its source code.
2. **Given** a database is open, **When** the user chooses About HoploDex from the database menu, **Then** the same screen opens, and the open database is not read, written or locked by it.
3. **Given** About HoploDex is open, **When** the user asks to read the license, **Then** the complete text of GPL-3.0 is shown in the app, without a network connection.
4. **Given** About HoploDex is open, **When** the user looks for a component by typing part of its name, **Then** the list narrows to the components whose name contains it, and choosing one shows its version, license and full notice text.
5. **Given** a component whose license asks for a NOTICE file or an attribution to be kept (an Apache-2.0 component with a NOTICE file; the strength hint's dictionary under ODC-BY), **When** the user views it, **Then** that NOTICE or attribution is shown word for word.
6. **Given** the source address, **When** the user selects it, **Then** it can be copied; the app opens nothing outside itself.

---

### User Story 2 - Every Build Carries Notices That Match What It Ships (Priority: P2)

The maintainer adds, updates or removes a dependency and builds HoploDex. The notices in that build list exactly the third-party components it ships, for the operating system it was built for, with no list kept by hand. The few components the dependency tools can't see (C code compiled inside a dependency, a crypto library linked into the program, a data file with an attribution of its own) are recorded once, with the shipped dependency that carries them, and a check flags them when that dependency changes. A shipped component whose notice can't be found stops the check rather than vanishing from the list. The same notices are also written as a plain-text file that the release bundles carry beside the program (#19).

**Why this priority**: Story 1 is only as good as its list. A list kept by hand would drift from the dependencies on the first update, and a release that silently omits one component's notice is out of compliance with that component's license.

**Independent Test**: Build the app and compare its notices with what the license audit counts as shipped: every shipped component present, no development-only tool present. Add a shipped dependency and rebuild: it appears with its notice, with no other change. Remove its license file in a fixture: the check fails and names it. Change the version of the dependency that carries SQLCipher in a fixture: the check names the hand-recorded entry to review.

**Acceptance Scenarios**:

1. **Given** a build for one operating system, **When** its notices are produced, **Then** they list every third-party component that build ships (interface packages, backend components for that operating system and the features the release enables, C code compiled into it, the crypto library linked into it where one is, fonts and data files) and no development-only or test-only component.
2. **Given** a new shipped dependency, **When** the app is built, **Then** its notice appears without any manual step; **and Given** a dependency removed, **Then** its notice is gone.
3. **Given** a shipped component whose license requires its notice but whose notice text can't be found, **When** the notices are produced, **Then** the check fails naming the component, unless an exception records where its text comes from and why.
4. **Given** a component recorded by hand because the tools can't see it, **When** the dependency that carries it is no longer shipped or its version changes, **Then** the check fails and names the entry to review.
5. **Given** a build, **When** it finishes, **Then** the same notices are also available as one plain-text file, ready for a release bundle to carry beside the program.

---

### User Story 3 - The License Audit Is Recorded and Its Open Questions Are Closed (Priority: P3)

The audit that found HoploDex's dependencies compatible with GPL-3.0 (#20) is written down where the next reviewer will look for it: what was checked, how, and what was found, including the crypto library each operating system's build links and the artwork, fonts and data files shipped with the app. The strength hint's word lists that state no source each have a recorded source and license, or no longer ship. And the release process knows which Linux bundles it publishes: none that carries LGPL libraries inside it, so there is no library source to publish beside them.

**Why this priority**: These are the manual checks the constitution requires before a release ("Licensing"; "Development Workflow & Quality Gates"). Unlike Stories 1 and 2 they change little in the app, except possibly the strength hint, but the first release can't be tagged with them open.

**Independent Test**: Read the recorded audit and confirm it covers the backend and interface dependency trees, the crypto linkage on each operating system and the OpenSSL version rule, the MPL Exhibit B check, system libraries, and the bundled assets. Confirm each of the seven word lists has a recorded source and license or is gone from the shipped app, and that the strength hint still rates a passphrase. Confirm the LGPL decision is written where the release process (#19) will take it from.

**Acceptance Scenarios**:

1. **Given** the recorded audit, **When** a reviewer reads it, **Then** it states, for each part of what ships (backend dependencies, interface dependencies, C code compiled inside a dependency, the crypto library per operating system, system libraries, artwork, icons, fonts and data files), what was checked, how, the result, and the date.
2. **Given** each of `wikipedia.json`, `firstnames.json`, `lastnames.json`, `wordSequences.json`, `passwords.json`, `diceware.json` and `adjacencyGraphs.json`, **When** the audit is complete, **Then** its source and license are recorded and it is GPLv3-compatible, or it no longer ships.
3. **Given** one or more word lists were removed, **When** the user types a passphrase, **Then** the strength hint still rates it and suggests improvements using the lists that remain.
4. **Given** a release, **When** the release process is followed, **Then** its Linux bundles are the Flatpak, .deb and .rpm, none carrying LGPL libraries inside it; an AppImage can still be built but is not published (FR-017).

---

### Edge Cases

- **A component offered under a choice of licenses** (for example "MIT OR Apache-2.0"): its notice names the license HoploDex uses it under, which is one the license audit allows, and gives that license's text.
- **Many components with the same license text**: the text may be shown once with the components that share it, but each component's own copyright lines are kept and shown with it.
- **A component with a declared license but no license file in its package**: the check fails, unless an exception records where its notice text was taken from (for example the license file in the component's source repository at that version) and why.
- **Public-domain and no-notice components** (SQLite): listed, with their dedication, even though nothing requires it, so the list is complete.
- **A component shipped on only one operating system** (OpenSSL 3, linked into the Windows build; a backend component used only on macOS): it appears in that operating system's build and not in the others.
- **The system's own libraries** (WebKitGTK and GTK on Linux, WebView2 on Windows, WKWebView and CommonCrypto on macOS, the system crypto library on Linux): not shipped by HoploDex, so not in its notices. An AppImage, which would carry the Linux ones, is not published (FR-017).
- **A database locks or closes while About HoploDex is open from its menu**: the screen closes with the rest of that database's screens, as every screen of an open database does (003); it can be opened again from the chooser.
- **A narrow window, or a very long notice**: the screen fits the window, and long texts scroll within it.
- **Searching with no match**: the list says that no component matches.
- **A development build** run with the development server: it shows notices, which may be from the last full build; only a release or end-to-end build is held to Story 2's checks.

## Requirements *(mandatory)*

### Functional Requirements

**About HoploDex (#18)**

- **FR-001**: The application MUST offer an "About HoploDex" screen from the database chooser and from the open database's menu. Opening it MUST NOT need a database, a passphrase or a network connection, and MUST NOT read, write or lock any database or change any setting.
- **FR-002**: About HoploDex MUST show the program's name and the version of the running build; the copyright notice, "Copyright (C) <years> exodious and the HoploDex contributors", whose years run from 2026 to the year of the build's release; a statement that HoploDex is free software that may be shared and changed under the terms of the GNU General Public License version 3 only; a statement that it comes with no warranty, to the extent permitted by law; and the address where its source code is published.
- **FR-003**: The complete GPL-3.0 text MUST be readable within the application, taken from the program itself, not from the network.
- **FR-004**: About HoploDex MUST list every third-party component the running build ships (FR-008), ordered by name, each with its version, the license HoploDex uses it under, and the full copyright and license text that license requires, including any NOTICE file or attribution the license asks to keep. Components that share a license text MAY share its display, provided each component's copyright lines are kept.
- **FR-005**: The list MUST let the user narrow it by typing part of a component's name, and MUST say when nothing matches.
- **FR-006**: The source address MUST be selectable and copyable. About HoploDex MUST NOT open a browser or any other program, and MUST make no network request (constitution V).
- **FR-007**: About HoploDex MUST follow the shared design system and dialog patterns (constitution III), be fully usable by keyboard, meet WCAG 2.1 AA, and keep the window's layout at narrow widths, with long texts scrolling inside the screen. It is a new screen, so it joins the screenshot walk.

**Notices that match the build (#18)**

- **FR-008**: The notices MUST be produced as part of every build of the program, from the dependencies that build ships: the interface packages bundled into it (as the license audit counts them), the backend components compiled into it for its operating system with the features a release enables, and the components FR-009 records. Development-only and test-only components MUST NOT appear. No list of shipped dependencies may be kept by hand.
- **FR-009**: Components the dependency tools can't see MUST be recorded once, each with its notice text and the shipped dependency and version that carries it: today SQLCipher (BSD-3-Clause) and SQLite (public domain), compiled inside the database dependency; OpenSSL 3 (Apache-2.0), linked into the Windows build; and the strength hint's dictionary's ODC-BY attribution, with the dictionary's NOTICE text kept word for word. The check MUST fail, naming the entry, when its carrying dependency is no longer shipped or its version differs from the one recorded.
- **FR-010**: Producing the notices MUST fail, naming the component, when a shipped component's required notice text can't be found, unless an exception records where its text comes from and why. A build MUST NOT produce notices that silently leave a shipped component out.
- **FR-011**: Every build MUST also produce the same notices, together with the copyright notice and the GPL-3.0 text, as a plain-text file that a release bundle carries beside the program (#19 puts it in each bundle).
- **FR-012**: The project's automated checks MUST show that a build's notices list the same components as the license audit counts as shipped for that build, so the two can't drift apart.

**The license audit (#20)**

- **FR-013**: The license audit's result MUST be recorded in this feature's research: for the backend and interface dependency trees, C code compiled inside a dependency, SQLCipher's crypto library on each operating system (the system's library on Linux, CommonCrypto on macOS, OpenSSL 3 linked statically on Windows) and why the OpenSSL version decides compatibility, the MPL-2.0 Exhibit B check, system libraries, and the bundled artwork, icons, fonts and data files: what was checked, how, the result, and the date.
- **FR-014**: The source and license of each strength-hint word list that states none (`wikipedia.json`, `firstnames.json`, `lastnames.json` and `wordSequences.json` in the English dictionary; `passwords.json`, `diceware.json` and `adjacencyGraphs.json` in the common one) MUST be found and recorded with the audit.
- **FR-015**: A word list whose source and license can't be established, or whose license isn't GPLv3-compatible, MUST no longer ship. It MAY be replaced by a list whose source and license are recorded and compatible. The strength hint MUST keep working with the lists that remain (003 FR-003).
- **FR-016**: Every word list that ships and whose license asks for an attribution MUST have that attribution in the notices (FR-009).
- **FR-017**: No bundle published with a release may carry LGPL libraries inside it. Linux releases are a Flatpak (#42), which takes WebKitGTK and the GTK stack from its Flathub runtime, and .deb and .rpm packages, which use the system's libraries. The build MUST keep the ability to make an AppImage (which carries them), but an AppImage MUST NOT be published with a release; publishing one would first need a decision on how it makes its LGPL libraries' corresponding source available. The decision MUST be recorded with the audit and listed for the release process (#19).
- **FR-018**: The development instructions' license audit section MUST describe how the notices are produced, how to record a component the tools can't see or an exception for a missing notice, and which manual checks remain before each release; the checks this feature closes MUST no longer be listed as open.

### Key Entities

- **Component notice**: one third-party component a build ships: its name, version, where it comes from (an interface package, a backend component, compiled-in C code, a linked library, a font, a data file), the license HoploDex uses it under, and its copyright and license text, with any NOTICE or attribution text.
- **Hand-recorded component**: a component notice the dependency tools can't produce, with the shipped dependency and version that carries it and the reason it is recorded by hand.
- **Notice exception**: a shipped component whose notice text is taken from somewhere other than its package, with where and why.
- **Audit record**: the dated result of the license audit, per part of what ships, with how each part was checked.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In a release build for each operating system, 100% of the components the license audit counts as shipped appear in About HoploDex and in the plain-text notices, and 0 development-only components do.
- **SC-002**: Adding or removing a shipped dependency changes the notices on the next build with 0 manual edits; a shipped component with no findable notice fails the check in 100% of cases.
- **SC-003**: From the chooser or an open database, a user reaches the license statement and source address in 2 actions or fewer, and the full GPL-3.0 text in 3 or fewer.
- **SC-004**: About HoploDex opens within 1 second, narrowing the list shows results within 500 milliseconds of typing, and the list scrolls without pauses with every notice loaded (constitution IV).
- **SC-005**: About HoploDex makes 0 network requests and 0 reads or writes of any database or setting.
- **SC-006**: All 7 word lists have a recorded source and compatible license or are absent from the shipped app, and 0 of the manual license checks #20 names remain open.

## Assumptions

- **One feature for two issues**: specified together at the owner's request; Stories 1 and 2 are #18 and Story 3 is #20. The audit (Story 3) may remove a word list, which changes what Story 2 lists, so the plan may order Story 3's word-list check first.
- **Where the screen is reached**: the chooser and the open database's menu cover every state of the app, since a locked or closed database returns to the chooser. A menu bar item on macOS is left to the plan.
- **The source address is shown, not opened**: the app opens no web pages today, and FR-006 keeps it that way; the user copies the address.
- **"Ships" means what the license audit already counts**: interface packages that aren't development-only, and backend components for the target operating system and release features. The end-to-end build's test-only components (for example its WebDriver server) never ship and aren't listed.
- **System libraries are the operating system's** (or, for the Flatpak, its Flathub runtime's), not HoploDex's, and aren't in its notices; no published bundle carries them (FR-017).
- **Word lists**: dropping a list weakens the strength hint only for the words it held; the hint is advice, not a rule (003 FR-003), so a weaker hint is preferred to shipping data whose license is unknown.
- **The bundles themselves** (installers, signing, which formats) are #19's. This feature produces the plain-text notices and records which Linux bundles may be published (FR-017), and #19 puts the notices into each bundle and the bundle rule into the written release process.
- **Testing**: the notices' completeness, the hand-recorded entries' check and the missing-notice failure are tested by automated checks against fixtures and the real dependency trees; About HoploDex's journey (reached from the chooser and from an open database, the license text, narrowing the list) by an end-to-end test that also shows no database is touched (constitution II).

## Relationship to Earlier Features

- **003** (`specs/003-database-protection-management/`): the chooser and the database menu gain About HoploDex. "About databases and security" (FR-030) stays the guide it is. The strength hint (FR-003) and its dictionary's license exception (clarification of 2026-09-26, research §18) are where FR-014 to FR-016 pick up: the attribution recorded there for the release notices is now shown (FR-009), and the word lists left as a manual check are settled.
- **007** (`specs/007-document-preview/`) and **008** (`specs/008-hardening-batch/`): the confined helpers are the same program, so the notices cover them with no separate list.

## Source Request

This feature was filed as two GitHub issues, specified together at the owner's request (`/speckit-specify issues 18 and 20`). Each issue is reproduced verbatim; neither had comments when this spec was written.

### #18: "Show third-party license notices and GPL notices in the app" (labels: new feature, release, licensing)

> ## Request
>
> > build the dependencies' license notices into the app, so every copy carries them.
>
> ## Why
>
> Needed for a GPL-3.0 release. MIT, BSD, Apache-2.0, ISC and OFL-1.1 all require their copyright and license notices to ship with the app, and neither the minified frontend bundle nor the stripped release binary carries them (the license audit, #20, found this). GPL-3.0 §5(d) also asks for "Appropriate Legal Notices" in an interactive UI. One About / Licenses screen can cover both: the app's own copyright, the GPL-3.0 notice, the no-warranty statement and a source link, then the third-party list.
>
> ## Proposed shape
>
> Generate the notices at build time rather than committing them by hand:
>
> - [ ] Rust crates: `cargo about generate`, with the same allow list as `deny.toml`.
> - [ ] Shipped npm packages: a script over `package-lock.json` (like `scripts/check-npm-licenses.mjs`) that reads each package's `LICENSE` file from `node_modules`, including the `@fontsource` OFL notices.
> - [ ] Notices `cargo about` can't see: SQLCipher (BSD-3-Clause, inside `libsqlite3-sys`) and, on Windows, OpenSSL 3 (Apache-2.0), linked into the binary statically.
> - [ ] Emit one generated file, which the frontend imports or the Rust side exposes through a command. Hook the generation into `npm run build` so it can't go stale.
> - [ ] Also ship it as a plain-text file in the bundles (#19).
> - [ ] The About / Licenses screen. It's a new screen, so add it to the screenshot walk in `e2e/screenshots/screens.e2e.ts`.
>
> A small UI feature, so it may be worth a short spec. Lands before the first release.
>
> ---
> _Generated by [Claude Code](https://claude.ai/code)_
>

### #20: "License compatibility: record the audit and finish the release work" (labels: documentation, release, licensing)

> ## Original question
>
> > the application is intended to be GPL v3; do any of the libraries/runtimes used have licenses that cause a conflict?
>
> ## Done
>
> - **The audit and tooling:** `npm run audit:licenses` runs a lockfile check for shipped npm packages and `cargo deny check licenses` for Rust. The policy and the manual checks (SQLCipher/OpenSSL linkage, MPL Exhibit B, system libraries, assets) are under "License audit" in `DEVELOPMENT.md`.
> - **The result:** no incompatible license among shipped dependencies. OFL-1.1 fonts are allowed as separate font files.
> - **The policy and gates** are in constitution 1.2.0 ("Licensing" and the audit and release gates).
> - **The license check in CI:** the dependency audit workflow (`audit.yml`) runs `audit:licenses` (#25).
> - **OpenSSL 3 on Windows:** the Windows environment builds against OpenSSL 3 from vcpkg (`x64-windows-static-md`), linked into the app statically, so no libcrypto DLL ships (#27).
>
> ## Remaining
>
> - [ ] Record the audit result in `research.md`. 001's dependency review (constitution "Security & Data Handling") checked for telemetry, not licenses, and later specs record only the dependencies they added. Cover the Rust and npm trees, the `bundled-sqlcipher` → crypto library linkage (system `libcrypto` on Linux, CommonCrypto on macOS, static OpenSSL 3 on Windows; the OpenSSL version decides GPLv3 compatibility), and the bundled assets (line-art drawings, icons, fonts, data files).
> - [ ] Find the source and license of the zxcvbn word lists that state none: `wikipedia.json`, `firstnames.json`, `lastnames.json` and `wordSequences.json` in `@zxcvbn-ts/language-en`, and `passwords.json`, `diceware.json` and `adjacencyGraphs.json` in `@zxcvbn-ts/language-common` (`DEVELOPMENT.md`, "License audit"). Record the result with the audit, and drop or replace any list that isn't GPLv3-compatible.
> - [ ] Decide how each release bundle that carries LGPL system libraries inside it (the AppImage's WebKitGTK and GTK stack, for example) makes their source available, and write it into the release process (#19). The Flatpak (#42) has the same question.
>
> Tracked elsewhere:
> - Building releases against OpenSSL 3.x: #19.
> - Third-party license notices with each release: #18.
>
> ---
> _Generated by [Claude Code](https://claude.ai/code)_
