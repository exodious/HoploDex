# Feature Specification: Build and Distribute a Flatpak

**Feature Branch**: `011-flatpak`

**Created**: 2026-10-18

**Status**: Draft

**Input**: User description: "Issue #42: Build and distribute a Flatpak". The issue, its decisions of 2026-10-18 and its two comments (from features 007 and 010) are reproduced under Source Request.

## Clarifications

### Session 2026-10-18

- Q: Where does each release publish the Flatpak? → A: Flathub, which gives users updates and a store listing and installs the runtime from Flathub with its source (010 FR-017). The project also builds a `.flatpak` locally for its own testing, but doesn't publish it (FR-020).
- Q: How much of the filesystem may the Flatpak reach? → A: The user's home folder and removable drives (`/media`, `/run/media`, `/mnt`), so every location inside them that works on a `.deb` install works here, typed paths included; anything else only as a single file picked through the desktop's file chooser or dropped, with FR-004's message for whatever needs a folder outside them (FR-003).
- Q: How does the Flatpak reach the keyring for saved passphrases? → A: Through the desktop's Secret Service, as the `.deb` does, with the same entries. The sandbox doesn't narrow what the keyring lets HoploDex reach, which this feature's research records with the permissions (FR-002, FR-006).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A Linux User Installs HoploDex as a Flatpak and Keeps Their Collection in It (Priority: P1)

A collector on Linux installs HoploDex as a Flatpak, from the place releases are published (FR-018), and starts it from their desktop's application list. Everything they do on any other Linux install works the same: they create a database in the suggested `HoploDex` folder in Documents or a folder of their own, open one they already have, on a removable drive too, and the backups appear in `HoploDex backups` beside it or in the folder they chose. They change the passphrase, restore a backup, import and export a spreadsheet, attach photos and documents by choosing them or dropping them from the file manager, preview a document, and open one in its usual application. They save a passphrase so the database opens without asking. Nothing tells them they are in a sandbox unless something they asked for is outside what the sandbox lets HoploDex reach, and then it says what and how to fix it.

**Why this priority**: It is the feature. 010 FR-017 made the Flatpak the published self-contained Linux bundle, in place of the AppImage, so a Linux user who can't or won't use a `.deb` or `.rpm` has no other release to install. A Flatpak that starts but can't keep backups beside the database, or loses the recent list, is worse than none, because it fails at the moment the user needs it.

**Independent Test**: Build the Flatpak and install it into a throwaway Flatpak installation, with throwaway home, config and keyring (FR-021). Walk the journey: create a database in the suggested location, close it (a backup is made beside it), reopen it from the recent list, change its passphrase, restore the backup, import and export a spreadsheet, attach a photo by the file chooser and a document by dropping it, preview a PDF and a TIFF, open a document in another application (with its consent), save the passphrase and reopen without typing it, then do the same with a database on a second mount standing in for a removable drive.

**Acceptance Scenarios**:

1. **Given** the Flatpak is installed, **When** the user starts HoploDex from the desktop's application list, **Then** it starts with its own name and icon, and the database chooser is shown.
2. **Given** the create dialog, **When** the user accepts the suggested location, **Then** the database is created in `HoploDex` in the user's Documents folder, exactly where a `.deb` install would put it (003 research.md §19).
3. **Given** a database the user created or opened in a location the Flatpak may reach (FR-003), **When** a normal close makes a backup, the passphrase is changed or a backup is restored, **Then** each succeeds exactly as on a `.deb` install, with its files in the same places: the backup in `HoploDex backups` beside the database or the chosen folder, the replaced database at its own path.
4. **Given** a database was opened in an earlier run, **When** HoploDex starts again, **Then** it is in the recent list and opens from there, and its saved passphrase, if there is one, still opens it.
5. **Given** an open database, **When** the user drops a photo or a document from the file manager onto a record, **Then** it is attached exactly as on a `.deb` install, whatever form of path the sandbox hands the drop.
6. **Given** a document on a record, **When** the user previews it, **Then** a PDF, a TIFF, an image and a text document each show as on a `.deb` install, under the confinement FR-012 describes.
7. **Given** a document on a record and the "Open documents" setting allowing it, **When** the user opens it in another application and consents (007), **Then** the desktop's usual application for its type opens the decrypted copy, and the copy is deleted when the database closes, as it is on a `.deb` install (001 FR-035).
8. **Given** a file or folder outside what the Flatpak may reach (FR-003), **When** the user names it as a database, a backup folder or an import or export file, **Then** HoploDex says that this install can't reach it and how to give it access or where to move the file, before anything is written, and no file is left half-made.

---

### User Story 2 - The Database's Protections Hold Inside the Sandbox (Priority: P1)

A collector relies on HoploDex locking the database when the computer sleeps, the screen locks, the session ends or they walk away (003). In the Flatpak each of those still locks it, with the same settings and the same timing, and where the sandbox or the desktop can't tell HoploDex about a screen lock, the settings say so, as they already do on a desktop that doesn't report it. The confined helpers that read imports and render TIFF pages are still confined, and still die when HoploDex does. Saved passphrases, decrypted copies and the PDF preview keep the promises they make on any other install, and what the sandbox does and doesn't add is written down for the release security review.

**Why this priority**: The issue names the risk: Flatpak blocks the system bus by default, so without the right permissions locking on sleep would stop working silently. A release that loses a protection without saying so is a security regression, which is why this is P1 beside Story 1 rather than after it.

**Independent Test**: In the Flatpak under the throwaway setup, with a database open: send the sleep, screen-lock and shutdown notices a desktop sends (as the existing OS event checks do, #36) and confirm the database locks for each, in the same time as a `.deb` install; remove the screen-lock services from the session and confirm the setting shows as unavailable; start HoploDex with no keyring service and confirm saved passphrases show as unavailable; read an import and render a TIFF and confirm each helper reports itself confined and is gone once HoploDex is killed; read the recorded statement of what a compromised web process can reach.

**Acceptance Scenarios**:

1. **Given** a database open in the Flatpak, **When** the computer is about to sleep, **Then** the database locks before the computer sleeps, holding the sleep back for as long as a `.deb` install does (003 FR-037).
2. **Given** a database open in the Flatpak and the screen-lock setting on, **When** the screen locks, **Then** the database locks as on a `.deb` install (003 FR-038).
3. **Given** a database open in the Flatpak, **When** the session logs out or the computer shuts down, **Then** it closes at once, keeping unsaved input as pending changes, as on a `.deb` install.
4. **Given** a desktop whose screen lock the Flatpak can't hear about, **When** the user views the settings, **Then** the screen-lock option is shown as unavailable with the existing explanation, never as available but silent.
5. **Given** no keyring service reachable from the sandbox, **When** the user views the chooser or the settings, **Then** saving a passphrase is shown as unavailable, as on any computer without a keyring (003 FR-019), and nothing fails.
6. **Given** an import or a TIFF preview, **When** its helper runs in the Flatpak, **Then** it is confined at least as tightly as on a `.deb` install on the same computer, or it refuses to run, as it already does when its confinement can't be put in place; **and When** HoploDex is killed, **Then** the helper ends at once.
7. **Given** the PDF preview in the Flatpak, where WebKit's own web-process sandbox doesn't run (007 research.md §9), **When** the release security review (#21) reads this feature's research, **Then** it finds what the Flatpak's permissions leave a compromised web process able to reach, recorded with the residual risks.

---

### User Story 3 - The Maintainer Builds the Flatpak From Source, Offline, Like Every Other Bundle (Priority: P2)

The maintainer runs one command beside `npm run bundle:linux` and gets a Flatpak built from the project's own source, with no network during the build, as Flathub requires. The lists of downloaded dependencies the offline build needs are made from the lockfiles, and a check fails when they fall behind. The bundle carries the plain-text notices beside the program (010 FR-011, #19), its desktop entry, AppStream metadata and icon, and none of the runtime it runs on. The dependency and license audit covers what it ships, and the build runs in the dev container where the tools work there, with the development instructions saying how.

**Why this priority**: Without it there is nothing to publish, but it is the maintainer's journey, not the collector's, and Stories 1 and 2 decide what the build must contain.

**Independent Test**: In the dev container, run the Flatpak build command from a clean checkout with the network cut off after the dependency sources are fetched: it produces an installable Flatpak. Change a dependency in a lockfile without updating the offline source lists: the check fails and names them. Open the bundle: the notices file, desktop entry, AppStream metadata and icon are there and pass their validators, and no runtime library is inside.

**Acceptance Scenarios**:

1. **Given** a clean checkout, **When** the maintainer runs the Flatpak build command, **Then** it produces an installable Flatpak of the current version, built from source, with every downloaded input fetched before the build starts and none during it.
2. **Given** a dependency changed in either lockfile, **When** the project's checks run, **Then** they fail, naming the offline source list to regenerate, until it is regenerated.
3. **Given** a built Flatpak, **When** its contents are listed, **Then** it carries the same plain-text notices as every other build of that version, the desktop entry, the AppStream metadata and the program icon at the sizes the desktop asks for, and no library of its runtime (010 FR-017).
4. **Given** the built Flatpak's AppStream metadata and desktop entry, **When** the validators Flathub uses check them, **Then** they pass.
5. **Given** the Flatpak build, **When** the dependency and license audit runs, **Then** it covers every component the Flatpak ships that other builds don't, if any, and the notices list them.
6. **Given** the development instructions, **When** a contributor follows them, **Then** they can build the Flatpak, run it with throwaway data, and know what the build needs on a host where the dev container can't run it.

---

### User Story 4 - Linux Users Get HoploDex and Its Updates Through Flathub (Priority: P2)

A Linux user finds HoploDex on Flathub, installs it with their desktop's software centre or one command, and receives each new release as an update, with the same version, release notes and license as every other bundle of that release. The release process (#19) says how each release reaches Flathub.

**Why this priority**: Updates are one of the issue's reasons for a Flatpak. It follows Story 3 because publishing needs a build, and Flathub's review of a new app runs on its own timetable.

**Independent Test**: Take a release candidate through Flathub's build of the app's submission or update (its test build), install that build, then publish the next version and confirm the installed copy updates to it.

**Acceptance Scenarios**:

1. **Given** a release, **When** the release process is followed, **Then** the Flatpak of that version is published on Flathub with the other bundles, under the app id `io.github.exodious.HoploDex`.
2. **Given** a user with the previous release installed as a Flatpak, **When** the next release is published, **Then** their install offers it as an update, and after it the database they had open opens as before (009's upgrade question if its layout is older).
3. **Given** the Flatpak's Flathub listing, **When** a user reads it, **Then** it names the license as GPL-3.0-only, links the source, and describes what the app is and that it keeps everything on the computer, with screenshots taken from the app.
4. **Given** the published Flatpak, **When** a user installs it, **Then** its runtime comes from Flathub with its source (010 FR-017), not from a copy packed into what HoploDex publishes.

---

### User Story 5 - The Flatpak Is Tested in Its Sandbox, Away From Real Data (Priority: P3)

The maintainer runs an automated check of the Flatpak in its sandbox, which drives the parts the sandbox changes, and a short manual check before each release (#37). Neither ever reaches the developer's real databases, backups, config, keyring or own Flatpak installation.

**Why this priority**: It is how Stories 1 and 2 stay true after this feature ships; it builds on the existing E2E and isolation tooling, so it comes last.

**Independent Test**: Run the automated Flatpak check on a machine with a real HoploDex install and a real Flatpak installation, and confirm afterwards that neither changed: no new file in the developer's Documents, config or `~/.var/app`, no new keyring entry, no app installed into their Flatpak installation.

**Acceptance Scenarios**:

1. **Given** the automated Flatpak check, **When** it runs, **Then** it starts the Flatpak build in its sandbox with the permissions it ships with and covers at least: create, close with a backup, reopen from the recent list, change passphrase, restore, a file dropped onto a record, a document opened in another application, a saved passphrase, and a sleep and a screen-lock notice locking the database.
2. **Given** the automated Flatpak check, **When** it runs, **Then** it uses a throwaway Flatpak installation, home, config, Documents and keyring, and leaves the developer's own untouched (constitution II).
3. **Given** the manual procedures (#37), **When** a release is prepared, **Then** they include the Flatpak's steps the automated check can't take (a real sleep, a real screen lock, a drop from the desktop's own file manager), naming the database and passphrase each step uses.

---

### Edge Cases

- **A database outside what the Flatpak may reach**, chosen through the desktop's file chooser: the chooser can hand the app one file without its folder. Opening it must not lead to a backup, a passphrase change or a restore failing half-way. Each operation that needs the folder says so before it writes anything (FR-004).
- **A typed location**: the create dialog and the backup folder setting take typed paths, which no file chooser grants. A typed path outside what the Flatpak may reach is refused with the same explanation as Story 1, scenario 8.
- **A database on a network share or a synced folder** (003's guide says either works): one mounted where the Flatpak may reach works as on a `.deb` install; one mounted elsewhere (for example a desktop's own network mounts under the user's runtime folder) is treated as outside reach.
- **A file dropped from an application that is itself sandboxed, or dropped from outside reach**: it arrives as a path the sandbox made for that one file. It is read and attached like any other.
- **The same computer with both the `.deb` and the Flatpak installed**: they keep separate settings and recent lists (FR-009). Both can open the same database, though not at once: the file's exclusive lock (003) holds across the sandbox.
- **A `.deb` user moving to the Flatpak**: their recent list and per-computer settings stay with the `.deb` install; the Flatpak starts with an empty recent list, the user opens their databases with Open, and a saved passphrase in the keyring is found again because it is keyed by the database's id, not its path (003 research.md §10). The guide says so (FR-009).
- **A desktop with no keyring service**, or one the sandbox can't reach: saved passphrases show as unavailable (Story 2, scenario 5).
- **A desktop with no portal for opening files in other applications**: opening a document in another application fails with the existing "no application for this document" message, not a hang or a silent nothing.
- **The runtime is updated under an installed HoploDex** (Flathub updates runtimes on their own): HoploDex keeps working on any runtime of the branch it names; the build names a branch, not a single build of it.
- **A runtime branch reaches its end of life**: the release process moves the build to a supported branch before it does (FR-016).
- **Sleep while a backup is being written in the Flatpak**: the backup is stopped and the database locks, as 003 research.md §14 describes, within the time the delay allows.

## Requirements *(mandatory)*

### Functional Requirements

**Running in the sandbox**

- **FR-001**: HoploDex MUST run as a Flatpak with the app id `io.github.exodious.HoploDex`, on a Flathub runtime that provides the web view and toolkit libraries it uses, and MUST behave as on a `.deb` install on the same computer except where this specification says otherwise.
- **FR-002**: The Flatpak MUST ask for no permission it doesn't use, and each permission it asks for MUST be recorded with the behaviour that needs it, so the release security review and the store's own review can check them.
- **FR-003**: The Flatpak MUST be able to reach the user's home folder and removable drives (`/media`, `/run/media`, `/mnt`), and no more of the filesystem, besides the files a user picks through the desktop's file chooser or drops onto the app. Within what it may reach, databases, backups, the replacement of a database file during a passphrase change or restore, and the scratch files each makes MUST work as on a `.deb` install (003 research.md §4, §7).
- **FR-004**: Before an operation writes anything, HoploDex MUST check that the locations it needs (a database's folder, the backup folder, an import or export file) are within reach. When one isn't, the operation MUST stop with a message that names the location, says this install can't reach it, and says how to give access or where to move the file. It MUST NOT leave a scratch file, a partial backup or a half-replaced database.
- **FR-005**: The sleep, wake, screen-lock, logout and shutdown notices MUST reach HoploDex in the Flatpak, including the delay that holds sleep and shutdown back until the database is locked (003 research.md §14). Where a notice can't reach it, the setting that depends on it MUST show as unavailable, as on a desktop that doesn't report it (003 FR-038).
- **FR-006**: Saving a passphrase MUST work in the Flatpak through the desktop's Secret Service, as on a `.deb` install, with entries named as on every other install (service `io.github.exodious.HoploDex`, user `passphrase:<database id>`). When it can't be reached, saving a passphrase MUST show as unavailable (003 FR-019). The permission record (FR-002) MUST say that this reaches the user's whole keyring, as a `.deb` install does.
- **FR-007**: Files dropped onto the app MUST be read and attached whatever form of path the sandbox gives them, including a path the sandbox made for that one file.
- **FR-008**: Opening a document in another application MUST work in the Flatpak through the desktop's portal for it, after the same consent (007, `services::consent`); when the desktop has no application for the type or no such portal, the existing "no application" failure MUST be shown. Decrypted copies MUST be kept inside the app's own sandboxed storage and deleted as on every other install (001 FR-035).
- **FR-009**: The Flatpak's per-computer settings, recent list and caches MUST live in its own sandboxed storage. The in-app guide MUST say that a Flatpak and another install of HoploDex on the same computer keep separate recent lists and settings, and that saved passphrases are shared only if both reach the same keyring.
- **FR-010**: The suggested location for a new database MUST be `HoploDex` in the user's Documents folder, as on a `.deb` install, not a folder inside the sandbox's own storage.
- **FR-011**: The PDF preview MUST work in the Flatpak. Where WebKit's own web-process sandbox doesn't run (inside Flatpak, 007 research.md §9), this feature's research MUST record what the Flatpak's permissions (FR-002) leave a compromised web process able to reach, as a residual risk for the release security review (#21).
- **FR-012**: The import reader and the render helper MUST be confined in the Flatpak at least as tightly as on a `.deb` install on the same computer, or refuse to run as they already do when their confinement can't be put in place, and MUST end at once when HoploDex ends (008).
- **FR-013**: Every quit, shutdown and signal path that clears decrypted copies and keeps pending changes MUST behave in the Flatpak as on a `.deb` install.

**Building it**

- **FR-014**: One project command, documented beside `npm run bundle:linux`, MUST build the Flatpak from the project's source with every downloaded input fetched before the build starts and no network during it. It MUST run in the dev container, or the development instructions MUST say what it needs and why it can't.
- **FR-015**: The lists of downloaded dependencies the offline build uses MUST be made from `Cargo.lock` and `package-lock.json` by a documented command, and the project's checks MUST fail, naming the list, when either lockfile changes without it.
- **FR-016**: The build MUST name a runtime branch that Flathub supports and that provides the web view version HoploDex needs; the release process MUST move to a newer branch before the named one reaches its end of life.
- **FR-017**: The Flatpak MUST carry the build's plain-text notices beside the program (010 FR-011), a desktop entry, AppStream metadata (name, summary, description, license `GPL-3.0-only`, source address, releases, screenshots and content rating) and the program icon at the sizes the desktop needs, generated from the program icon's source (`npm run icons`). The metadata and desktop entry MUST pass the validators Flathub uses.
- **FR-018**: The Flatpak MUST NOT carry any library of its runtime (010 FR-017); a check on the built bundle MUST fail if it does.
- **FR-019**: The dependency and license audit MUST cover what the Flatpak build ships, and the notices its build produces MUST list the same components as the other Linux builds of that version plus anything only the Flatpak ships.

**Publishing it**

- **FR-020**: Each release MUST publish the Flatpak on Flathub, built there from the same recipe and source as the project's own build, at the same version as the other bundles, and the release process (#19) MUST say how, including the first submission and Flathub's verification of the app id. A `.flatpak` bundle built locally MUST NOT be published with a release.

**Testing it**

- **FR-021**: An automated check MUST run the Flatpak build, with the permissions it ships with, in a throwaway Flatpak installation with throwaway home, config, Documents and keyring, and cover Story 5's journey. It MUST NOT read, write or delete the developer's real databases, backups, config, keyring entries or Flatpak installation (constitution II; CLAUDE.md, "Never touch the real databases").
- **FR-022**: The manual procedures run before each release (#37) MUST include the Flatpak steps the automated check can't take, naming the database and passphrase each step uses.
- **FR-023**: The development instructions MUST describe how to build, install into a throwaway installation, run and test the Flatpak, and how to regenerate the offline source lists; CLAUDE.md MUST point to them.

### Key Entities

- **Flatpak manifest**: the build recipe: the runtime and its branch, the app's permissions with the reason for each, and the sources the build uses.
- **Offline source list**: one per lockfile, every downloaded dependency with its checksum, made from that lockfile.
- **AppStream metadata**: the store listing: name, summary, description, license, source address, releases with their notes, screenshots, content rating.
- **Permission record**: each permission the Flatpak asks for, with the behaviour that needs it and what it lets a compromised process reach.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of Story 1's journey (create, back up, reopen, change passphrase, restore, import, export, attach by chooser and by drop, preview, open in another application, saved passphrase) succeeds in the Flatpak for a database in a location it may reach, with the same files in the same places as a `.deb` install.
- **SC-002**: In the Flatpak, the database locks on sleep, screen lock, logout and shutdown in 100% of the automated check's runs, within the same time as a `.deb` install on the same computer; 0 notices are lost silently.
- **SC-003**: 0 operations leave a scratch file, partial backup or half-replaced database when a location is out of reach; each one names the location and the fix.
- **SC-004**: The Flatpak builds from a clean checkout with 0 network requests during the build, and a lockfile change without regenerated source lists fails the checks in 100% of cases.
- **SC-005**: The built Flatpak's metadata and desktop entry pass Flathub's validators with 0 errors, and the bundle carries 0 runtime libraries.
- **SC-006**: Interactive actions stay within the constitution's budgets in the Flatpak (search ≤500 ms and actions ≤1 s at 10,000 items), and a cold start reaches the chooser within 1 second of a `.deb` install's cold start on the same computer.
- **SC-007**: After the automated Flatpak check runs on a machine with real data, 0 files, settings, keyring entries or installed apps of the developer's own have changed.
- **SC-008**: A user with the previous Flatpak release installed receives the next release as an update with 0 manual steps beyond their desktop's usual update.

## Assumptions

- **Replaces the AppImage as the published self-contained bundle**: 010 FR-017 decided this; the AppImage stays buildable and unpublished, and nothing here changes it. The `.deb` and `.rpm` are unchanged.
- **The runtime**: GNOME's Flathub runtime is the likely fit, since it carries WebKitGTK 4.1 and GTK 3; the plan confirms the branch and the web view version against what HoploDex needs (FR-016).
- **Built from source**: Flathub requires a build from source with no network, so the plan builds from source rather than repackaging the `.deb`. The recipe the project builds with is the one Flathub builds from (FR-020).
- **Linux on x86-64 first**: other architectures are in scope only if the release process (#19) builds other Linux bundles for them.
- **No change to the database format, IPC or the frontend's behaviour** beyond the out-of-reach messages (FR-004) and the guide's note (FR-009). If those messages change a dialog, the dialog patterns and screenshots follow (constitution III).
- **Sleep and shutdown notices keep coming from logind** with the permission to talk to it on the system bus, since the desktop portal reports a session ending but offers no delay before sleep; the plan confirms the store accepts the permission and records it (FR-002).
- **Signing, installers and the rest of the release** are #19's; this feature supplies the Flatpak and its part of the written release process.
- **Automating the OS event checks generally** is #36's; this feature's automated check sends the notices the Flatpak needs to see (Story 5) and reuses whatever #36 has built by then.
- **Testing**: the sandbox-specific behaviour (out-of-reach checks, path forms for drops, keyring and notice availability) is tested at the cheapest level that shows it; the journey under the real sandbox by the automated Flatpak check (FR-021), which runs where Flatpak tooling is available and is documented where it isn't (constitution II).

## Relationship to Earlier Features

- **003** (`specs/003-database-protection-management/`): the database's folder, backups, file replacement, keyring, OS notices and the guide are where the sandbox bites (FR-003 to FR-006, FR-009, FR-010). Nothing in 003's behaviour changes on other installs.
- **007** (`specs/007-document-preview/`): the PDF preview runs without WebKit's sandbox inside Flatpak, as its research.md §9 already handles; FR-011 records what that leaves reachable. Opening a document in another application goes through the desktop's portal (FR-008).
- **008** (`specs/008-hardening-batch/`): the helpers' confinement and their dying with the app must hold in the sandbox (FR-012).
- **010** (`specs/010-license-notices/`): FR-017 made the Flatpak the published self-contained Linux bundle and set that it carries no runtime library; FR-011's notices file goes in the bundle (FR-017 here).

## Source Request

GitHub issue #42, "Build and distribute a Flatpak" (labels: new feature, needs spec, release), reproduced verbatim with its two comments.

> New feature, not yet specified. Run it through `/speckit-specify`. The spec's `## Source Request` section should quote the request below verbatim.
>
> ## Decided (2026-10-18)
>
> Moved to 0.1.0. Spec 010 (FR-017, `specs/010-license-notices/spec.md`) settles which Linux bundles a release publishes: the Flatpak, `.deb` and `.rpm`, none of which carries the LGPL libraries (WebKitGTK, the GTK stack) itself. The AppImage stays buildable (`scripts/build-appimage.sh`), but isn't published with a release. Publishing one later would first need a decision on how it makes those libraries' source available. So the Flatpak no longer comes "in addition to the AppImage": it replaces the AppImage as the published self-contained Linux bundle. Points below that compare the two (shared config, the LGPL note) apply only to a locally built AppImage.
>
> ## Original request
>
> > Create a github issue for supporting flatpak builds in addition to appimage
>
> ## Why
>
> - The AppImage bundles the build machine's display-stack libraries, and we had to work around it to start on hosts with a newer Mesa (`scripts/build-appimage.sh`, tauri-apps/tauri#15976). A Flatpak runs on the runtime's own WebKitGTK, GTK and Mesa, so that whole class of host mismatch doesn't apply.
> - Flatpak gives Linux users updates and a sandbox, and it's the usual way to reach Flathub.
> - Tauri's `bundle.targets` produces `.deb`, `.rpm` and AppImage, but not Flatpak, so this needs its own manifest and build step. The app id `io.github.exodious.HoploDex` is already reverse-DNS, so it can be the Flatpak app id as it is.
>
> ## Things the sandbox affects (to settle in the spec)
>
> - **Database and backup files.** The user can keep a `.hoplodex` file anywhere, and backups go in a `HoploDex backups` folder next to it or a folder they chose. The app also replaces files atomically (`services/file_swap`) and reads the open database through `db/raw_file.rs`. A portal file chooser grants access to the one file picked, not to its folder, so backups and file swaps next to the database may fail. Decide between `--filesystem=home` / `xdg-documents`, asking the user for the backup folder through the portal, or something else, and what the sandbox promises about "never touches other files".
> - **Saved passphrases.** `services/keyring` uses the OS keyring (Secret Service). The sandbox needs `--talk-name=org.freedesktop.secrets`, or the Secret portal. Check that the "no keyring available" path (research.md §10) still behaves.
> - **Sleep, shutdown and screen-lock events.** `platform/linux.rs` listens to logind on the *system* bus and to the `org.freedesktop.ScreenSaver` / `org.gnome.ScreenSaver` names on the session bus, and takes a delay inhibitor. Flatpak blocks the system bus by default, so this needs `--system-talk-name=org.freedesktop.login1` and the ScreenSaver names, or another route. Without them, locking on sleep would silently stop working, which is a security regression (issue #36 is about testing these events).
> - **File drops.** Drops arrive as filesystem paths through Tauri's native drag-drop event. Inside a sandbox a dropped path may be a document-portal path (`/run/user/<uid>/doc/...`); check the `add_*_from_path` commands and that a database or attachment dropped this way can be read.
> - **Config and data locations.** `machine.json` lives under the app's config directory, which becomes `~/.var/app/<id>/config` in a Flatpak. The recent-databases list and any saved paths need to stay valid, and the AppImage and Flatpak won't share them. Note this in the guide.
> - **Decrypted document copies** are cleared on exit and at startup (FR-035); confirm the temp location is inside the sandbox and that "open with the default application" still works (it needs the OpenURI portal).
>
> ## Build and release
>
> - Pick a runtime (`org.gnome.Platform` is the likely fit for WebKitGTK 4.1) and check that the version we use is there.
> - Decide how the Flatpak is built: from source with `flatpak-builder`, or from the `.deb`. Flathub needs an offline build, which means generated Cargo and npm source lists (`flatpak-cargo-generator`, `flatpak-node-generator`). Those must stay in step with `Cargo.lock` and `package-lock.json`, and the dependency and license audit still applies.
> - Add the metainfo (AppStream) file, desktop entry and icon (`npm run icons` already generates the program icon).
> - Add `npm run bundle:flatpak` (or similar) beside `bundle:linux`, and document it in `DEVELOPMENT.md`. Run it in the dev container if `flatpak-builder` works under podman there; otherwise say what it needs.
> - Decide whether to publish to Flathub or only build a `.flatpak` for GitHub releases. Flathub has its own review and source-availability rules; the LGPL note under "System libraries" in `DEVELOPMENT.md` still applies to whatever we ship.
>
> ## Testing
>
> - Run the E2E suite, or a smoke subset, against the Flatpak build under the sandbox, with throwaway `XDG_*`/`HOME` so a session never reaches the developer's real databases, config or keyring (see "Test isolation" in `DEVELOPMENT.md`).
> - A manual check per release (see #37): open, lock on sleep and on screen lock, saved passphrase, backup, restore, drop a file, open an attachment.
>
> ## Related
>
> - #36 automate the OS event checks (sleep, screen lock, log-out).
> - #37 manual test procedures run before each release.
>
> ---
> _Generated by [Claude Code](https://claude.ai/code)_

### Comment from feature 007 (PR #80), 2026-10-08

> ## A note from feature 007 (PR #80) for the Flatpak spec
>
> **WebKit's sandbox doesn't run inside Flatpak.** 007 previews PDFs in a child web view with WebKitGTK's own viewer, and on Linux turns WebKit's web-process sandbox on where a startup probe shows it works (`specs/007-document-preview/research.md` §9). Inside Flatpak (`/.flatpak-info`) the probe is skipped and the sandbox isn't turned on, since WebKit itself won't sandbox there and the app is already in Flatpak's. A Flatpak build therefore previews PDFs without WebKit's sandbox, under Flatpak's own sandbox instead. research.md §1 records this among the residual risks for the release security review (#21); the spec should say what the Flatpak's permissions leave a compromised web process able to reach.
>
> ---
> _Generated by [Claude Code](https://claude.ai/code)_

### Comment from feature 010 (PR #127), 2026-10-18

> ## A note from feature 010 (PR #127) for the Flatpak spec
>
> **LGPL libraries come from the runtime.** The Flatpak's WebKitGTK and GTK stack come from its Flathub runtime, and Flathub distributes that runtime with its source. So the Flatpak is a published Linux bundle that carries no LGPL library itself (010 FR-017, research.md §4). That holds as long as the runtime is installed from Flathub and not packed into what we publish. If the spec chooses a GitHub-release `.flatpak` over Flathub, it should check that the bundle doesn't include the runtime. The notices don't list the runtime's libraries, which are treated like the system's (010 spec, Assumptions). The "System libraries" note in DEVELOPMENT.md's "License audit" now states this rule.
>
> **The bundle should carry `NOTICES.txt` too, beside the program.** Every `npm run build` writes `dist/NOTICES.txt` for the build's target. Tauri embeds `dist/` in the program, though, so a bundle doesn't get it as a file unless its build adds it (#19's written process is to add it to every bundle).
>
> **For the offline build:** the build's notices step needs `node_modules`. It also runs `cargo fetch --locked`, `cargo tree` and `cargo metadata` on `src-tauri/Cargo.toml` and reads each crate's license files from its source folder. Under `flatpak-builder` those have to work from the generated Cargo and npm source lists with no network, which still needs confirming. `git` is optional: without it, the copyright's last year is the build's own year.
