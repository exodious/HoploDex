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

Test names are as they stand in the files (Vitest titles in quotes, Rust tests
as `fn` names, E2E titles in quotes). A Vitest title may be shortened here at
`…` only.

| Scenario / criterion | Test |
|---|---|
| US1-1 thumbnail of the same photo id in another database | `e2e/specs/us14-session-isolation.e2e.ts` "shows Shared's own thumbnail, not Main's, after locking Main and opening Shared" (also "shows Main's own thumbnail again after switching back through the database menu", "keeps showing only Main's after restoring a backup of it"); `src/features/browse/FirearmThumbnail.test.tsx` "doesn't show scope A's thumbnail for photo 7 under scope B, which loads its own" and "never shows the other scope's cached image while its own loads, even if both are alive" |
| US1-2 a load finishing after the switch | `FirearmThumbnail.test.tsx` "drops a deferred response that arrives after the scope ended: not shown, not cached"; `PhotoGallery.test.tsx` and `DocumentList.test.tsx` "drops a list that resolves after the scope ended"; `sessionScope.test.ts` "rejects a response that resolves after end() and never returns its value" and "rejects a failure that arrives after end() as DATABASE_CLOSED too"; `usePendingDraft.test.tsx` "sends nothing from a debounced flush that fires after its scope ended" |
| US1-3 resumed draft never fills B's form | `usePendingDraft.test.tsx` "doesn't match a draft resumed in one scope to a form of the same kind, mode, version and target in another" and "opens only in the scope that resumed it when both forms are mounted"; `SessionProvider.test.tsx` "keeps the changes for the next open when the database locks while opening (US1-3)"; `pending_resume_test.rs` `a_lock_while_resuming_with_nothing_staged_leaves_the_row_and_the_next_open_starts_unresolved`, `a_user_lock_a_sleep_and_a_close_while_resuming_leave_the_row`, `opened_deletes_the_row_and_stages_the_draft`, `after_opened_a_lock_with_nothing_staged_by_a_form_keeps_the_changes` and `after_opened_a_sleep_or_shutdown_keeps_the_changes` |
| US1-4 form can't open → dialog again | `SessionProvider.test.tsx` "brings the dialog back with its error when the record fails to load (US1-4)", "offers Resume editing again after a failure, and sends resume again" and "mounts the collection afresh when the changes are discarded after a failed resume"; `FirearmRecordPage.test.tsx` "reports notOpened when the firearm fails to load while its draft is opening", `AccessoryRecordPage.test.tsx` "reports notOpened when the accessory fails to load while its draft is opening", `PolicyEditors.test.tsx` "reports notOpened when the policy isn't in the collection while its draft is opening"; `pending_resume_test.rs` `not_opened_returns_the_summary_for_the_dialog_and_blocks_again` |
| US1-5 batch of attachments interrupted | `PhotoGallery.test.tsx` "sends exactly one add of three dropped photos when the scope ends after the first"; `DocumentList.test.tsx` "stops a batch at the scope's end: of three dropped documents, exactly one add is sent"; `session_scope_test.rs` `with_colliding_ids_a_late_add_after_a_switch_writes_nothing_to_the_other_database`. "Keeps what was added" has no assertion of its own: the add that was sent is the backend's, committed to the session it named |
| US1-6 notifications | `SessionProvider.test.tsx` "removes every toast of the session when it ends, and keeps the chooser's notice", "shows nothing for a notify from the ended session, late", "removes the toasts as the closing screen replaces the collection", "renders no useToast consumer outside the tree keyed by the open (research.md §4)" and "renders no useToast consumer on the closing screen" |
| US1-7 any late request refused | `session_scope_test.rs`: every accessor with an ended id (`a_session_closed_is_refused`, `a_session_locked_is_refused`, `a_session_locked_with_a_draft_is_refused`, `the_same_file_reopened_is_a_new_session`, `a_second_database_opened_over_the_first_is_a_new_session`, `a_restore_of_the_open_database_is_a_new_session`, `a_session_taken_over_is_refused` (Unix), `every_id_is_refused_with_nothing_open`), a never-issued id (`zero_and_a_never_issued_id_are_refused_by_every_accessor`), and no header (`a_path_names_a_database_and_no_path_names_the_open_session_by_header`; `src-tauri/src/session/scoped.rs` unit tests `a_missing_header_is_refused`, `a_malformed_header_is_refused`, `a_zero_header_is_refused`); a late close or lock leaves the next session open (`a_late_close_or_lock_of_the_same_file_reopened_leaves_it_open`, `a_late_close_or_lock_after_a_switch_leaves_the_other_database_open`, `a_late_close_with_nothing_open_is_refused`, `a_close_or_lock_of_the_current_session_still_works`); the command lists (`unscoped_commands_are_all_commands`, `a_command_takes_the_state_session_only_if_unscoped`) |
| FR-001 the session header reaches the command on every IPC path | `research.md` §2 (the throwaway probe's results on Linux, macOS and Windows, T006 to T008; the probe spec was deleted at T132); `us4-photos-documents.e2e.ts` "leaves the IPC custom protocol working beside the document filter (#73)" (the Windows custom-protocol case); every `us14` test, which needs the header to get any answer |
| SC-001 | `us14-session-isolation.e2e.ts` and `session_scope_test.rs` `with_colliding_ids_a_late_read_after_a_switch_gets_nothing_of_the_other_database` and `with_colliding_ids_a_late_add_after_a_switch_writes_nothing_to_the_other_database` (two databases seeded with the same ids through `ops`, by `colliding_worlds`) |
| US2-1 impossible declared count | `import_reader_test.rs` `shared_strings_declared_as_u64_max` and `shared_strings_declared_one_over_the_limit`: refused for the shared-strings limit, the helper dead after (`no_helper_runs`), the app running|
| US2-2 too large, or expands too far | `import_reader_test.rs` `a_csv_one_byte_over_256_mib`, `a_csv_field_of_300_mb`, `a_zip_bomb`, `a_thousand_and_one_parts` |
| US2-3 rows, columns, sheets, cells, cell text | `import_reader_test.rs`, one test per limit (`a_workbook_of_100001_rows`, `a_csv_of_100001_rows`, `a_workbook_row_of_257_cells`, `a_csv_row_of_257_cells`, `seventeen_sheets`, `sixteen_sheets_past_the_cell_limit`, `a_workbook_cell_of_32768_characters`, `a_csv_cell_of_32768_characters`, `a_workbook_past_the_text_limit`, and the two-file cases), each checking the message names the file, the sheet and the limit; `every_case_of_the_corpus_is_run` keeps the list whole |
| US2-4 cancel | `import_reader_test.rs` `cancelling_stops_the_reading_within_a_second_wherever_it_is`, `an_import_already_stopped_starts_no_helper`; `lock_test.rs` `cancel_import_stops_only_an_import_that_is_reading`; `us5-export-import.e2e.ts` "stops reading a large file when asked, and imports nothing (008 US2, Stop reading)" |
| US2-5 lock while reading | `import_reader_test.rs` `a_session_that_ends_goes_ahead_at_once_and_the_reading_ends_within_a_second`, `a_stop_made_by_the_session_ending_is_never_the_users_stop`; `lock_test.rs` `a_screen_lock_during_the_reading_goes_ahead_and_the_import_ends_closed`, `a_sleep_during_the_reading_goes_ahead_and_the_import_ends_closed` and `a_quit_during_the_reading_goes_ahead_and_the_import_ends_closed` (each `DATABASE_CLOSED`, nothing imported) |
| US2-6, SC-003 the largest export imports | `import_largest_test.rs` `import_largest_export_as_a_workbook_and_as_two_csv_files` (release, `--ignored`): generated workbook and two CSVs |
| SC-002 | `import_reader_test.rs` over `support/hostile_spreadsheets.rs`'s corpus (including `a_csv_of_25_million_empty_cells` and `sparse_rows_padded_past_the_cell_limit`): refused within 5 s, parent's resident memory rise ≤ 512 MB, helper dead after |
| SC-004 | `import_reader_test.rs` `cancelling_stops_the_reading_within_a_second_wherever_it_is` and `a_session_that_ends_goes_ahead_at_once_and_the_reading_ends_within_a_second`; `us5-export-import.e2e.ts` "Stop reading" (1 s banner wait) |
| US3-1 to US3-3 menus (page side) | `src/features/app/browserControls.test.ts` "the context-menu decision" ("prevents it on a blank area, a row, a card, a button, an image and a dialog body", "allows text fields, text areas, editable regions and selects", "allows a click inside a non-empty selection, and not outside it") and "cancels a context menu except in editable text"; `us15-browser-controls.e2e.ts` "cancels the context menu on a blank area, a row, a card, a button and an image" and "cancels it on a dialog's body, and not on a text field, a text area or a selection" |
| US3-1 to US3-3 menus (native side), SC-005 | the window controls check, below, on each OS |
| US3-4 reload and navigation keys | `us15-browser-controls.e2e.ts` "keeps the window and the typed input through every reload and navigation key, and works the editing keys with real input" (Linux only, XTest: F5, Ctrl+R, Alt+←/→ and the mouse's back and forward); the window controls check on macOS and Windows; `browserControls.test.ts` "blocks Alt+←/→ on Windows and Linux only", "blocks ⌘[, ⌘] and ⌘←/→ outside editable text on macOS only", "treats buttons 3 and 4 (back, forward) as navigation", "cancels mouse buttons 3 and 4 on mousedown, mouseup and auxclick" and "moves by word on macOS: Option+←/→ with and without Shift, in text and elsewhere" |
| FR-011 look-ups without a menu (macOS) | `browserControls.test.ts` "prevents a force click on macOS only" and "blocks ⌃⌘D (Look Up) on macOS only"; the macOS window controls check; M1 steps 8 and 9 |
| US3-5 page shortcuts off, app shortcuts on | `browserControls.test.ts` "blocks the reload, print, find and developer-tools keys on …", "lets the app's keys and the editing keys through on …" (each platform), "lets Ctrl+L reach a later capture listener, as SessionProvider's" and "cancels Ctrl/⌘+F for the web view but lets the app's search shortcut see it"; `us15-browser-controls.e2e.ts` "focuses the search box with "/" and leaves the editing keys alone", "closes a dialog with Escape" and "locks with Ctrl/⌘+L from the open form" |
| US3-6 development build | `browserControls.test.ts` "is not installed when DEV"; `src-tauri/src/window_controls/mod.rs` unit tests `an_e2e_build_installs_the_filters` (`cargo test --profile e2e --features e2e window_controls`) and `a_plain_debug_build_does_not_install_the_filters` |
| US4-1, US4-6 wording, version | `PendingChangesDialog.test.tsx` "names the version that kept them when it is recorded and isn't this build's", "says a different build when the version number is this build's", "says another version when none is recorded", "keeps the record-gone sentence, and no version sentence, when the record is gone" and "still offers Resume editing for a form this build knows, whichever version kept it"; `last_saved_version_test.rs` |
| US4-2, US4-3, SC-006 close and resume with the right version | `us9-locking.e2e.ts` "pending changes from another version: close, then reopen" (seeded `--pending-from 1.2.0`) and "close from the dialog, reopen and resume" (the default seed, kept by this version: every field as kept); `pending_resume_test.rs` `a_close_leaves_the_row_and_its_backup_has_no_pending_changes` and `resume_after_a_close_returns_every_field_as_written`; `PendingChangesDialog.test.tsx` "closes the database, keeping the changes, for %s" |
| US4-4 discard asks first | `PendingChangesDialog.test.tsx` "asks before discarding, then discards" and "goes back to the question when discarding is cancelled" (existing, kept) |
| US4-5 Close offered in every case | `PendingChangesDialog.test.tsx` "is offered between Discard and Resume for %s" (a resumable change, changes of another version, a record that is gone), "focuses Resume editing when it is offered", "focuses Close the database, not Discard, for %s" and "stays available while Resume editing shows Opening…"; `SessionProvider.test.tsx` "offers Close the database with the other two in that dialog (US4, T095)" |
| FR-015 when the version is and isn't written | `last_saved_version_test.rs`: create (`a_new_database_records_this_version`, `creating_through_the_db_layer_records_it_too`), each write kind (`a_write_that_changed_rows_records_this_version`, `a_write_open_that_changed_rows_records_this_version`, `a_housekeeping_write_that_changed_rows_records_this_version`), `write_pending` (`a_lock_that_keeps_a_draft_records_this_version`, `the_idle_lock_that_keeps_a_staged_draft_records_this_version`, `the_immediate_close_at_sleep_or_shutdown_that_keeps_a_draft_records_this_version`), and no change on open, close, backup, restore bookkeeping, passphrase change, a write that changed nothing or a read (`opening_and_its_marker_leave_the_version_alone`, `closing_leaves_the_version_alone`, `a_backup_carries_the_version_and_the_original_keeps_it`, `a_restore_names_the_version_that_saved_the_backup_not_this_one`, `a_passphrase_change_leaves_the_version_alone`, `a_write_that_changed_nothing_leaves_the_version_alone`, `a_read_leaves_the_version_alone`) |
| Field maximums | `text_limits_test.rs` (`a_firearm_field_at_its_maximum_is_saved_and_one_more_character_is_refused`, the accessory and policy counterparts, `the_recipient_of_either_dispose_dialog_has_a_maximum`, `characters_are_counted_in_unicode_scalar_values_after_trimming`, `an_imported_cell_over_its_maximum_is_a_row_error_and_one_at_it_imports`); `src/lib/textLimits.test.ts`; `src/components/TextArea.test.tsx` (the "TextArea counter" tests) |
| Versions agree | `app_version_test.rs` `the_three_version_files_name_the_same_version` and `app_version_is_that_version` |
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
   Copy, Paste (and Spelling and Emoji & Symbols where offered, and macOS's
   own AutoFill item, which stays: FR-011's note). No Look Up,
   Translate, Search With Google, Share, Services, Speech, Writing Tools,
   Reload or Inspect Element.
6. Open **Glock 19 Gen5** and double-click the word `Glock` in the record's heading to select it.
7. With two fingers, click the selection. Expected: a menu with Copy (and
   AutoFill, as in step 5), and none of the items step 5 excludes.
8. Force-click (press hard on) the selected word. Expected: no Look Up
   panel.
9. Tap the selected word with three fingers (if the trackpad is set to
   look up with a three-finger tap), then press ⌃⌘D. Expected: no Look Up
   panel either time.
10. Click in the search field, type `glock lookup`, and press Option+← twice.
    Expected: the caret moves back one word each time.
11. On the trackpad, swipe two fingers left, then right. Expected: the page
    doesn't move back or forward.

### M2: Import a workbook saved by other programs

Covers workbooks the generated test files don't: a sheet saved by Excel
and one saved by LibreOffice Calc, within the limits.

1. Do Setup S1.
2. Open `.human-testing/import-samples/import-collection.xlsx` in
   LibreOffice Calc, choose **File > Save As**, keep the Excel 2007-365
   format, and save it as
   `.human-testing/import-samples/collection-libreoffice.xlsx`.
3. In HoploDex, press **Import** in the top bar, press **Choose…** and pick
   `collection-libreoffice.xlsx`, then press **Import** in the dialog.
   Expected: "Reading collection-libreoffice.xlsx…" shows briefly, then the
   import report matches what `import-collection.xlsx` gives on a fresh seed
   (`scripts/human-testing.sh --reset`).
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
4. The row says "Shared collection" is marked as open on "Workshop PC".
   Press **Take over…**, type the passphrase again and press **Take over**.
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
