# Data Model: Database Protection, Portability & Management

**Feature**: `003-database-protection-management` | **Date**: 2026-09-25 | **Plan**: [plan.md](./plan.md)

This feature adds no fields to firearms, photos, documents, policies or
disposition history. It adds three tables **inside each database file**
(travelling with it, FR-013), one **machine-local** settings file, one kind
of **keyring** entry, the **backup file** convention, and the in-memory
**session** that holds an open database. Following CLAUDE.md, the new tables
go into `db/migrations/0001_initial.sql` in place. An existing development
database is incompatible anyway: it is keyed by a random key, not a
passphrase, and is not converted (spec Assumptions).

Deltas to feature 001's data model: 001's `data-model.md` gains a one-line
"amended by 003" pointer at its storage note (the database lives wherever
the user chooses, not only in the app-data directory) and at its entity list.

---

## Inside the database

### `collection_settings` (collection data; a change makes a backup due)

Exactly one row. Created with defaults when the database is created.

| Column | Type | Default | Rules | Requirement |
|---|---|---|---|---|
| `id` | INTEGER PK | 1 | `CHECK (id = 1)` | |
| `backups_enabled` | INTEGER | 1 | `CHECK IN (0,1)` | FR-024 |
| `backup_keep_count` | INTEGER | 5 | `CHECK BETWEEN 1 AND 100` | FR-025 |
| `backup_location` | TEXT | `'default'` | `'default'`, or an absolute path (validated as absolute by `validate_backup_settings_input`). Changing it asks what to do with the backups at the old location first (research §22) | FR-026, research §7 |
| `idle_lock_enabled` | INTEGER | 1 | `CHECK IN (0,1)`; also decides the lock at sleep | FR-034, FR-037 |
| `idle_lock_minutes` | INTEGER | 10 | `CHECK BETWEEN 1 AND 240` | FR-034 |
| `lock_on_screen_lock` | INTEGER | 0 | `CHECK IN (0,1)` | FR-038 |

Changing any of these is a collection change (FR-025 lists backup and lock
settings), so the table carries the change-tracking triggers below.

### `app_state` (housekeeping; never makes a backup due)

Exactly one row. Written by the application on its own at create, open,
close, backup and restore.

| Column | Type | Rules | Meaning / requirement |
|---|---|---|---|
| `id` | INTEGER PK | `CHECK (id = 1)` | |
| `database_id` | TEXT NOT NULL | 32 lowercase hex digits, random at creation | Identity for backup names, rotation and the keyring entry (research §7, §10). Replaced when a backup is opened directly (research §9) |
| `created_at` | TEXT NOT NULL | UTC ISO-8601 | |
| `open_machine_id` | TEXT | 32 hex digits | Open marker: which machine (FR-032) |
| `open_machine_name` | TEXT | ≤ 255 chars | Open marker: name shown to the user |
| `open_since` | TEXT | UTC ISO-8601 | Open marker: since when |
| `changes_waiting` | INTEGER NOT NULL DEFAULT 0 | `CHECK IN (0,1)` | Backup record: changes not yet in a backup (FR-025) |
| `last_backup_at` | TEXT | UTC ISO-8601 | Backup record: most recent backup. The once-a-day rule compares its **local** date on the closing computer with today's, and holds only while one of today's backups is still at the backup location |
| `disk_encryption_note_dismissed` | INTEGER NOT NULL DEFAULT 0 | `CHECK IN (0,1)` | FR-008 |
| `backup_made_at` | TEXT | UTC ISO-8601; set only in backup copies | Backup stamp (research §9) |
| `backup_of_name` | TEXT | set only in backup copies | Database name for the "backup opened directly" notice |

`CHECK ((open_machine_id IS NULL) = (open_machine_name IS NULL) AND (open_machine_id IS NULL) = (open_since IS NULL))`:
the marker is set or cleared as a whole.

### `pending_changes` (housekeeping; never in a backup)

At most one row (FR-039: one form open for editing at a time).

| Column | Type | Rules |
|---|---|---|
| `id` | INTEGER PK | `CHECK (id = 1)` |
| `kind` | TEXT NOT NULL | `CHECK IN ('firearm','policy')` |
| `mode` | TEXT NOT NULL | `CHECK IN ('add','edit','dispose','restore','coverage')`; `coverage` only with `kind = 'firearm'` |
| `target_id` | INTEGER | NULL only for `mode = 'add'`. Not a foreign key: the record may have been deleted on another computer, in which case the draft can only be discarded |
| `label` | TEXT NOT NULL | ≤ 200 chars, e.g. "Glock 19 — edit", "New insurance policy" |
| `form_version` | INTEGER NOT NULL | the frontend form's draft version |
| `values_json` | TEXT NOT NULL | `CHECK (length(values_json) <= 1048576)`; opaque to the backend |
| `saved_at` | TEXT NOT NULL | UTC ISO-8601 |

### Change-tracking triggers

For each collection table `T` in `firearms`, `photos`,
`document_attachments`, `disposition_history`, `insurance_policies`,
`firearm_types` and `collection_settings`:

```sql
CREATE TRIGGER T_marks_backup_due_after_insert AFTER INSERT ON T
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;
-- …and the same AFTER UPDATE and AFTER DELETE
```

Housekeeping tables (no triggers): `app_state`, `pending_changes`,
`schema_migrations`, and the FTS5 table with its shadow tables. A guard test
(`backup_due_tracking_test.rs`) fails when a table in `sqlite_schema` is in
neither list (research §5).

### Pinned file format

Cipher settings are in research §1 and contracts/database-file.md. A database file is
exactly one file, `<name>.hoplodex`, with no sidecar files. SQLite's rollback
journal may exist next to it only while a write is in progress.

---

## Backup files (on disk, next to or away from the database)

| Aspect | Rule |
|---|---|
| Folder | `backup_location = 'default'` → `<database folder>/HoploDex backups/`; otherwise the absolute path stored |
| Name | `<database name> <YYYY-MM-DD HHMMSS> <id8>.hoplodex` (local time of the computer that made it; `<id8>` = first 8 hex digits of `database_id`) |
| In progress | `<final name>.partial`, renamed on completion; never listed |
| Content | a byte copy of the database made while it is idle, read through the open connection's own file handle (research.md §3), then stamped: open marker cleared, `pending_changes` emptied, `changes_waiting = 0` (its content is a backup), `backup_made_at` and `backup_of_name` set. Protected by the passphrase current at that moment (FR-023) |
| Listed as this database's | in the folder of the **current** `backup_location`, with a name that matches the pattern and carries this database's `<id8>`. Backups left at an earlier location are not listed, rotated or deleted with all backups; setting the location back to their folder makes them the database's again (FR-026) |
| Rotation | after a successful backup, the oldest beyond `backup_keep_count` are securely deleted (FR-025), except the backup a restore is using (FR-028). Moving backups never rotates, so a move into a folder that already holds some may leave more than `backup_keep_count` until the next backup (research §22) |
| Moved | to a new location only by the user's choice when changing it (FR-026): hard-linked on the same drive, otherwise copied to `<final name>.partial`, verified byte for byte, renamed, and the original securely deleted. A name already taken at the new folder is never overwritten: that backup stays behind (research §22) |

---

## Machine-local: `machine.json` (per OS account, never inside a database)

Location: the OS app config directory (`~/.config/com.hoplodex.app/machine.json`
on Linux). Written atomically. Plain JSON: it holds paths and names, never
collection data or secrets (research §11).

```jsonc
{
  "version": 1,
  "machineId": "9f1c…",            // 32 hex digits, random on first run (research §6)
  "recentDatabases": [             // most recent first (FR-012)
    {
      "path": "/home/u/Documents/HoploDex/My collection.hoplodex",
      "name": "My collection",     // file name without extension, refreshed at each open
      "lastOpenedAt": "2026-09-25T14:30:05Z",
      "databaseId": "3fa2c9d1…",   // cached at each open; null until first opened here
      "backupFolder": "/home/u/Documents/HoploDex/HoploDex backups", // resolved, cached at each open and
                                   //  when the location is saved (for restoring a damaged database)
      "passphraseSaved": true      // FR-017; the passphrase itself is only in the keyring
    }
  ],
  "unfinishedBackup": {            // research §7; null when none
    "databasePath": "…", "partialPath": "…", "startedAt": "…"
  },
  "unfinishedBackupMove": {        // research §22; null when none
    "databasePath": "…", "databaseId": "3fa2c9d1…",
    "fromFolder": "…", "partialPath": "…"   // partialPath: the copy in progress, or null
  },
  "notices": [                     // shown once in the chooser, then removed
    { "kind": "operationStopped", "databasePath": "…", "operation": "import" },
    { "kind": "operationStopped", "databasePath": "…", "operation": "moveBackups",
      "leftBehindCount": 3, "folder": "…" },
    { "kind": "backupsLeftBehind", "databasePath": "…", "folder": "…", "count": 3 },
    { "kind": "pendingChangesLost", "databasePath": "…" },
    { "kind": "backupFailed", "databasePath": "…", "reason": "locationUnavailable" }
  ]
}
```

Rules:
- An entry is **unavailable** when `path` does not exist when the chooser
  lists it (FR-012). It may be removed or re-located; re-locating replaces
  `path` and keeps the rest.
- Removing an entry also deletes its keyring entry when `passphraseSaved`
  (FR-018). The database file is never touched.
- The same database reached by two paths (a symlink, or a copy) gives two
  entries. They are identified by path, as the spec's edge case on same-named
  files requires.

---

## Keyring: saved passphrase (per database, per computer, opt-in)

| Field | Value |
|---|---|
| Service | `com.hoplodex.app` |
| User (account) | `passphrase:<database_id>` |
| Secret | the passphrase, NFC-normalized (research §1) |

It is created only after the FR-017 confirmation. It is updated on a
passphrase change here (FR-018), after a successful typed open when the saved
one failed (US5-5), and after a restore (research §8). It is deleted by
"forget saved passphrase" and when the recent entry is removed. The
pre-feature entry (`user = sqlcipher-key`) is never read, written or deleted
(research §10).

---

## In memory: the session (backend)

```text
Session (Tauri state) = Mutex<Option<OpenDatabase>>

OpenDatabase {
  conn: Connection            // exclusive lock held; SQLCipher holds the derived key
  path, name, database_id
  fingerprint: FileFingerprint  // identity + length + mtime after our last commit (research §6)
  interrupt: InterruptHandle
  staged_draft: Option<Draft>   // research §16; never written except by a lock or OS shutdown
  pending_unresolved: bool      // collection commands refused while true (FR-039)
  storage_lost: bool            // set when the file became unreachable; every later
                                //  write is refused with DATABASE_UNAVAILABLE (research §6)
  lock_settings                 // the file's lock settings, for the idle clock
}

Closing { normal: path of a close finishing on its own thread (its backup),
          immediate: an immediate close from its first step to its last }
  // from an immediate close's first step, every command is refused with
  // DATABASE_CLOSED; a sleep during a normal close hands it the rest

Operations registry: at most one running long operation
  { kind: backup | passphraseChange | restore | import | export | deleteBackups
          | moveBackups,
    cancel: AtomicBool, interrupt: Option<InterruptHandle>,
    done: how far it got (rows imported, backups deleted; for a move,
          the backups not yet moved),
    folder: for a move, the old location the rest are still in }

IdleClock { settings (None while nothing is open), last_input (wall clock),
            paused_by: set<nativeDialog> }   // a registered operation also pauses it
// Both are held by the Session.
```

The backend never holds a passphrase between commands (FR-007, research §1).

### Session states and transitions

```text
                 create / open (passphrase or saved)
  NoDatabase ─────────────────────────────────────────────▶ Open
      ▲  ▲                                                  │ │ │
      │  │ open refused: PASSPHRASE_INCORRECT, IN_USE,      │ │ │
      │  │ NEWER_VERSION, OPEN_ELSEWHERE (→ take over),     │ │ │
      │  │ DAMAGED (→ restore)                              │ │ │
      │  └──────────────────────────────────────────────────┘ │ │
      │                                                        │ │
      │   Closing(normal): close · switch · user quit ·        │ │
      │   lock now · idle · screen lock                        │ │
      │   [draft→pending only for locks] → backup if due       │ │
      │   (progress, skippable) → clear marker → close conn →  │ │
      │   delete document copies                               │ │
      ├───────────────────────────────────────────────────────┘ │
      │                                                          │
      │   Closing(immediate): sleep (idle lock on) · OS shutdown │
      │   stop operation → emit session:closed → save draft as   │
      │   pending (+ clear marker in the same write; the marker  │
      │   alone when no draft is staged) → close                 │
      │   conn → delete document copies → the stopped operation  │
      │   removes its partial files as it unwinds, and the close │
      │   waits for it to report how far it got                  │
      ├─────────────────────────────────────────────────────────┘
      │
      │   TakenOver: fingerprint mismatch before a write or at close
      │   → refuse the write → emit session:closed(takenOver) → close conn
      │   without writing anything more
      └───────────────────────────────────────────────────────────
```

- **Storage lost** is not a state of its own: the session stays **Open** with
  `storage_lost` set when the file cannot be reached before a write (research
  §6). Writes are refused with `DATABASE_UNAVAILABLE`, and the next
  Closing(normal) skips the backup (`failed`) and the marker clear and writes
  nothing.
- A **sleep during Closing(normal)** turns it into Closing(immediate) from
  that point, whatever the idle-lock setting: the running backup is stopped
  and its changes stay waiting (FR-037, research §14).

- Pending-changes resolution happens in **Open** before any collection
  command is accepted.
- A passphrase change and a restore pass through a brief internal close and
  reopen (research §4), and the user stays in the collection. A sleep during
  either one abandons it and becomes Closing(immediate) (FR-037).
- `NoDatabase` holds nothing from any collection: no connection, no draft
  and no cached rows. The frontend unmounts the whole collection tree on
  `session:closed` (contracts/ui-databases.md).

### Close reasons (`session:closed.reason`)

`closed`, `switched`, `quit`, `lockedByUser`, `idle`, `screenLocked`,
`sleep`, `shutdown`, `takenOver`. Each has its own sentence in the
chooser's message (contracts/ui-databases.md). `sleep` and `shutdown` never make a
backup (FR-027, FR-037). Since Lock is the in-app close (FR-010 as amended
2026-09-27, research §17), `closed` comes only from the "couldn't be shown"
panel, and `switched` only when `open_database` finds another database
still open.

---

## Validation rules (backend is authoritative)

| Input | Rule | Error |
|---|---|---|
| New passphrase | NFC-normalized; ≥ 12 Unicode scalar values; no NUL; equals confirmation (checked in the frontend; the backend checks length and NUL again) | `VALIDATION_ERROR`, `fieldErrors.passphrase` |
| Database name | 1–120 chars and at most 200 bytes of UTF-8 (the longest file made from it, a `.partial` backup, adds 44 bytes to a 255-byte file-name limit); no path separator or character invalid on any supported OS (`<>:"/\|?*`, control characters); not `.`/`..`; not ending in a space or dot | `VALIDATION_ERROR`, `fieldErrors.name` |
| Database folder | absolute; a writable folder, or a path that does not exist yet, which create makes (the suggested `<Documents>/HoploDex` usually does not exist on first run, US1, FR-009) | `VALIDATION_ERROR`, `fieldErrors.folder` |
| Create target | `<folder>/<name>.hoplodex` must not exist | `DATABASE_EXISTS` |
| `backup_keep_count` | 1–100 | `VALIDATION_ERROR` |
| `backup_location` | `default`, or an absolute path; a path that does not exist is accepted, so a location on another computer's drive can be kept, but is reported on save as currently unavailable | `VALIDATION_ERROR` |
| A changed `backup_location` | when the old folder holds backups of the database, `existingBackups` must say what to do with them; when the old folder cannot be read, only `leave` is accepted (research §22) | `BACKUPS_AT_OLD_LOCATION`, `OLD_BACKUP_LOCATION_UNAVAILABLE` |
| `existingBackups: "move"` | the new folder is available and has room for every backup to be moved, plus 5% | `BACKUP_LOCATION_UNAVAILABLE`, `INSUFFICIENT_SPACE` |
| `idle_lock_minutes` | 1–240 | `VALIDATION_ERROR` |
| Pending draft | `values_json` ≤ 1 MiB; `kind`/`mode` pair valid | `VALIDATION_ERROR` |
