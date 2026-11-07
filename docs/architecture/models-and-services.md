# Models and services

_Part of the [architecture docs](../architecture.md)._

## `models/`

`models/` holds the serde types (camelCase over IPC), input validation
(`validate_*_input`) and enums.

`models/record.rs` has `RecordRef` (firearm or accessory by id) and
`RecordLabel`, the one shape for "a record" in a mount, a list or a link;
photos, documents and disposition history take an `owner: RecordRef` rather than
a firearm id.

`models/rules.rs` holds the amount, date, disposition and insurance rules a
firearm and an accessory share, so the two can't judge them differently, and the
free-text maximums (`MAX_SHORT_TEXT_CHARS` 200, `MAX_LONG_TEXT_CHARS` 4,000,
`check_text_length`; the entry fields keep `MAX_ENTRY_CHARS` 100) that save and
import both check, mirrored in `src/lib/textLimits.ts`, and the most photos and
documents one record may have (`MAX_PHOTOS_PER_RECORD`,
`MAX_DOCUMENTS_PER_RECORD`, 50 each; `ATTACHMENT_LIMIT_REACHED`) mirrored in
`src/features/media/attachmentLimits.ts`, and the most bytes (`MAX_PHOTO_BYTES`
15 MiB and `MAX_DOCUMENT_BYTES` 30 MiB a file, the two mirrored there too, and
`MAX_ATTACHMENT_BYTES_PER_RECORD` 1 GiB for a record's photos and documents
together, which only the backend holds; `ATTACHMENT_TOO_LARGE`).

## `services/`

`services/` holds pure/domain logic: spreadsheet import/export
(`FIREARM_COLUMNS` and `ACCESSORY_COLUMNS` are the canonical column lists, one
per table; import recognises each sheet's table by its header and reads columns
by header, so a sheet exported before a column existed still imports, and one
file or two can carry both tables; import passes a row's `record_id` to
`create_firearm` and `create_accessory`, whose `uid` argument is `None`
everywhere else, so the identifier and `mounted_on` survive), import matching,
insurance status, valuation, attachment storage, secure delete,

- and for 003 `passphrase` (NFC-normalized, zeroized) and `passphrase_dialog`
  (amended 2026-11-04 to 003 FR-007: the one place a passphrase is typed, a
  dialog of the OS's own, modal to the main window, so no passphrase ever
  reaches the web view. `ask` drives a `PassphraseDialog`: `NativeDialog`
  (`native/gtk.rs`, `native/appkit.rs`, `native/win32.rs`; GTK and AppKit on the
  main thread through `run_on_main_thread`, Win32 on a thread of its own with
  its own message loop), the E2E build's `e2e::E2eDialog`, which answers from
  `HOPLODEX_E2E_PASSPHRASES`' queue unless
  `HOPLODEX_E2E_PASSPHRASE_DIALOG=native`, or a test's. The dialog stays up,
  busy, while `ask`'s step tries what was typed, and shows a `Retry` under its
  field. `native::Controller` is what every OS's dialog shows, from `checks`
  (the rules as typed) and `strength` (the `zxcvbn` crate's estimate, which
  replaced the page's strength worker); the idle lock waits while one is up),
- `machine_settings` (`machine.json`),
- `keyring` (saved passphrases: every `Entry` is keyring-core's; outside a
  Flatpak the `keyring` crate's OS store, and inside one (011)
  `keyring/portal.rs`'s `Store`, a keyring-core store over oo7's file keyring at
  `$XDG_DATA_HOME/keyrings/default.keyring` keyed by the Secret portal's per-app
  secret, chosen by `Keyring::for_reach(&Reach)`; never the Secret Service and
  never `oo7::Keyring::new()`, and available only when
  `org.freedesktop.portal.Desktop` has an owner or can be started and its Secret
  interface's `version` reads, never by calling `RetrieveSecret`; its
  `SecretPortal` seam gives tests a fixed secret),
- `backups` (`BackupKind`: a `Regular` backup follows the settings;
  `BeforeUpgrade` is always made, whatever they say, is never the day's backup,
  and its file name ends ` before <version>` after the id),
- `file_swap` (atomic file replacement; `finish_interrupted` runs before every
  open and `remove_leftovers` once the passphrase has opened it),
- `scratch` (scratch files are created exclusively, removed only by what created
  them, and recovered or cleaned only when `is_plain_file`: #63) and
  `disk_space`;
- and for 011 `sandbox` (`Reach`, found once at startup and held by `Session`
  (`Session::reach()`, and managed state for the commands that take no session):
  `Unconfined` outside a Flatpak, where every path is within reach and nothing
  is checked; `Reach::Flatpak` holds the grants `/.flatpak-info` lists, the
  roots an override removed (a removal beats an equally close grant), the user's
  Documents folder (`documents`, read from the host's `user-dirs.dirs`; none
  when it is unset or is the home folder, since a Flatpak can't be granted a
  home) and a `HostPaths` that turns a Documents-portal path into the host path
  it stands for. The shipped grants are `xdg-documents/HoploDex`, `/media` and
  `/run/media`, each writable, and no home folder; a suggested location that has
  no grantable Documents is refused. `reach.check(path, Need)` takes one of six
  `Need`s: `DatabaseFolder` (the database file's folder must be within a
  writable grant), `WriteFolder`, `BackupFolder` (like `WriteFolder`, but a
  Documents-portal path outside reach is refused too: backups keep writing
  there), `ReadFile`, and `AttachPhoto` and `AttachDocument` (`ReadFile`, whose
  refusal says to attach the file with its button, since an out-of-reach host
  path can only be a drop). Every command that names a location asks it before
  writing, `backups::check_location` (which takes the `&Reach`) included;
  `update_backup_settings` asks only when the custom location changes, and
  `check_backup` asks `ReadFile` only. It never writes a probe file);
- `AppOpener` (007) takes an `OpenerRoute`, `XdgOpen` or, inside a Flatpak,
  `Portal { timeout }`, which asks the OpenURI portal and treats no answer
  within the timeout as opened;
- and for 004 `entry_text` (the entry key and 004 FR-015 rules), `cartridges`
  (the embedded catalog and caliber derivation) and `suggestions` (ranking,
  snapping, the import sheet pass);
- for 005 `registration` (the built-in form names, used only as suggestions);
- and for 006 `mounts` (`MountGraph`, a scan of the `mounts` table that every
  mount rule goes through: no cycles, one host per record, the descendants of a
  record; `atomically` runs a multi-write change in one savepoint) and
  `record_id` (the random UUID that names a record across export and import;
  insurance policies, photos and documents carry a `uid` too since 009, in the
  file only: no IPC type, export or import has it);
- and for 007 `document_types` (the one allowlist: `classify` checks a file's
  content at attach, preview and open), `consent` (`Consent`, `needs_consent`,
  the native confirmation's text before a document goes to another app) and
  `preview/` ([Document preview](preview.md));
- and for 008 `helper/` (the confinement, Windows job object and AppContainer, framing and
  spawn/kill plumbing that `--render-helper` and `--import-helper` share; the
  memory cap is a parameter) and `import_reader/` (the import reads CSV and XLSX
  in `--import-helper`, never in the app: `reader.rs` is the child's side,
  `client.rs` the app's, which feeds the file in chunks and turns the rows into
  import rows as they arrive; `limits.rs` holds the file, unpacked-size, entry,
  sheet, row, column, cell, text and shared-string maximums, measured by
  inflating, and a breach is `IMPORT_LIMIT_EXCEEDED` before any row is saved; it
  also holds Excel's own limits on a workbook's styles (cell formats, number
  formats, fonts, fills and borders, which `calamine` keeps every one of, and
  the length of a number format's code or a font's name, which it copies into
  every cell format) and on its theme's colour slots, and a file past them is
  unreadable).

The read happens outside the session mutex and ends at once, with the child
killed, when the session ends (`DATABASE_CLOSED`) or `cancel_import` is called
(`OPERATION_STOPPED`, only for the user's cancel); saving rows then runs under
the lock as before.

`commands/entries.rs` holds `suggest_entries`, `settle_entry`,
`list_action_types`, `list_firearm_types` and `list_registration_classes`.

For 006, `commands/accessories.rs` holds the accessory commands (an accessory is
its own table, `accessories`, with the same create, update, dispose, reverse and
delete shape as a firearm's), and `commands/mounts.rs` holds `mount_record`,
`list_mount_candidates` and `ops::dispose_with_mounted`, which both `dispose_*`
ops call to dispose of a record and the mounted records chosen with it in one
step.
