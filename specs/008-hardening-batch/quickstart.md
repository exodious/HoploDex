# Quickstart: validating feature 008

How to show that each story works. Most of it is automated, and the
commands follow DEVELOPMENT.md: on a host with podman, run them through
`scripts/dev-container.sh` (CLAUDE.md "Run tests in the dev container"). Run
`npm run build` before any E2E or screenshot run.

## Automated

```bash
scripts/dev-container.sh bash -c 'cd src-tauri && cargo test'          # Rust, real SQLCipher, real helper binary
scripts/dev-container.sh npm test                                       # Vitest
scripts/dev-container.sh bash -c 'npm run build && npm run test:e2e'    # WebdriverIO, sandboxed
scripts/dev-container.sh bash -c 'cd src-tauri && cargo test --release -- --ignored import_largest'   # SC-003
```

### Where each scenario and criterion is tested

| Scenario / criterion | Test |
|---|---|
| US1-1 thumbnail of the same photo id in another database | `e2e/specs/us14-session-isolation.e2e.ts` "shows B's own thumbnail after locking A"; `FirearmThumbnail.test.tsx` "two scopes with colliding photo ids" |
| US1-2 a load finishing after the switch | `FirearmThumbnail.test.tsx`, `PhotoGallery.test.tsx` and `DocumentList.test.tsx` "a deferred response after the scope ended is dropped"; `sessionScope.test.ts` |
| US1-3 resumed draft never fills B's form | `usePendingDraft.test.tsx` "a draft resumed in one scope is not matched in another"; `SessionProvider.test.tsx` "lock while opening keeps the changes for the next open"; `pending_resume_test.rs` "row kept across a lock while resuming" |
| US1-4 form can't open → dialog again | `SessionProvider.test.tsx` "a failed record load brings the dialog back with its error"; `FirearmRecordPage.test.tsx`, `AccessoryRecordPage.test.tsx`, `PolicyEditors.test.tsx` "report notOpened"; `pending_resume_test.rs` "notOpened blocks the collection again" |
| US1-5 batch of attachments interrupted | `PhotoGallery.test.tsx` and `DocumentList.test.tsx` "stops at the scope's end, keeps what was added"; `session_scope_test.rs` "an add with an ended id writes nothing to the next database" |
| US1-6 notifications | `SessionProvider.test.tsx` "toasts go with the session; a late notify shows nothing; the chooser notice stays" |
| US1-7 any late request refused | `session_scope_test.rs`: every accessor with an ended id, a never-issued id and no header; the command lists |
| SC-001 | `us14-session-isolation.e2e.ts` and `session_scope_test.rs` "colliding ids" (two databases seeded with the same ids through `ops`) |
| US2-1 impossible declared count | `import_reader_test.rs` "uniqueCount beyond the limit is refused as unreadable, the helper ends, the app doesn't" |
| US2-2 too large, or expands too far | `import_reader_test.rs` "file over 256 MiB", "1000× zip bomb" |
| US2-3 rows, columns, sheets, cell text | `import_reader_test.rs`, one test per limit, checking the message names the file, the sheet and the limit |
| US2-4 cancel | `import_reader_test.rs` "cancel_import stops the reading within 1 s"; `us5-export-import.e2e.ts` "Stop reading" |
| US2-5 lock while reading | `import_reader_test.rs` "a lock goes ahead at once and the reading ends within 1 s"; `lock_test.rs` "screen lock and sleep during reading" |
| US2-6, SC-003 the largest export imports | `import_largest_test.rs` (release, `--ignored`): generated workbook and two CSVs |
| SC-002 | `import_reader_test.rs` over `support/hostile_spreadsheets.rs`'s corpus: refused within 5 s, parent's resident memory rise ≤ 512 MB, helper dead after |
| SC-004 | the two 1 s tests above |
| US3-1 to US3-3 menus (page side) | `browserControls.test.ts`; `us15-browser-controls.e2e.ts` "contextmenu is prevented on blank, row, card, button, image; allowed on fields and selections" |
| US3-1 to US3-3 menus (native side), SC-005 | the window controls check, below, on each OS |
| US3-4 reload and navigation keys | `us15-browser-controls.e2e.ts` "real F5, Ctrl+R, Alt+Left and mouse back keep typed input" (Linux, XTest); the window controls check on macOS and Windows |
| US3-5 page shortcuts off, app shortcuts on | `browserControls.test.ts`; `us15-browser-controls.e2e.ts` "/, Ctrl+L, Escape and editing keys still work" |
| US3-6 development build | `browserControls.test.ts` "not installed when DEV"; `window_controls` unit test "filters compiled for release and e2e only" |
| US4-1, US4-6 wording, version | `PendingChangesDialog.test.tsx`, one test per wording; `last_saved_version_test.rs` |
| US4-2, US4-3, SC-006 close and resume with the right version | `us9-locking.e2e.ts` "pending changes from another version: close, then reopen" (seeded `--pending-from 1.2.0`); `pending_resume_test.rs` "close leaves the row; backup has no pending changes" |
| US4-4 discard asks first | `PendingChangesDialog.test.tsx` (existing, kept) |
| US4-5 Close offered in every case | `PendingChangesDialog.test.tsx` "Close the database in all three cases" |
| FR-015 when the version is and isn't written | `last_saved_version_test.rs`: create, each write kind, `write_pending` (lock and immediate), and no change on open, close, backup, restore bookkeeping, passphrase change, a write that changed nothing |
| Field maximums | `text_limits_test.rs` (each field, each record kind, import row errors); `textLimits.test.ts`; `TextArea.test.tsx` "counter" |
| Versions agree | `app_version_test.rs` |
| Screens | `npm run screenshots`: contracts/ui.md §5 |

### The window controls check (SC-005, every OS)

`src-tauri/examples/window_controls_check.rs` opens the main window with the
app's own `window_controls` code and a test page. It drives the window with
real input, and fails on any menu item outside FR-011's list, a menu where
none belongs, or a reload or navigation. Run it on every OS before the pull
request, and whenever the web view engine is updated:

- **Linux**: `scripts/dev-container.sh scripts/window-controls-check.sh`
- **macOS**: `scripts/macos/window-controls-check.sh`, from a Mac with the
  tart VM set up, as the PDF surface check runs.
- **Windows**: `powershell -ExecutionPolicy Bypass -File
  scripts\windows\window-controls-check.ps1`, in the signed-in desktop
  session.

Logs go to `e2e/screenshots-out/window-controls/`. Exit codes: 0 passed, 1 no
window, 3 usage, 4 a check failed.

## Manual checks (best effort before a release, not merge gates)

### Setup S1 (Linux or macOS)

1. From the checkout, run `scripts/human-testing.sh --reset --release`. It
   seeds `.human-testing/`. With `--release` it launches the E2E-profile
   build, which has the browser controls, instead of `tauri dev`.
2. In the chooser, select **Main collection**.
3. Type the passphrase `human testing passphrase` and press **Open**.

### M1: Right-click menus with a trackpad (macOS)

Covers what the window controls check can't send: a trackpad's two-finger
click and force click.

1. Do Setup S1 on a Mac (or in the macOS 26 VM with a trackpad passed
   through).
2. With two fingers, click a blank area of the collection list. Expected:
   no menu.
3. With two fingers, click the row **Glock 19 Gen5**. Expected: no menu.
4. Click the search field and type `glock`.
5. With two fingers, click the search field. Expected: a menu with Cut,
   Copy, Paste (and Spelling and Emoji & Symbols where offered). No Look Up,
   Translate, Search With Google, Share, Services, Speech, Writing Tools,
   Reload or Inspect Element.
6. Open **Glock 19 Gen5** and double-click the word `Glock` in the record's heading to select it.
7. With two fingers, click the selection. Expected: a menu with Copy, and
   none of the items step 5 excludes.
8. Force-click (press hard on) the selected word. Expected: no Look Up
   panel.
9. On the trackpad, swipe two fingers left, then right. Expected: the page
   doesn't move back or forward.

### M2: Import a workbook saved by other programs

Covers workbooks the generated test files don't: a sheet saved by Excel
and one saved by LibreOffice Calc, within the limits.

1. Do Setup S1.
2. Open `.human-testing/import-samples/import-collection.xlsx` in
   LibreOffice Calc, choose **File > Save As**, keep the Excel 2007-365
   format, and save it as
   `.human-testing/import-samples/collection-libreoffice.xlsx`.
3. In HoploDex, choose **File > Import**, pick `collection-libreoffice.xlsx`,
   and press **Import**. Expected: "Reading collection-libreoffice.xlsx…"
   shows briefly, then the import report matches what
   `import-collection.xlsx` gives.
4. If Excel is available, repeat steps 2 and 3 saving from Excel as
   `collection-excel.xlsx`. Expected: the same result.
5. In LibreOffice Calc, add 20 sheets to `collection-libreoffice.xlsx`, type
   `x` in cell A1 of each, and save it.
6. Import it as in step 3. Expected: refused, with a message naming
   `collection-libreoffice.xlsx` and saying HoploDex imports at most 16
   sheets. Nothing is imported.

### M3: Pending changes from another version, by hand

Covers the dialog's wording and the round trip, on the seeded **Shared
collection**.

1. From the checkout, run `scripts/human-testing.sh --reset --pending-from 1.2.0`.
2. In the chooser, select **Shared collection**.
3. Type the passphrase `human testing passphrase` and press **Open**.
4. If asked to take over from "Workshop PC", press **Take over**.
   Expected: the dialog "Unsaved changes to Beretta 92FS — edit", saying
   "They were kept by HoploDex 1.2.0", with **Discard changes** and **Close
   the database** and no **Resume editing**.
5. Press **Close the database**. Expected: the closing screen, then the
   chooser with **Shared collection** selected.
6. Select **Shared collection**, type `human testing passphrase`, and press
   **Open**. Expected: the same dialog as in step 4. The changes are still
   kept.
7. Press **Discard changes**. Expected: the confirmation "Discard the changes
   to Beretta 92FS — edit?".
8. Press **Discard changes** in the confirmation. Expected: the collection
   of **Shared collection**.
