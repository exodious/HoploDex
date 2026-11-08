# Feature Specification: Troubleshooting Log

**Feature Branch**: `fm/hoplodex-troubleshooting-log`

**Created**: 2026-11-07

**Status**: Draft

**Input**: GitHub issue #128, "Troubleshooting log: a log file users can send, and a level they can raise before start". A log on the user's own computer, turned on and raised to gather diagnostics for a repeatable reported issue, on by default before 1.0.0 and opt-in from 1.0.0, designed to hold nothing sensitive.

## Clarifications

### Session 2026-11-07

The owner set the direction first: "My intention is for this logging to be selectively turned on as gathering diagnostics for user reported repeatable issues. I think prior to the 1.0.0 release I'll want it defaulted to on (opt-out) and switch it back to opt-in for the 1.0.0 release. It'll be important that the log not contain anything sensitive." Then the owner answered the batched decisions: "All recommended plus the library change and the softer notice."

- Q: How does the default change at 1.0.0? → A: It is worked out from the app's version: on when the major version is 0, off otherwise. There is one place and nothing to remember at release. A test pins `0.x` on, and `1.0.0` and `1.0.0-rc.1` off (FR-001).
- Q: What happens to someone who never chose, at 1.0.0? → A: The log goes off and their files are deleted at the first start. A saved On stays on, because that is an opt-in; a saved Off stays off (FR-002, FR-012).
- Q: Where is it turned on and off? → A: A saved choice on this computer, with the control in About, which can be reached from the chooser, so no database needs to be open (FR-003).
- Q: How is the level raised? → A: Two ways, neither saved. A button in About, "Detailed until you quit", works at once. `HOPLODEX_LOG=off|on|debug` covers one run, for issues during startup. `debug` is the only raised level: no `trace`, no per-module filters (FR-004, FR-005).
- Q: Where is the file, how big, for how long? → A: `<app cache directory>/logs/hoplodex.log`, 2 MiB with one rotated file (4 MiB at most), deleted after 14 days and at once when the log is turned off (FR-006 to FR-012).
- Q: May the log name paths? → A: No, only their shape, such as `~/*/*.hoplodex`: the root and depth, with every name hidden. The same rule applies to standard error and the journal, not only the file (FR-014, FR-015).
- Q: What else is sensitive? → A: A line holds only fixed text, HoploDex's own codes, OS error text and codes, durations and path shapes. No record values, file names, passphrases, draft text, counts of a user's records, row ids or SQLite message text; time is seconds since start, with only the date in a run header (FR-013, FR-016 to FR-018).
- Q: Which lines reach the file? → A: Only HoploDex's own. Library warnings stay in standard error and the journal (FR-019). This narrows what the log can explain: see "What this leaves out".
- Q: What is a user told while it is on by default? → A: A one-time chooser notice with a Turn off button, the state in About, and the same wording in the README and the store listing. The wording says what the log is **designed to leave out**, not that it never contains something, because OS error text and untested paths can't be ruled out; the claim is no wider than the tests and the review question support (FR-020, FR-021, FR-022).
- Q: How does a user find the file? → A: About shows the state and the real path as selectable text, and a "Show log file" button. There is no "Copy log": the web view never reads the log's contents (FR-023, FR-024).
- Q: Debug lines and crashes? → A: About 30 debug lines across open, close and lock, backups, import, helpers, preview and the portals, written to the file only, so a raised level never adds to the journal. A panic logs its source location, never its message. Native crashes stay out (FR-025 to FR-027).
- Q: How is the rule kept? → A: A real-persistence journey test, the same scan in one E2E journey, and a source check on the log macros. They cover the paths they walk and the patterns they match; `docs/attack-surface.md` keeps a review question for the rest (FR-028 to FR-030).
- Q: Do the helpers or the sandbox probe log for themselves? → A: No. They get no file access and no logging; the parent logs for them, as today, and another piece of work is restricting what the helpers can open (FR-031).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Gather a log for a repeatable issue (Priority: P1)

A user has a problem they can repeat and has been asked for a log. They open About, turn the log on if it is off, choose "Detailed until you quit", repeat the problem, press "Show log file" and attach the file to their report.

**Why this priority**: This is the request. Without it, a reported bug on Windows or a Finder-started Mac leaves nothing to look at.

**Independent Test**: Start the app, raise the level in About, do something that writes debug lines, and read the file named in About.

**Acceptance Scenarios**:

1. **Given** the log is on, **When** the user chooses "Detailed until you quit", **Then** debug lines are written from that moment, with no restart.
2. **Given** the level is raised, **When** the user quits and starts again, **Then** the level is back to normal with nothing to turn off.
3. **Given** the log is off, **When** the user turns it on in About, **Then** the file starts at once and the choice is saved for later starts.
4. **Given** a log file exists, **When** the user presses "Show log file", **Then** the OS shows it in its folder, in the Flatpak too.
5. **Given** About is open, **When** it shows the log's state, **Then** it also shows the file's real path as text that can be selected.
6. **Given** no database is open, **When** the user opens About from the chooser, **Then** all of this is available.

---

### User Story 2 - The log is designed to hold nothing sensitive (Priority: P1)

A user sends the file to someone, possibly on a public tracker. The log gives enough to diagnose the problem without saying who they are, what their database is called, what is in it or how much of it there is.

**Why this priority**: The owner's condition for the feature: "It'll be important that the log not contain anything sensitive."

**Independent Test**: Run a journey at the raised level with distinctive planted text in the home folder, database name, record values, document names and passphrase, and search the output for each.

**Acceptance Scenarios**:

1. **Given** a database at `<home>/Guns/Smith family.hoplodex`, **When** a line refers to it, **Then** it reads `~/*/*.hoplodex`: no user name, folder name or database name.
2. **Given** a database on a removable drive or a network share, **When** a line refers to it, **Then** the drive's or server's name and the user name are hidden, and whether it is a removable drive is still visible.
3. **Given** a record, a document attached under a telling name and a draft, **When** they are saved, opened, backed up, exported, imported or fail, **Then** none of their text reaches the log.
4. **Given** a collection of any size, **When** the log is read, **Then** it doesn't say how many records there are.
5. **Given** a passphrase is typed, changed or saved, **When** the log is read, **Then** the passphrase isn't there.
6. **Given** an always-on log, **When** it is read, **Then** it doesn't say at what time of day the collection was opened, only seconds since the run began.

---

### User Story 3 - Told, and able to say no, while it is on by default (Priority: P2)

A user of a released 0.x build has a log on their computer they didn't ask for. They are told once, plainly, and can turn it off from the notice or from About. At 1.0.0 the log stops by itself unless they chose it.

**Why this priority**: Released 0.x builds count as before 1.0.0, so real users are affected. A default-on log with no notice would be a surprise.

**Independent Test**: Start a 0.x build with no saved choice and read the chooser; turn the log off from the notice; check the file is gone and stays gone at the next start.

**Acceptance Scenarios**:

1. **Given** a 0.x build and no saved choice, **When** the chooser first appears, **Then** the notice is shown once, with a Turn off button, and is not shown again.
2. **Given** the notice, **When** the user presses Turn off, **Then** the log stops, its files are deleted and Off is saved.
3. **Given** a 1.0.0 build and no saved choice, **When** the app starts, **Then** no log is written, any files from before are deleted and no notice is shown.
4. **Given** a saved On, **When** the app is upgraded to 1.0.0, **Then** the log stays on.
5. **Given** a saved Off, **When** the app starts at any version, **Then** nothing is written.

---

### User Story 4 - An issue during startup (Priority: P2)

The problem happens before the user can reach About. They are given one line to run, the app starts with detailed logging for that run only, and nothing is saved.

**Why this priority**: The issue asks for a level that can be raised before start, and startup failures can't use the button.

**Independent Test**: Start the app with `HOPLODEX_LOG=debug` and check for debug lines from the start of the run, then start without it and check for none.

**Acceptance Scenarios**:

1. **Given** `HOPLODEX_LOG=debug`, **When** the app starts, **Then** debug lines are written from the first line, including those from before the app's folders were known.
2. **Given** `HOPLODEX_LOG=off`, **When** the app starts with a saved On, **Then** nothing is written for that run, existing files are left alone and the saved choice is unchanged.
3. **Given** `HOPLODEX_LOG=on` with a saved Off, **When** the app starts, **Then** the log is written at the normal level for that run only.
4. **Given** an unknown value, **When** the app starts, **Then** it is ignored and a line says so without quoting it.

---

### User Story 5 - The file stays small and goes away (Priority: P3)

The log never grows without bound, doesn't linger, and doesn't travel with the user's data.

**Why this priority**: A file on disk is new surface; these limits are what make it acceptable.

**Independent Test**: Write past the cap and count the files; age a file and start the app; make a backup and an export and look for the log in them.

**Acceptance Scenarios**:

1. **Given** the file reaches 2 MiB, **When** another line is written, **Then** it becomes the one rotated file, replacing any earlier one, and a new file starts.
2. **Given** a file older than 14 days, **When** the app starts, **Then** that file is deleted.
3. **Given** the user turns the log off, **When** the action completes, **Then** both files are deleted at once.
4. **Given** a backup or an export, **When** it is made, **Then** the log is in neither.
5. **Given** something other than a plain file at the log's name, such as a symbolic link, **When** the app starts, **Then** it is not followed or changed and no file log is kept for that run.

---

### Edge Cases

- **The folder can't be created or written**: the app runs normally, standard error is unchanged, About says the file couldn't be written, and there is no error dialog. A log must never stop the app.
- **A disk fills up during a run**: the failed line is dropped; the app carries on.
- **A line holds a newline or a terminal control code** (from OS error text): it is escaped, so it can't forge a line or drive a terminal when the file is shown.
- **Two copies of the app at once**: each appends whole lines. A rotation by one may cut the other's history short. Acceptable for a diagnostic file.
- **Lines from before the app's folders are known**: held in memory, within a limit, and written once the file starts, or dropped if the log is off.
- **The level is raised while the log is off**: not possible. "Detailed until you quit" appears only while the log is on, because a later start with the log off would delete what was gathered.
- **The app can't finish starting**: the reason goes to standard error, and to the file when `HOPLODEX_LOG` set this run's state, since then the file starts before anything in `setup` can fail. A failure in Tauri's `build()`, before `setup` runs, reaches standard error only, because no folder can be named yet. With no variable the file isn't touched, because only the saved choice, not yet read, can say what happens to it.
- **A panic while the logger is in use**: the panic line is skipped rather than risking a deadlock.
- **The user deletes the file while the app runs**: the app carries on writing to what it has open until the next rotation or start. Show log file then reports that there is no file, without an error dialog.

## Requirements *(mandatory)*

### Functional Requirements

**On and off**

- **FR-001**: The default MUST be worked out in one place from the app's version: on when the major version is 0, off otherwise. A test MUST pin `0.x` on, and `1.0.0` and `1.0.0-rc.1` off.
- **FR-002**: A user's saved choice MUST take precedence over the default, at every version. A user who never chose MUST follow the default.
- **FR-003**: The user MUST be able to turn the log on and off from About, reachable with no database open. The choice MUST be saved on this computer, take effect at once, and be kept outside any database.
- **FR-004**: "Detailed until you quit" MUST raise the level at once while the log is on, end when the app quits, and never be saved. Nothing lowers it short of that but turning the log off.
- **FR-005**: `HOPLODEX_LOG` MUST accept `off`, `on` and `debug`, apply to that run only, take precedence over the saved choice for that run, and change nothing that is saved. A choice made in About during the run supersedes it. An unknown value MUST be ignored and reported without being quoted.

**The file**

- **FR-006**: The file MUST be `<app cache directory>/logs/hoplodex.log`, resolved the way the app's other directories are, so tests and E2E runs stay in their sandbox.
- **FR-007**: A file MUST NOT exceed 2 MiB; at the cap it becomes `hoplodex.log.1`, replacing any earlier one, so at most 4 MiB exists.
- **FR-008**: At each start, unless `HOPLODEX_LOG=off` leaves the files for that run (FR-012), a log file made over 14 days ago MUST be deleted, however recently it changed; where the file system doesn't record when a file was made, its last change counts.
- **FR-009**: Turning the log off in About or from the notice MUST delete both files at once.
- **FR-010**: A new file MUST be created exclusively and readable only by this user on Unix. An existing object that isn't a plain file of this user's MUST be left alone and no file log kept for that run.
- **FR-011**: The app MUST carry on normally if the log can't be written, and About MUST say so.
- **FR-012**: A start with the log off, by default or by a saved choice, MUST delete any log files left over. A run turned off only by `HOPLODEX_LOG=off` MUST leave them.
- **FR-013**: The file MUST never be part of a backup or an export.

**What a line may hold** (this replaces the earlier rule in `logging.rs` and CLAUDE.md, on every output)

- **FR-014**: A line MUST NOT name a path. It MAY give a path's shape: its root (home as `~`, a drive letter, a removable-drive or mount root, the Flatpak's document portal), its depth, and an extension from a short list, with every other name hidden. A network share's server and share names MUST be hidden.
- **FR-015**: FR-014 and FR-016 to FR-018 MUST apply to standard error and the journal as well as the file, so there is one rule.
- **FR-016**: A line MUST NOT hold a record's values, a photo's or document's name, draft text, a passphrase, a count of a user's records, a row id, or SQLite's message text (SQLite's result code is enough). It MAY hold fixed text, HoploDex's own codes, OS error text and codes, durations, and path shapes.
- **FR-017**: As a backstop for OS error text, the output MUST replace the home folder's path wherever it appears in a line, and escape line breaks and control characters.
- **FR-018**: Time in the file MUST be seconds since the run began, with only the date in a header at the start of each run. The header MUST also give the app's version and the OS, and nothing about the computer's or user's name.
- **FR-019**: Only HoploDex's own lines MUST reach the file, at any level. Other crates' lines MUST stay at `warn` in standard error and the journal, and MUST NOT reach the file.

**What the user is told**

- **FR-020**: With a 0.x build and no saved choice, the chooser MUST show a notice once on this computer, with a Turn off button.
- **FR-021**: The notice MUST read: "While HoploDex is before version 1.0, it keeps a troubleshooting log on this computer, up to 4 MiB and for up to 14 days. It is designed to leave out your records, file names, passphrases and how many items you have. Nothing is sent anywhere, and you can read it before you share it. You can turn it off in About."
- **FR-022**: The README and the store listing MUST carry the same claim, no wider, and the README MUST say how to gather a log for a report, including the one-line start on each OS.

**Finding it**

- **FR-023**: About MUST show the log's state (off, on, or detailed) and the file's real path as selectable text.
- **FR-024**: "Show log file" MUST show the file in the OS's file manager. The command MUST take no path from the web view, and the web view MUST NOT be able to read the log's contents.

**Debug lines and crashes**

- **FR-025**: The app MUST write debug lines for opening, closing and locking a database (with the reason), backups, import phases, helper start and exit status, preview, and portal calls and their results, all within FR-016.
- **FR-026**: Debug lines MUST go only to the file. Raising the level MUST NOT change what standard error or the journal receive.
- **FR-027**: A panic MUST write its source location to the file and MUST NOT write its message, which can quote a value. In a release build the same holds for standard error and the journal; a debug build's standard error MAY keep the message. Native crashes are out of scope.

**Keeping the rule**

- **FR-028**: A test with real persistence MUST run a journey at the raised level with planted text in the home folder, database name, record values, document names and passphrase, and fail if any reaches the output.
- **FR-029**: One E2E journey MUST do the same against the real file written by the real app.
- **FR-030**: A source check MUST fail on a log macro that prints a path other than through the shape function.

**Helpers**

- **FR-031**: The render helper, the import helper and the sandbox probe MUST NOT log, open the file, or receive `HOPLODEX_LOG`. The parent logs for them.

### What this leaves out

Named so the limits are on record:

- **Library warnings aren't in the file (FR-019).** A warning or error from Tauri, wry, tao, the dialog plugin or zbus reaches only standard error and the journal. On Windows, and on a Mac app opened from Finder, it reaches nothing, so a failure inside a library shows in the log only as far as HoploDex's own line around it says. Someone diagnosing such a failure asks the reporter to start the app from a terminal.
- **Debug lines aren't in standard error (FR-026).** A developer at a terminal follows the file instead.
- **A release build's panic has no message (FR-027).** A report gives the source location only, and the reporter's steps have to supply the rest.
- **Shapes lose detail (FR-014).** Two databases at the same depth look alike, and a bug about a particular character in a path can't be seen in the log.
- **No wall-clock times (FR-018).** Matching a line to a time of day takes the reporter's word.
- **Not built:** sending from inside the app, Copy log, encrypting the file, configurable sizes or retention, `trace`, per-module filters, capturing native crashes, and capturing WebKit's own output.

### Key Entities

- **Log choice**: what this computer has saved: on, off, or nothing chosen. Kept in `machine.json`.
- **Log state**: what applies now: off, on or detailed, worked out from the version's default, the saved choice, the variable and anything done in About this run.
- **Log file**: the current file and at most one rotated file, plain text, in the app's cache directory.
- **Log notice**: the one-time statement to the user, with the fact that it has been shown kept on this computer.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With the log on, a user can go from About to a detailed log and find its file in under a minute, with no terminal.
- **SC-002**: With text planted in the home folder, database name, record values, document names and passphrase, none of it appears in the output of the tested journeys, on Linux, macOS, Windows and in the Flatpak.
- **SC-003**: The log's files never take more than 4 MiB, and no file outlives 14 days from when it was made (FR-008) once the app has started again without `HOPLODEX_LOG=off`.
- **SC-004**: A build at 1.0.0 writes no log for a user who never chose one, with no step at release.
- **SC-005**: Turning the log off leaves no log file behind, at once.
- **SC-006**: Search stays within 500 ms and actions within 1 s at 10,000 items with the level raised.
- **SC-007**: A run with no log and no variable set writes exactly what it writes today to standard error, apart from paths becoming shapes, the lines FR-016 changes and a release build's panic (FR-027).

## Assumptions

- The app's version is the one source of truth for "before 1.0.0"; a pre-release of 1.0.0 counts as 1.0.0.
- A reporter is asked for a log by someone who can tell them what to do; the app doesn't prompt for one after a failure.
- Same-user processes can read the file, as they can `machine.json`. The design relies on what is left out of the file, not on protecting it.
- A log is not secret by design, but it is deleted the way the app deletes other files it no longer wants, since OS error text can't be ruled out.
- On macOS, Time Machine excludes `Library/Caches` (checked on a macOS 27 machine). On Linux, many home backup tools skip `~/.cache`, but that is the user's tool's choice. On Windows the folder isn't roamed.
- No new crate or native dependency is expected.

## Relationship to Earlier Features

- **011 (Flatpak)** added the stderr logger and accepted the log reaching standard error or the journal ([011 research.md §20](../011-flatpak/research.md)). This feature keeps that for HoploDex's normal lines and library warnings, and changes what those lines may say.
- **`docs/attack-surface.md`'s `logging` entry** is widened by this feature, and the `ipc-commands` entry gains the new unscoped commands.

## Source Request

GitHub issue #128, "Troubleshooting log: a log file users can send, and a level they can raise before start". It is reproduced verbatim below.

> ## Request
>
> When a user reports a bug, we may need a log they can send, with more detail than usual. Feature 011 added the app's first logger, but there is no way to turn on more detail before the app starts, and on most platforms no log a user can find.
>
> ## Where things stand (as of 011, `src-tauri/src/logging.rs`)
>
> - The `log` macros go to **standard error only**. Our crates log at `info`, other crates at `warn`. The levels are fixed in the code: no environment variable, setting or `machine.json` field.
> - **Linux and the Flatpak:** a desktop launch puts stderr in the systemd journal. Elsewhere there is nothing to collect:
>   - **Windows:** release builds use `windows_subsystem = "windows"`, so there is no console.
>   - **macOS:** an app opened from Finder sends stderr to `/dev/null`.
> - **Not much to raise:** there are about 185 log calls, 166 of them `error`/`warn` on failure paths. Exactly one is `debug`, so raising the level would add almost nothing until debug lines are written.
> - **What a line may carry:** nothing from inside a database. No record values, no original photo or document file names (nor their source paths or copies named after them), no draft text, never a passphrase. Database, backup and app-file paths may be named (CLAUDE.md; `logging::folder_of`, `logging::json_error`).
>
> ## Decisions to make at spec time
>
> - **The log file:** where it goes on each OS (the app's log or cache directory, `~/.var/app/<id>` in the Flatpak), how big it can get, how many files are kept, and whether it is written all the time or only while troubleshooting.
> - **Raising the level:** before start, through an environment variable (e.g. `HOPLODEX_LOG=debug`; `flatpak run --env=`), a `machine.json` field, and/or a settings switch. Whether a raised level turns itself off after a while or a number of runs.
> - **Finding the log:** a "Show log file" or "Copy log" action, and from where (About, settings, the chooser).
> - **Debug lines:** the ones worth adding (open, close and lock, backups, import, preview, the portals), each held to the no-database-contents rule. Possibly a test that keeps that rule.
> - Whether the helpers (`--render-helper`, `--import-helper`) and the WebKit sandbox probe should log for themselves; today the parent logs for them.
>
> Raised during feature 011 (Flatpak), which keeps the stderr logger as it is.

The decisions recorded for it are in Clarifications above, with the owner's words.
