# Frontend

_Part of the [architecture docs](../architecture.md)._

**Frontend** (`src/`): `features/<area>/` each has a `*Service.ts` (typed
wrappers over `invoke`, one per command), `types.ts` mirroring the Rust models,
and the components and CSS.

`features/app/` holds the shell, navigation, theme, and
`CollectionProvider`/`collectionStore` (shared collection state: policies, value
summary).

`AboutDialog` (010) is About HoploDex, rendered once by `SessionProvider` and
opened by `session.showAbout()` from the chooser's top bar, the database menu
and `app:about-requested`; it closes on any phase change except the window's
start, reads the build's `/notices.json` (the app's own origin, so no command
and no database access), and shows the program block from the
`__HOPLODEX_PROGRAM__` constants the Vite plugin defines. Two parts call
commands: its `TroubleshootingLogSection` (013, `troubleshootingLog.ts`) gets
the log's state and path, never its contents, and its `ProjectLink`s (#256)
call `open_project_page` with a page's name, never an address.

`features/session/sessionScope.ts`'s `SessionScope` is what one open owns: its
`id`, `invoke` (sends the `HoploDex-Session` header; after `end()` it rejects
with `DATABASE_CLOSED` without sending and drops a late response), the thumbnail
cache, the resumed draft until its form opens, and `onEnd` clean-ups.

Every `*Service.ts` function that reaches the open database takes the scope
first, and a component captures the scope it renders under, so work begun in one
session can't land in the next; only the unscoped commands use `tauriClient`'s
`invoke` directly.

`SessionProvider` creates a scope per open, ends it synchronously on
`session:closing`/`session:closed` for that `sessionId`, a lock, the next open
or the chooser, and holds `ToastProvider` (keyed by the open) so toasts end with
the session.

`features/app/browserControls.ts` cancels the page's reload, history, print,
save, find, view-source and developer-tools keys and its own context menu
outside editable text and selections, in production builds only (`main.tsx`);
`src/lib/textLimits.ts` and `TextArea`'s `limit` show the free-text maximums
with the backend as the authority; `src/lib/formFocus.ts` moves focus to the
first field a failed save refused (the form's own check or the backend's
`fieldErrors`), for every form.

Every form that edits something saved says which fields differ from what is
saved, and Save waits for one (014): `components/fieldChanges.ts`'s
`changedFields`/`formChanges` compare what a save would store for each field
(each form's `stored`), and that one `isDirty` feeds the form's Save, its
`useDirtyForm` (the quit question and the pending changes kept at a lock) and
the close question. A field component takes `change` (`FieldFrame` renders
`ChangedMark`, the "Changed" tag and its revert, the tag's id first in the
control's `aria-describedby`); `Button`'s `unavailable` is a Save that looks
disabled but stays focusable, its reason as tooltip and description; and
`Dialog`'s close guard (`components/closeGuard.ts`, `useCloseGuard`) asks
"Discard changes?" before Escape, the close button or the form's Cancel drops
unsaved input (`ConfirmDialog`'s `unsavedChanges` for Restore). Mount changes
to an existing record are made only in the firearm and accessory edit forms'
Mounted list (`features/mounts/MountedEditor.tsx`, `mountedChanges.ts`), sent
as `update_*`'s `mountChanges` and saved with the record; the record page's
Mounted section is read-only apart from Mount → New accessory….

`features/session/SessionProvider` shows the database chooser whenever nothing
is open and renders the collection's providers only while a database is, keyed
by that open, so a close or switch drops all collection state; it also handles
the unsaved-changes question, pending changes after a lock, Ctrl/⌘+L and idle
activity.

`features/databases/` holds the chooser, create dialog, database menu and its
dialogs (settings, change passphrase, restore, the guide); its `plate/` is the
chooser's animated catalogue plate, whose timing lives in `plate/timing.ts` and
is tuned with `npm run tuner` (`tools/plate-tuner/`, see
`docs/development/plate-and-icon.md`).

The program icon is generated from `tools/icons/AppIcon.tsx` with `npm run
icons`.

The frontend reads the firearm types (and which fields each has) and the
registration classes from the backend, with no hard-coded list;
`IdentificationGuide` is the "How do I record this?" guide (it was
`OriginGuide`).

Shared primitives, built on Radix, live in `src/components/` and are exported
from `index.ts` (`Menu.tsx` also has radio items, used by the collection's
grouping menu); `Combobox.tsx` is the shared suggestion list.

A dialog is centred, so one that reads data after it opens grows and moves
(#116): it opens through `useShownOnceLoaded` (its result is the dialog's
`open`) once the data is in, as `ExportDialog`, `PolicyDeleteDialog` and
`RestoreBackupDialog` do.

`features/accessories/` holds the Accessories page, `AccessoryForm` and
`AccessoryRecordPage`; `features/mounts/` is shared by both record pages and
both dispose and delete dialogs: `RecordName` (a record's name, as a link or
text), `MountChooser` (the "Mounted on" combobox),
`MountedList`/`MountedSection` (the flat depth-first Mounted list and its Mount
menu), `MountedOnChain` and `DeleteMountedNote`.

`features/media/` holds photos and documents: `DocumentList`, `DocumentPreview`
(the viewer, a dialog over the record), `PreviewSurface` (the page area the
backend lays the PDF's child web view over, reporting its bounds),
`mediaService.ts` and, for the per-computer "Open documents" setting,
`documentOpening.ts`.

`features/browse/` is shared by the collection and Accessories pages:
`GroupByMenu`, `searchHooks.ts` (the search debounce and the "/" shortcut),
`MountLines` and `GroupedTable` (#51), the one list layout: a single table for
every group, so the columns line up, whose heading row and current group's title
row stick under the top bar; a column the narrow window drops is named
(`narrowHidden`), not counted. Its rows are memoised, so a page keeps its
`columns` and `onRowClick` stable (`useMemo`/`useCallback`) and its row objects
the same across batches; otherwise every 150-row batch renders every mounted row
again (`renderLimit.ts`'s `useRenderLimit` resets the limit in the render that
sees a new search or grouping, not in an effect).

Design tokens are in `src/styles/tokens.css`. Money values are **whole dollars**
(integers), not cents; see `src/lib/money.ts`.

## File drops

**File drops** arrive as filesystem paths through Tauri's native drag-drop event
(`listenForFileDrops`). They go to the `add_*_from_path` commands, which read
only a path the main web view's `DragDrop` event recorded in Rust for the open
session (`session::grants`, from `drops::register`'s `on_webview_event`; the
main web view is a child of its window, so the window's own `DragDrop` never
fires): at most two minutes after the drop, and once. Any other path the page sends is refused with
a fixed `VALIDATION_ERROR` before the file is looked at, so the page can't
name a file the user didn't drop. A drag that starts in the app itself (out of
the page, or out of the PDF surface) is not a drop of the user's files, and
`drops::refuse_app_drags` keeps it from reaching the recorder with a path:
on Linux by the drag's source widget being in the process, on macOS by its
`draggingSource`, on Windows by Chromium's mark on a page's drag data (only a
data object that answers `S_OK` has it: File Explorer's answers `S_FALSE` for
a format it lacks, #207). The
[drop check](../development/surface-checks.md#drop-check) drags a file from another app and links out
of the page with real input, in a build without the `e2e` feature, and on
Windows (from File Explorer) and macOS drops a photo and a PDF onto the app's
own record page. `read_attachment_file` judges the opened file's size
before reading any of it, its type before the rest (a document's from its name
and first bytes, `document_types::classify_header`; a photo's from its first
bytes alone, `photo::photo_type`), and reads no more than the size it admitted.

A file from the picker is read in the web view, so the panel refuses one over
its limit from `File.size` before reading it, and the backend checks the bytes
it receives again.

**The import's files, the export's folder, a new database's folder, the
backup location and the upgrade's backup folder** are picked in a chooser the
backend opens (`choose_import_files`, `choose_export_folder`,
`choose_database_folder`, `choose_backup_folder`, `choose_upgrade_backup_folder`,
`services::chooser`), never typed: the page gets a token for each answer and
shows its path (`ChosenPathField` in the import, export and create dialogs),
and the commands take the tokens. One that finishes uses them up; one that
fails leaves them, so the dialog can try again (the upgrade picks again). The
settings dialog sends `current` for the saved location. **Open another database
file…** and **Locate…** use `choose_database_file` and send the picked path
(`open_database`, `locate_database`). Don't add a path field back, or a command
that takes a file or folder the page names. An E2E build answers the chooser
from a queue (`answerChooser` in `e2e/support/ui.ts`) and hands the
`tauri://drag-drop` events the specs send to `drops::record`, the recorder a
real drop goes through.

WebKitGTK gives HTML5 drops no `File`, so leave `dragDropEnabled` at its default
and don't switch to HTML5 file drops.
