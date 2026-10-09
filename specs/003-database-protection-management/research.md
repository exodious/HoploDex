# Research: Database Protection, Portability & Management

**Feature**: `003-database-protection-management` | **Date**: 2026-09-25 | **Plan**: [plan.md](./plan.md)

Each section resolves one open question from the plan's Technical Context or
one of the spec's "Follow-ups handed to the plan". Where a decision rests on
how SQLCipher actually behaves, it was checked with a throwaway spike against
the app's own bundled SQLCipher (4.14.0 community, via `rusqlite` 0.40
`bundled-sqlcipher`), run in the dev container on 2026-09-25. The spike is not
kept; its findings are quoted where they are used.

---

## §1 Passphrase to key: SQLCipher's own key derivation, with pinned settings

**Decision**: The passphrase is the SQLCipher key (`PRAGMA key = '<passphrase>'`),
so SQLCipher derives the page key with PBKDF2 from the passphrase and the
random 16-byte salt it keeps in the file's first page. Every connection the
application opens or attaches sets the cipher settings explicitly, straight
after the key and before the first read:

| Setting | Value | Why pinned |
|---|---|---|
| `cipher_page_size` | 4096 | SQLCipher 4 default, stated so a build default can never differ |
| `kdf_iter` | 1,000,000 | Raised from the default 256,000 (see below) |
| `cipher_kdf_algorithm` | `PBKDF2_HMAC_SHA512` | SQLCipher 4 default, stated |
| `cipher_hmac_algorithm` | `HMAC_SHA512` | SQLCipher 4 default, stated |
| `cipher_plaintext_header_size` | 0 | Whole file encrypted, salt in the clear as usual |

The settings live in one `CipherSettings` constant in `db/cipher.rs`, and a
single helper, `apply_cipher_settings(conn, schema)`, applies them to `main`
or an attached schema. No code path opens a database without it (FR-011).

**Work factor**: the spike measured opening an existing database with the
correct passphrase at **86 ms for 256,000 iterations and 336 ms for
1,000,000** (container on the development machine). A modest older laptop at
about half the single-thread speed would take about 0.7 s. That fits
SC-003, which holds opening to constitution IV's budget for interactive
actions (feedback within 100 ms, done within 1 s at 10,000 firearms): the
rest of the open (schema check, migrations check, marker write) reads and
writes a few pages and does not grow with the collection. It is also the
cost of each wrong attempt, which is the point: it multiplies an offline
guesser's cost fourfold over the default. Going higher was rejected because
it would leave too little of the 1 s budget on slower hardware, and because
passphrase change and restore each derive the key twice more (open the copy,
verify it), and those budgets are already dominated by file size. The open
is measured in `performance_test.rs`; the frontend shows the busy state at
once, since key derivation alone is over 100 ms
(contracts/ui-databases.md §1).

**Passphrase normalization**: the passphrase is normalized to Unicode NFC
before it is used as a key, and after that, only its length is checked:
at least 12 Unicode scalar values, and no NUL character (it cannot pass
through SQLite's C string API). It is never trimmed, because spaces are
significant. The same string typed on macOS (whose input methods can produce
decomposed forms) and on Windows then gives the same key, which FR-011
requires. This adds `unicode-normalization` (MIT/Apache-2.0, no transitive
dependencies, pure Rust).

**Passing the passphrase to SQLCipher**: `PRAGMA key` cannot take a bound
parameter, so the value goes through `rusqlite`'s `pragma_update`, which
renders it as a correctly quoted SQL string literal (the spike used a
passphrase containing `'`). `ATTACH … KEY ?2` does take a bound parameter,
and the passphrase change uses that form. A passphrase that happens to be
written exactly in SQLCipher's raw-key syntax (`x'` + 64 or 96 hex digits + `'`)
would be read as a raw key. That is deterministic on every platform, so it
does not break portability, and it is too unlikely to be worth a rule that
would contradict FR-003.

**Memory (FR-007)**: every passphrase the backend receives is moved at once
into `zeroize::Zeroizing<String>` (zeroize 1.9 is already in the dependency
tree; it becomes a direct dependency) and dropped as soon as the open, create,
change or keyring write that needed it returns. The backend never keeps a
passphrase for the rest of the session: backups do not need it (§3). SQLCipher
keeps the derived key for as long as the connection is open, and zeroes it
when the connection closes, so closing the connection is what "the key is
cleared from memory" (FR-033) means. On the frontend a JavaScript string
cannot be wiped. Passphrase inputs are uncontrolled `<input type="password">`
fields that are read once on submit and then reset, and are never kept in
React state or any store. That is as far as FR-007 can be met in a webview,
and the plan says so.

**SQLCipher logging**: SQLCipher writes `ERROR CORE … hmac check failed` to
stderr on every wrong passphrase (seen in the spike). It contains no key
material, but it is noise in a user's terminal. `PRAGMA cipher_log_level =
NONE` is set once per process in release builds, and left at the default in
debug builds.

**Alternatives considered**:
- *Derive the key in Rust (PBKDF2 over the file's salt) and open with a raw
  keyspec.* Rejected: it duplicates SQLCipher's KDF in our code (custom
  cryptography the Source Request rules out), and it is only useful for
  holding a raw key between operations, which §3 makes unnecessary.
- *A passphrase-wrapped random key (Argon2 + key wrap).* Already rejected in
  the spec's Source Request: it needs a second file, or a custom container.
- *Relying on SQLCipher's compiled-in defaults.* Rejected by FR-011: a future
  SQLCipher major version, or a differently configured build, would silently
  write a file the other platforms cannot read.

---

## §1a Checking a typed passphrase while the database is open

Changing the passphrase (FR-015) and saving it in the keyring from the
settings (FR-017) both need the **current** passphrase checked while the
database is open. The backend does not hold it (§1), and the open connection's
exclusive lock (§2) stops a second connection from opening the file.

**Decision**: `db::verify_passphrase(conn, candidate)` copies the file's
first page, read through the open connection's own file handle (§3,
"Reading the open file"), (4096 bytes of ciphertext: the salt plus page 1) into a temporary
file in the app cache directory, opens that with the candidate and the pinned
settings, and reads a value that lives only on page 1 (`PRAGMA
schema_version`). SQLCipher checks page 1's HMAC before anything else, so the
candidate is right if and only if that read does not fail with
`SQLITE_NOTADB`. The probe file is then deleted. It is ciphertext only, and
nothing decrypted touches the disk. A spike on 2026-09-25 against a 2.5 MB
database held in exclusive mode confirmed it. The right passphrase got past
the HMAC check (a full schema read then reported the expected `CORRUPT`,
because only page 1 was present), the wrong one gave `NOTADB`, and the main
file was byte-for-byte unchanged. It costs one key derivation, about 0.34 s.

**Alternatives considered**: keeping the passphrase for the session
(rejected: FR-007); closing and reopening with the candidate (rejected: a
wrong candidate would leave the session unable to reopen); a verifier value
stored in `app_state` (rejected: custom cryptography).

---

## §2 Opening a database: telling the failure cases apart

**Decision**: `db::open_database(path, passphrase)` returns either an open
connection or exactly one of the following, and each maps to one command
error code (contracts/tauri-commands.md):

| Outcome | How it is detected | Code |
|---|---|---|
| File missing or unreadable | `metadata`/open fails before SQLite | `DATABASE_NOT_FOUND` / `DATABASE_UNREADABLE` |
| In use by another copy of HoploDex | `SQLITE_BUSY` on the first read (the other copy holds an exclusive lock) | `DATABASE_IN_USE` |
| Wrong passphrase, not a HoploDex database, or damaged first page | `SQLITE_NOTADB` on the first read | `PASSPHRASE_INCORRECT` (message per FR-006) |
| Opens, but is not a HoploDex database | `schema_migrations` missing, or the file has tables but no `app_state` row | `PASSPHRASE_INCORRECT` (same message: FR-006 does not distinguish) |
| Newer data layout | `schema_migrations` names a migration this build does not know | `DATABASE_NEWER_VERSION` |
| Damaged beyond the first page | `SQLITE_CORRUPT` while reading the schema or state at open, or from any later command | `DATABASE_DAMAGED` |
| Open on another computer | `app_state` open marker names a different machine id (§6) | `DATABASE_OPEN_ELSEWHERE` |

**Exclusive locking**: every connection sets `PRAGMA locking_mode =
EXCLUSIVE` and then does a write (setting the open marker), so that it holds
the file lock for as long as it is open. The spike showed that a second
connection then gets `SQLITE_BUSY` ("database is locked") **before** its
passphrase is checked, even with a wrong passphrase, so "in use" (FR-014) is
reported cleanly and never looks like a wrong passphrase. When the first
connection closes, the lock is released. Over SMB and NFS network shares,
SQLite's locks usually reach other computers too, so a database open on one
computer is reported "in use" on another. The message therefore says "in use
by another copy of HoploDex (on this or another computer)". On cloud-synced
folders each computer has its own copy of the file and locks do not cross,
which is the case the open marker (§6) exists for.

**Order of checks at open** (nothing is written before step 5, so a refused
open never modifies the file, as FR-006 and FR-014 require):
1. key and cipher settings (§1), then `locking_mode = EXCLUSIVE`;
2. first read (`SELECT count(*) FROM sqlite_schema`), which gives BUSY, NOTADB or CORRUPT;
3. newer-version check against `schema_migrations`, read-only;
4. open-marker check (§6), read-only;
5. apply any missing migrations, set the open marker, and (for a backup opened
   directly) take a new identity (§9). These are housekeeping writes (§5).

**Why not a full integrity check at open**: `PRAGMA quick_check` and
`cipher_integrity_check` read every page, which for a multi-gigabyte
collection of photographs would break SC-003. Damage past the first page
surfaces as `SQLITE_CORRUPT` when the damaged page is read. `CommandError::from_db`
maps it to `DATABASE_DAMAGED` wherever it happens, and the frontend then
offers the restore flow (FR-028, §8). The full checks run only where a copy
must be proved sound: passphrase change and restore (§3, §8).

**Data layout version (FR-014)**: the spike showed that `sqlcipher_export`
does **not** copy `PRAGMA user_version` (the copy read 0 where the source had
7), so the version is not kept there. The existing `schema_migrations` table,
which is an ordinary table and so is copied, is the version record. A database
naming a migration this build lacks is "newer". The migrations themselves are
edited in place while the application is unreleased (CLAUDE.md), so this
check is future-facing, and once the application is released it needs no change.

**Alternatives considered**: a sidecar lock file next to the database for
"in use" (rejected: it is left behind by crashes, and cloud-sync clients sync it);
checking the OS process list (rejected: not portable, and cannot see a copy
running under another account).

---

## §3 Making copies: raw copy for backups and restores, `sqlcipher_export` for a new passphrase

**Decision**:
- **Backup** (same passphrase): a chunked byte-for-byte copy of the database
  file to a temporary file in the backup folder. Then the copy is
  `ATTACH`ed to the open connection **without a key**, and its housekeeping
  is stamped (open marker cleared, pending changes removed, backup stamp set;
  §5, §9). Then it is `DETACH`ed, flushed to disk and renamed to its final
  name.
- **Passphrase change**: `ATTACH DATABASE ?tmp AS rekey KEY ?newPassphrase`
  (the file is created empty first: the session's connection is opened
  without `SQLITE_OPEN_CREATE`, so it can only attach a file that exists),
  `apply_cipher_settings(conn, "rekey")`, `SELECT sqlcipher_export('rekey')`,
  stamp the copy's housekeeping, `DETACH`, then verify (§4) and replace (§4).
- **Restore**: a chunked byte copy of the chosen backup to a temporary file
  next to the database, then opened with the **backup's** passphrase, verified
  and stamped (§8), then used to replace the database (§4).

**Spike findings this rests on**:
- A database `ATTACH`ed without `KEY` to a keyed connection is written with
  the main database's derived key **and its salt** ("copy shares salt with
  main: true"). The copy then opens on its own with the passphrase, so a raw
  byte copy of the main file can be attached and stamped without the backend
  ever holding the passphrase after open. This is what lets FR-007 hold for
  the whole session.
- `sqlcipher_export` copies tables, indexes, triggers and FTS5 virtual tables
  (the FTS5 query returned its row in the copy). It does **not** fire triggers
  while copying (the change-tracking trigger stayed unfired), so a copy does
  not mark itself as changed.
- `sqlcipher_export` into an `ATTACH … KEY ?` schema gives a file that opens
  with the new passphrase and refuses the old one.
- `rusqlite`'s `InterruptHandle::interrupt()` from another thread stops a
  running `sqlcipher_export` with `SQLITE_INTERRUPT` and leaves a partial
  file (32 KiB in the spike), which the caller must remove. This is how the
  passphrase change is stopped at sleep (§14).
- Throughput: 200 MiB exported in 1.05 s (about 190 MiB/s) on the development
  machine, in line with the spec's measurement.

**Reading the open file** (decided 2026-09-26, after a cross-process test
showed the problem): on Linux and macOS, closing any file descriptor on a
file drops every POSIX lock the process holds on it, SQLite's exclusive lock
included. A `std::fs` read of the open database (the page-1 probe of §1a, a
backup's raw copy) therefore released it: once the reader closed, another
copy of HoploDex could open a database that was in use (§2). SQLite's unix
VFS never closes a descriptor of its own while the file is locked, so every
read of an open database goes through **the descriptor SQLite already
holds**: `db::raw_file::RawFile` takes the main file's `sqlite3_file` with
`SQLITE_FCNTL_FILE_POINTER` and calls its `xFileSize` and `xRead`. SQLCipher
encrypts above the VFS, so the bytes read are the ciphertext on disk. It
costs nothing extra, and on Windows it also avoids the lock bytes that a
second handle cannot read. `tests/lock_held_test.rs` re-runs its own binary
as a second process and checks the database is still "in use" after each
such read. Files that are not open (a backup being restored from, the
`.old` and `.new` copies of §4) are read with `std::fs` as usual. Rejected:
opening a second SQLite connection just to read (it cannot read past the
first connection's exclusive lock), and a second descriptor kept open for
the whole session (it only moves the problem to the close).

**Why a raw copy for backups rather than `sqlcipher_export`**: the database
is quiet while a backup runs, because the backup runs at close, holding the
connection's mutex with no transaction open. Under `locking_mode = EXCLUSIVE`
with the default rollback journal, the file on disk is then complete and
consistent. A raw copy gives exact progress (bytes copied out of file size),
can stop between 1 MiB chunks without SQLite's interrupt machinery, and needs
no key derivation or page re-encryption. Its only cost is that it keeps free
pages, which `secure_delete` has already zeroed and which `reclaim_freed_space`
(`VACUUM` after deletes) keeps few.

**Progress for `sqlcipher_export`** (the one-call copy has no progress
callback, a follow-up from the spec): a monitor thread polls the size of the
destination file every 100 ms and reports it against the source's
`page_count × page_size`. The destination grows monotonically as pages are
written, so the bar is honest, if slightly uneven. Verification (§4) follows
as a second, indeterminate phase labelled "Checking the new copy…".

**Alternatives considered**:
- *SQLite Online Backup API* (`rusqlite::backup`, stepwise with progress):
  rejected. It needs a second connection keyed for the destination, which
  means holding the passphrase all session (against FR-007) or extracting the
  derived key (no supported API).
- *`VACUUM INTO`*: gives the same single blocking call as `sqlcipher_export`,
  but SQLCipher does not document its key handling, so it was not relied on.
- *`sqlcipher_export` for backups too*: it would work, and is kept as the
  fallback if a raw copy ever proves unsafe, but it is slower and its
  progress is less exact.

---

## §4 Verify, then replace in one step, then securely delete the old file

**Verification** (FR-015, passphrase change and restore): the temporary copy
is opened with a fresh connection using the passphrase it should have (new
for a change, the backup's for a restore), with the pinned settings, and must
pass all of:
`PRAGMA cipher_integrity_check` (every page's HMAC, no rows returned),
`PRAGMA integrity_check` (`ok`), the migration check (§2), and, for a
passphrase change, a row count for every table in `sqlite_schema` that equals
the source's (the source is idle, under the mutex). Any failure deletes the
copy and leaves the original untouched.

**Replacement protocol** (`services::file_swap`), with the database connection
closed first (Windows cannot replace an open file):
1. The verified copy sits at `<dir>/.<file>.new` (same folder, so the same volume).
2. **Preferred path**: hard-link the original to `<dir>/.<file>.old`, then
   `rename(.new → original)`, which atomically replaces the directory entry on
   Linux, macOS and Windows (Rust's `rename` uses `MoveFileExW` with
   `MOVEFILE_REPLACE_EXISTING`, then `fsync` of the directory where the OS
   supports it), then securely delete `.old` (§12). Because `.old` is a
   second name for the original's data, overwriting it overwrites the
   original's old contents. A plain `rename` over the original would free
   those blocks unwiped.
3. **Fallback**, where hard links are not supported (FAT/exFAT removable
   drives, some network shares): `rename(original → .old)`, then
   `rename(.new → original)`. The gap between the two renames is covered by
   recovery at the next open of that path: if the original is missing and
   `.new` is present, the second rename is completed; if only `.old` is present,
   it is renamed back.
4. If the final rename fails (a sync client or antivirus holding the file on
   Windows), it is retried a few times over about 2 s, then abandoned: `.new`
   is removed, the original is unchanged, and the user is told.
5. The database is reopened from the original path with the passphrase the
   operation just used, still held for this operation only.

Before starting, free space on the database's volume must be at least the
file size plus 5% (FR-016). The shortfall goes into `INSUFFICIENT_SPACE`'s
`details.bytesNeeded`. The free-space check uses `fs4` (MIT/Apache-2.0,
`available_space`; new, small, and built on `libc`/`windows-sys`, which are
already in the tree).

**Alternatives considered**: `ReplaceFileW` on Windows (does the same job,
but a Windows-only path to test for no gain over hard links, which NTFS
supports); a swap journal file (rejected: the fixed names `.new`/`.old`
already are the journal).

_Amended 2026-10-09 ([#63](https://github.com/exodious/HoploDex/issues/63)): a predictable name proves nothing about who put a file there, so the names stay but are treated that way. Every scratch file (`.new`, a backup's `.partial`, a restore's copy) is created exclusively (`services::scratch::create`), and an operation removes only a file it created: a file already at the name makes the operation fail and is left alone. Recovery is split in two: `file_swap::finish_interrupted` runs before the open and only completes or undoes the two-rename gap when the database is missing, and `file_swap::remove_leftovers` runs after the passphrase has opened the database, so a wrong passphrase deletes nothing. Both act only on a plain file of ours (`scratch::is_plain_file`: a regular file, not a symlink or folder, with one name, owned by this user on Unix) and skip and log anything else; `replace` refuses a `.new` that isn't one. Secure deletion never writes through a symlink and only unlinks a file with other names. `machine.json`'s temporary file has an unpredictable name and is created exclusively. A journal tied to file identities stays rejected; these checks cover the reported cases without one._

---

## §5 What counts as a change: triggers on collection tables, housekeeping kept apart

**Decision**: the database gets three new tables (data-model.md):
`collection_settings` (backup and lock settings; a collection change),
`app_state` (housekeeping: identity, open marker, backup record, dismissed
notes, backup stamp) and `pending_changes` (housekeeping). Every
**collection** table (`firearms`, `photos`, `document_attachments`,
`disposition_history`, `insurance_policies`, `firearm_types`,
`collection_settings`) gets `AFTER INSERT`, `AFTER UPDATE` and `AFTER DELETE`
triggers that run
`UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0`.
`app_state`, `pending_changes`, `schema_migrations` and the FTS5 tables have
none.

**Why triggers**: the flag is set inside the same transaction as the change,
so it is on disk the moment the first change of a session commits, which is
what FR-025 asks for ("a crash cannot lose that fact"). It covers every
command, including import and future ones, without each `ops` function
remembering to call anything. The `WHERE changes_waiting = 0` keeps it to one
real write per session. A guard test lists every table in `sqlite_schema` and
fails if a table is neither on the housekeeping list nor carrying all three
triggers, so a future table cannot slip through.

**Passphrase change** sets `changes_waiting = 1` explicitly in the new copy
(FR-025 counts it as a change). **Restore** sets it to 0 in the restored
copy, because the content equals an existing backup.

**Alternatives considered**: a flag set by the command layer on every
mutating command (rejected: easy to forget, and not transactional with the
change); SQLite's `data_version` or `total_changes()` (rejected: both count
housekeeping writes, and neither survives a crash).

---

## §6 The open marker and noticing a take-over

**Machine identity**: a random 128-bit id (`getrandom`, already a dependency)
is created on first run and kept in the machine-local settings file (§11). It
is per operating-system account rather than per computer, which does no harm:
two accounts on one computer cannot both have the file open anyway (§2's
exclusive lock), and an account is what "this computer" means for the keyring
too. The **display name** is the host name (`gethostname`, MIT/Apache-2.0,
new; a few hundred lines whose only dependencies are `libc` on Unix and
`windows-targets` on Windows, both already in the tree), with a trailing `.local`
removed on macOS. It is recorded with the marker so another computer can
name it (FR-032).

**Marker** (`app_state.open_machine_id`, `open_machine_name`, `open_since`):
set at open (step 5 in §2), cleared at every normal close in the same
transaction as the final backup-record write. At open:
- marker empty, or this machine's id: open (this machine's marker left by a
  crash is simply replaced; §2's exclusive lock has already proved no other
  copy here has the file open, which is FR-032's condition);
- another machine's id: refuse with `DATABASE_OPEN_ELSEWHERE`, with
  `details { machineName, since }`. The user may retry with
  `takeOver: true`, which the UI sends only after the destructive-action
  confirmation (contracts/ui-databases.md).

**Noticing a take-over while open** (FR-032: "before saving changes and at
close"): on a cloud-synced folder another computer's changes arrive as the
sync client **replacing the file** at the path. On Linux and macOS our open
file descriptor still points at the old, now unlinked, file, so reading the
marker through our connection would never see the change. Instead, the
session records the file's **fingerprint** after each of its own commits:
the file identity (device and inode on Unix, volume serial and file index on
Windows via `same-file`, already in the tree), its length and its modification
time. Before every write command, and at close, it compares the current
fingerprint at the path with the recorded one. A mismatch means someone else
replaced or wrote the file, which, with the local exclusive lock in place,
can only be another computer. The session then refuses the write
(`DATABASE_TAKEN_OVER`), stops, tells the user, and closes without writing
anything more (no backup, no marker clear). This needs no key and no second
connection. The check is a `stat`, about 10 µs, so it costs nothing measurable.
On Unix the identity comes from `stat` alone and the check never opens the
file: closing any descriptor on a file drops every POSIX lock the process
holds on it, SQLite's exclusive lock included. `same-file` is therefore used
on Windows only, where locks belong to the handle that took them.

Where the path is on a live network share and the other computer took over
after a crash here, SQLite's shared lock refuses its writes, or ours, instead.
Either way nothing is written from both sides.

**Storage that can't be reached is not a take-over** (spec Edge Cases, FR-032).
A removable drive that is unplugged, or a network share that drops, makes the
path vanish or its `stat` fail. Comparing fingerprints would then report a
take-over, which is the wrong message and the wrong advice. So the check
returns one of three results: `Same`, `Replaced` (the path exists and its
identity, length or modification time differ) or `Unreachable` (the path is
missing, or `stat` fails with an I/O or permission error). On `Unreachable`
the write is refused with `DATABASE_UNAVAILABLE { path }` before anything is
written, and the session is marked `storage_lost`. An SQLite I/O error
(`SQLITE_IOERR`, `SQLITE_CANTOPEN`) during a write does the same, since the
connection's file handle may already be dead. The session stays open, so the
form keeps its unsaved input on screen, but from then on every write is
refused with the same code, whatever the fingerprint later says, because a
remounted drive can come back with a new device number and look replaced.
The close that follows makes no backup (`CloseOutcome.backup = "failed"`,
with the `backupFailed` notice saying the file could not be reached) and writes
nothing, so the open marker stays set. It is this computer's own marker, so
the next open here, once the storage is back, clears it without a warning.
A drive that disappears and comes back between two checks, with a new
device number, is still reported as a take-over. That is rare, and it is
safe, because a take-over also writes nothing.

---

## §7 Backups: when, where, what they are called, how many

**When** (FR-025): at a normal close (close, switch, lock by command, idle
or screen lock, user quit), if `changes_waiting = 1` and
`last_backup_at` is not today in local time, and backups are on. Never at
sleep, OS shutdown or termination (FR-027, FR-037). After a successful
backup, in one transaction in the main database: `changes_waiting = 0`,
`last_backup_at = now`. If a backup is skipped or fails, nothing is written,
so the changes stay waiting.

**Where** (FR-026): the default location is stored as the token `default`
and resolves, on whichever computer opens the file, to a `HoploDex backups`
folder in the database's own folder. A custom location is stored as the
absolute path chosen (FR-024: it may not exist on another computer, in which
case the backup is skipped and reported, with a way to change the location,
FR-027). Changing the location asks what to do with the backups already at
the old one (§22).

**Names**: `<database name> <YYYY-MM-DD HHMMSS> <id8>.hoplodex`, for example
`Main collection 2026-09-25 143005 3fa2c9d1.hoplodex`. The database name is
the file name without its extension. `<id8>` is the first 8 hex digits of the
database's random id (`app_state.database_id`). Listing, rotation and "delete
all" select files by the id and the timestamp pattern, never by name alone,
so a custom folder shared by two databases, or a database renamed in the file
manager, never mixes up whose backups are whose. The name is readable and
sorts by date in a file manager. In-progress files end in `.partial`, which
no listing or open dialog matches, so they cannot be mistaken for a backup
(FR-027).

**Rotation**: after a new backup has been renamed into place, backups of this
database beyond `backup_keep_count` (default 5, range 1–100) are deleted,
oldest first, by secure deletion (§12). Excluded: the backup a restore is
being made from (FR-028). A restore's own "before restoring" backup is made
whatever the once-a-day limit says, and is named like any other.

**Progress** (FR-027, SC-005): the expected time is the file size divided by
a deliberately conservative 50 MiB/s, which allows for hard disks and network
folders; the spike measured about 190 MiB/s locally. When the estimate is
over 1 s, the closing screen shows the progress bar at once. When it is not,
the bar still appears if the copy is still running after 1 s. Progress events
report bytes copied out of the total. "Skip" stops the copy between chunks
and removes the partial file, and the changes stay waiting.

**Crash during a backup** (FR-027 scenario 7): before copying, the backup's
partial path is written to the machine-local settings file (§11). A normal
finish or skip clears it. At the next launch, a leftover entry means the last
backup did not finish: the partial file is removed and the chooser shows
"The last backup of <name> did not finish; its changes will be backed up at
the next close."

**Location unavailable or full**: detected before the copy (folder
missing, not writable, or free space under file size + 5%), or by an I/O
error during it. Either way the close goes ahead, the changes stay waiting,
and the chooser shows a notice with a "Change backup location…" action that
opens the database's backup settings after the next open.

**Disclosure before the first backup** (FR-024): in the create-database
dialog (contracts/ui-databases.md). The backup settings panel repeats it
(FR-029).

---

## §8 Restoring

**Decision** (FR-028):
1. The user picks a backup from the database's list (files in its backup
   folder carrying its id, newest first, with date, time and size).
2. They enter **that backup's** passphrase, since the backup keeps the one
   that was current when it was made. The dialog says so before they type.
   **Before anything is written**, the restore checks its preconditions and
   refuses with nothing changed if one fails: the backup location is
   available and writable (`BACKUP_LOCATION_UNAVAILABLE`, with a way to
   change it), and there is free space (§4's size + 5%) for the restored copy
   in the database's folder and for the "before restoring" backup (the
   current file's size) in the backup folder, both summed when the two are on
   the same volume (`INSUFFICIENT_SPACE`). A damaged database needs only the
   first, since it is renamed aside rather than backed up.
3. The backup is copied to `<db dir>/.<file>.new` (with progress, and
   stoppable), opened with that passphrase, verified (§4), and stamped: backup
   stamp cleared, `changes_waiting = 0`, `last_backup_at` unchanged, and
   identity kept.
4. Only then is the "before restoring" backup made from the current
   database, whatever the once-a-day limit, and even when automatic backups
   are turned off (FR-028): it is what makes the restore undoable, not an
   automatic backup. The restored copy is known good before the current state
   is touched. If this backup fails or is stopped, the restore is abandoned:
   its `.partial` and the `.new` copy are removed and the database is left as
   it was.
5. The database is closed and replaced (§4), with the old file securely
   deleted (a backup of it now exists), then reopened with the backup's
   passphrase. A saved keyring passphrase for this database is updated to it.
6. A notice says the database now opens with the passphrase current when the
   backup was made.

**A damaged database** (FR-028's last sentence, spec scenario US3-6): when
open fails with `DATABASE_DAMAGED`, or with `PASSPHRASE_INCORRECT` when
backups exist (the two cannot be told apart at page 1), the message offers
"Restore from a backup…". The backup folder is found from the recent-list
entry's cached `databaseId` and resolved backup folder (§11), because the
damaged file cannot be read. No "before restoring" backup can be made of a
file that does not open, so the damaged file is **kept**, renamed to
`<name> damaged <YYYY-MM-DD HHMMSS>.hoplodex` next to the restored database,
rather than deleted, and the notice says where it is.

---

## §9 A backup opened directly

A backup carries a **backup stamp** (`app_state.backup_made_at`, set in the
copy only). When a file with a backup stamp is opened other than through
restore (spec edge case: "opens like any other"), it becomes its own
database at open: it is given a new random `database_id` and the stamp is
cleared (housekeeping, not a change, §5), and a one-time notice says "This is
a backup of <name> made on <date>. Changes here are not part of <name>. It
is still in the backup folder, where it may be removed when older backups are
cleared: move the file elsewhere to keep it." With a new id its own backups,
its keyring entry and its rotation are separate from the original's, and it
does not share the original's saved passphrase, which could be newer than
the backup's.

---

## §10 Saving the passphrase in the keyring

**Decision**: keep the existing `keyring` 4.1 crate, with the platform stores
it already bundles (Secret Service over `zbus` on Linux, Keychain on macOS,
Credential Manager on Windows). Each saved passphrase is one entry: service
`io.github.exodious.HoploDex`, user `passphrase:<database_id>`. It is keyed by the
database's id rather than its path, so moving or renaming the file keeps it,
and a backup opened directly (§9) does not inherit it. The id is cached in
the recent-list entry so a saved passphrase can be found before the database
is opened.

**Availability** (FR-019): probed lazily, the first time the chooser or
settings need it, by reading a known-absent entry. `NoEntry` means available;
`PlatformFailure` or `NoStorageAccess` means unavailable, and the reason is
logged but never shown raw. The result is cached for the session.

**A saved passphrase that no longer works** (FR-018, US5-5): the open fails
with `PASSPHRASE_INCORRECT`, and the chooser falls back to the prompt with a
note ("The saved passphrase no longer opens this database"). A successful
typed open then overwrites the entry.

**The pre-feature key is left alone**: the developer's real database is
keyed by the old `sqlcipher-key` entry (CLAUDE.md, "Never touch the real
database"). This feature removes the code that reads it but **never deletes
or overwrites that entry**. No code path names it.

**Tests**: the `mock-keyring` feature stays, backing E2E and integration tests
with keyring-core's in-memory store. `HOPLODEX_E2E_DB_KEY` goes away, since
there is no random key to share any more. A new variable,
`HOPLODEX_E2E_KEYRING=unavailable`, read only by `mock-keyring` builds, makes
the probe report "unavailable" so FR-019 can be tested end to end. The
in-memory store forgets everything when the app exits, so E2E runs also set
`HOPLODEX_E2E_KEYRING_FILE` to a file in the sandbox that the store is loaded
from at launch and written back to after each save or forget, which lets a
remembered passphrase survive a relaunch. Tests that don't concern the
keyring never reach it at all: `MachineSettings::load` starts with the
keyring off, and only the app (and `keyring_test`) give it a real one.

---

## §11 Machine-local settings

**Decision**: one JSON file, `machine.json`, in the OS app **config**
directory (`~/.config/io.github.exodious.HoploDex/` on Linux), written atomically
(write to a temporary file, flush, rename). Contents (data-model.md): the
machine id (§6), the recent-databases list (path, name, last opened, cached
database id and resolved backup folder, whether a passphrase is saved, and
the file's modification time when this computer last closed it, FR-040), an
unfinished-backup record (§7), and notices waiting to be shown in the chooser
(a stopped operation, lost pending changes, a failed backup). This is
everything FR-013 keeps per computer. It holds no collection data and no
secret: file paths, names and times only.

A corrupt or unreadable `machine.json` is set aside as `machine.json.bad` and
a new one started, which loses only the recent list and cached ids. Saved
passphrases stay in the keyring and are found again once the database is
reopened.

**Alternatives considered**: `tauri-plugin-store` (rejected: a plugin for
one small file); keeping the recent list in `localStorage` (rejected: the
backend needs it at startup, and FR-018 removes keyring entries when an
entry is removed).

---

## §12 Secure deletion, extended

`services::secure_delete::secure_delete_file` (feature 001 FR-035) already
overwrites with zeros, flushes and unlinks. For whole database files and
backups, FR-016 also asks it to "ask the storage to discard the freed space
where available". After the overwrite and flush, it deallocates the file's
blocks before unlinking: `fallocate(FALLOC_FL_PUNCH_HOLE | FALLOC_FL_KEEP_SIZE)`
on Linux, `fcntl(F_PUNCHHOLE)` on macOS (APFS), and
`FSCTL_SET_ZERO_DATA` on Windows sparse-capable volumes. On a filesystem
mounted with discard, or with periodic trim, the freed blocks then reach the
SSD sooner. Every step is best effort, and a failure is ignored exactly as the
overwrite failure already is. Large files are overwritten in 1 MiB chunks
(the existing 64 KiB chunk stays for small document copies), and the overwrite
reports progress so that deleting all backups or rotating shows progress like
any other long-running operation.

---

## §13 Holding the open database: session state replaces `DbHandle`

**Decision**: `DbHandle(Mutex<Connection>)` becomes
`Session(Mutex<Option<OpenDatabase>>)` (module `session`), where
`OpenDatabase` holds the connection, the path, the database id, the file
fingerprint (§6), a `rusqlite::InterruptHandle`, and the staged form draft
(§16). Every existing command changes from `state.0.lock()` to one of two
helpers:
- `session.read(|conn| …)` fails with `DATABASE_CLOSED` when nothing is open;
- `session.write(|conn| …)` also runs the take-over check (§6) first and
  refreshes the fingerprint afterwards.

The `ops` functions keep taking `&Connection`, so integration tests and
`human_seed` are unchanged in shape. There is still one connection and no
pool (CLAUDE.md). Collection commands are refused with
`PENDING_CHANGES_UNRESOLVED` while the open database still has pending
changes (FR-039: "before the collection can be used").

**Long-running operations** register in an `Operations` registry: kind, a
cancel flag (`AtomicBool`) and, for `sqlcipher_export`, the interrupt handle.
Import and export check the flag between rows, as the spec's follow-up asks.
Import already commits row by row, so a stop keeps every row already
imported. Raw copies and overwrites check it between chunks. Deleting all
backups checks it between files and passes it to each file's overwrite; a
file whose overwrite is stopped part way is removed without finishing, so
nothing half overwritten is left to look like a backup. Moving backups to a
new location (§22) checks it between files and between each copy's chunks.
The registry is also what pauses the idle clock (§15).

---

## §14 Sleep, wake, screen lock and shutdown notices, per operating system

A `platform::system_events` module turns each OS's notices into one stream:
`WillSleep { ack }`, `Woke`, `ScreenLocked`, `ScreenUnlocked`,
`WillShutDown { ack }`. `ack` is a guard whose drop tells the OS the
application is ready. Each OS backend runs on its own thread.

| | Sleep (with time to finish) | Wake | Screen lock | OS shutdown / logout |
|---|---|---|---|---|
| **Linux** | logind `PrepareForSleep(true)` on the system bus, holding a **delay inhibitor** (`Inhibit("sleep", …, "delay")`, default maximum 5 s) that is released when the lock finishes and taken again on wake | `PrepareForSleep(false)` | logind session `Lock` signal and `LockedHint` property; `org.freedesktop.ScreenSaver` / `org.gnome.ScreenSaver` `ActiveChanged(true)` on the session bus | logind `PrepareForShutdown(true)` with a `shutdown` delay inhibitor; SIGTERM/SIGHUP/SIGINT (existing) |
| **macOS** | IOKit `IORegisterForSystemPower`: `kIOMessageSystemWillSleep`, answered by `IOAllowPowerChange` once the lock finishes (the system waits up to 30 s) | `kIOMessageSystemHasPoweredOn` | distributed notification `com.apple.screenIsLocked`, registered to be delivered immediately (AppKit holds distributed notifications back while the application isn't active) | `NSWorkspaceWillPowerOffNotification`, then the quit Apple event (tao reports it as an exit request, which is then known to be OS-initiated) |
| **Windows** | `WM_POWERBROADCAST` / `PBT_APMSUSPEND` (about 2 s allowed) | `PBT_APMRESUMEAUTOMATIC` | `WTSRegisterSessionNotification` then `WM_WTSSESSION_CHANGE` / `WTS_SESSION_LOCK` | `WM_QUERYENDSESSION` / `WM_ENDSESSION`, with `ShutdownBlockReasonCreate` while pending changes are saved |

On Windows these messages go only to **top-level** windows. Message-only
windows (`HWND_MESSAGE`) do not receive broadcasts, so the backend creates
one hidden, never-shown top-level window on its own thread with its own
message loop.

**Crates**: Linux uses `zbus` 5, already in the tree through the keyring's
Secret Service store; it becomes a direct dependency. macOS uses `objc2`,
`objc2-foundation` and `objc2-app-kit` (already in the tree through Tauri's
`tao`) and a short `extern "C"` block for four IOKit functions. Windows uses
`windows-sys` 0.61 (already in the tree) with the `Win32_System_Power`,
`Win32_System_RemoteDesktop`, `Win32_System_Shutdown` and
`Win32_UI_WindowsAndMessaging` features. Nothing new is downloaded, only
features switched on.

**Screen-lock availability** (FR-038): available on macOS and Windows.
On Linux it is available when the logind session object or either
ScreenSaver interface is reachable at startup. Otherwise the option is shown
as unavailable with the explanation from contracts/ui-databases.md.
Desktops that lock without telling logind or implementing ScreenSaver (some
bare window-manager setups) look available but never report a lock. The
in-app guide mentions this.

**Missed or late notices** (FR-037's "finish on waking"): a watchdog tick
every second compares wall-clock time with monotonic time. On all three
operating systems the monotonic clock stops during sleep. A jump of more than
5 s means the computer slept without a usable notice (Windows Modern Standby
can deliver `PBT_APMSUSPEND` late, and a lid close can beat the logind
signal). If the idle lock is on and a database is still open, the sleep lock
is carried out then (§15). The idle clock also runs on wall time (§15), so
any sleep longer than the idle duration locks at the first tick after waking
whatever else happened. The frontend is told first (`session:closed`) so it
drops the collection view before anything else runs. If a sleep lock was
started but not finished before the computer slept, `Woke` (or the
watchdog) runs `finish_on_wake`, which completes FR-037's steps 1–3 before
any command is accepted, then the rest.

**A sleep during a close already under way** (FR-037, FR-038): a close or
lock in progress (typically its backup, for a screen lock or a user close) is
turned into an immediate close whatever the idle-lock setting, since the
user has already asked for the database to close. The running backup is
stopped through the operations registry (its `.partial` removed, the changes
left waiting), and the close continues from `close_immediate`'s step 2. A
screen lock's close runs on a thread of its own, not the one that reads the
notices: closing a laptop's lid locks the screen and then sleeps, and the
sleep notice must not wait behind the lock's backup. The
idle-lock switch only decides whether `WillSleep` closes a database that is
open and not closing.

**Clearing passphrase fields** (FR-007): on `WillSleep` and `ScreenLocked`
the backend always emits `system:clear-passphrase-fields`, whatever the
settings and whether or not a database is open. Every passphrase field
listens for it and resets.

**Alternatives considered**: `tauri-plugin-*` community power plugins
(rejected: none covers all three operating systems and all four notices, and
each adds a dependency to review); polling the Linux `LockedHint` property
(rejected: signals are available and cost nothing between events).

---

## §15 The idle lock

**Decision**: the idle clock lives in the **backend**. The frontend listens
for `keydown`, `pointerdown`, `pointermove`, `wheel` and `touchstart` on the
window and sends `note_activity` at most once per second, on both the leading
and trailing edge, so the last input is always reported within 1 s. The
backend keeps the time of the last input on the **wall clock**, and a 1 s tick
locks when `now − last_input ≥ duration`, which is within SC-010's 10:00–10:05
window. The clock is **paused** while any long-running operation is
registered (§13) and while a native file or folder dialog is open. The
frontend wraps `@tauri-apps/plugin-dialog` calls in `withIdlePaused`. The
photo and document pickers are `<input type="file">` elements, which open the
system chooser with no promise to wrap and send the page no input while it is
open, so `pauseIdleForFileInput(input)` pauses on the input's `click` and
resumes on its `change` or `cancel` event, or on the window's next `focus` as
a fallback where `cancel` is not fired. On
resume, the idle time starts again from zero (spec edge case: "the idle time
starts once it finishes").

**Why the backend**: WebKitGTK and WebView2 throttle or freeze timers in
hidden or minimized windows, so a frontend timer can fire late, or not at
all, while the window is minimized. A throttled webview only delays
`note_activity`, and there is no input in a minimized window to report
anyway.

**Why wall time**: time asleep then counts as idle, which closes the gap
where the computer slept without a usable notice (§14).

**Alternatives considered**: OS-wide idle APIs (`XScreenSaverQueryInfo`,
`GetLastInputInfo`, `CGEventSourceSecondsSinceLastEventType`), rejected
because the spec counts only input to the application's own windows.

---

## §16 Pending changes: the frontend mirrors the draft to the backend

**Decision**: while a firearm record (add, edit, dispose, restore, or its
insurance coverage) or an insurance policy (add, edit) form has unsaved input, the frontend
**stages** a draft in backend memory with `stage_pending_changes`, debounced
to 250 ms after the last edit and flushed at once on blur. A clean form sends
`null`. The draft lives only in `OpenDatabase`, in memory, until a lock or an
OS shutdown writes it to `pending_changes` inside the database, in the same
transaction that clears the open marker. It is removed from memory with the
rest of the session.

**Why stage rather than ask at lock time**: at sleep and at OS shutdown the
backend has seconds or less, and cannot rely on a possibly throttled webview
to answer. With the draft already in the backend, the backend's lock
procedure never waits for the frontend. "Lock now" and the idle lock still
flush the latest draft first (`lock_database` takes it), so the saved changes
are exactly the input at the moment of locking (SC-010). At sleep, the staged
draft is at most 250 ms behind the last keystroke.

**Draft shape**: `{ formVersion, kind, mode, targetId, label, values }`.
`kind` is `firearm` or `policy`. `mode` is `add`, `edit`, `dispose`,
`restore` or `coverage`. `label` is what the resume prompt names, for example
"Glock 19 (edit)" or "New firearm". `values` is the form's own state object,
serialized as the form holds it. The backend treats `values` as opaque JSON
(limited to 1 MiB, which is ample for text fields, since photos and documents
are added through their own commands and are never part of a form). On
resume, a draft whose `formVersion` the frontend does not know, or whose
target record no longer exists, can only be discarded, with that stated.

**Resume**: after an open, `get_database_status` reports the pending
changes. Until the user resumes or discards, the collection commands answer
`PENDING_CHANGES_UNRESOLVED` (§13). The frontend shows the pending-changes
prompt, then navigates to the target and opens the form with the draft as
its initial, unsaved state. `resolve_pending_changes` removes the row either
way.

**Alternatives considered**: persisting drafts to disk on every keystroke
(rejected: it writes housekeeping continually, and puts unsaved input into
files every few seconds); `localStorage` (rejected: plaintext on disk,
against FR-039's "protected by its passphrase").

---

## §17 Close, switch and quit with unsaved changes

**Decision**: the window's close button and every other quit route become
**requests**:
- `WindowEvent::CloseRequested` and `RunEvent::ExitRequested` without an
  OS-initiated flag are prevented, and `app:quit-requested` is emitted.
- The frontend's session layer asks save, discard or cancel if a form is
  dirty (FR-010). Saving runs the form's own submit. Validation failure or
  cancel keeps everything open.
- It then calls `quit_application`, which does a normal close (backup if due,
  with progress and skip, FR-027) and exits.

Within the application a database is closed by locking it (spec,
Clarifications 2026-09-27): **Lock now** is the database menu's only close,
and switching is a lock followed by an open from the chooser. A lock never
asks (FR-033), so quitting is the one place the question is asked. A
separate close could not ask either: every form that can hold unsaved input
is a dialog, which covers the menu. `close_database` stays for the one
screen that still offers a close, the "couldn't be shown" panel
(contracts/ui-databases.md §13), where nothing unsaved can be recovered.
OS-initiated
exits (signals, `PrepareForShutdown`, `WM_ENDSESSION`, macOS power-off) never
ask. They save the staged draft as pending changes, clear the open marker if
that is part of the same write, close the connection, delete the decrypted
document copies, then exit (FR-039 ordering).

---

## §18 Passphrase strength hint

**Decision**: `@zxcvbn-ts/core` with `@zxcvbn-ts/language-common` and
`@zxcvbn-ts/language-en` (all declared MIT, no network access, no telemetry),
**loaded lazily** the first time a new-passphrase field is shown, so the
dictionaries (a few hundred KB) do not slow the application's start. The
hint shows a five-step meter and zxcvbn's own suggestions, with the sentence
"Longer is stronger: several unrelated words make a good passphrase". It
never blocks: only FR-003's 12-character minimum and the match with the
confirmation field block. The estimate runs in the frontend, so no
keystrokes cross IPC.

**Alternatives considered**: a length-and-character-class heuristic
(rejected: it rates `passwordpassword` highly and pushes users towards the
composition rules FR-003 forbids); the Rust `zxcvbn` crate (rejected: every
keystroke of the passphrase would cross IPC).

**Licenses of the dictionaries** (constitution 1.2.0, Licensing). The
packages declare MIT, which is all the license audit sees, but the
dictionaries are bundled data files, so their sources are recorded here.
`@zxcvbn-ts/language-en` 4.1.1 ships `commonWords.json`, which its
`THIRD_PARTY_LICENSES.md` says is generated from the OpenSubtitles 2024
dataset (via OPUS, Helsinki-NLP) under **ODC-BY**, a data license whose only
condition is attribution when redistributed. It is accepted as an exception
scoped to `@zxcvbn-ts/language-en`: the list is data the strength estimate
looks words up in, not code combined with the program, and the attribution
is met by the release's third-party notices. The package's other lists
(`wikipedia.json`, `firstnames.json`, `lastnames.json`,
`wordSequences.json`) and `@zxcvbn-ts/language-common` 4.1.3's
`passwords.json`, `diceware.json` and `adjacencyGraphs.json` state no
source; they are checked by hand before the first release. The exception and
the attribution are recorded in "License audit" in DEVELOPMENT.md, next to
the OFL-1.1 one, since `scripts/check-npm-licenses.mjs` reads only the
declared license and cannot express it. Dropping the English dictionary was
rejected: the hint would no longer notice ordinary English words.

---

## §19 File extension, default location and name

- **Extension** `.hoplodex` for databases and backups alike (a backup
  "opens like any database", FR-023). The open dialog filters on
  `.hoplodex`, with an "All files" option for renamed files.
- **Default location suggested** (FR-009): `<Documents>/HoploDex/`, from the
  OS documents folder. On Linux that is resolved through `XDG_CONFIG_HOME`'s
  `user-dirs.dirs`, with `~/Documents` as the fallback. The default name is
  "My collection". Nothing is ever suggested inside the app data directory, where
  the pre-feature `hoplodex.db` lives.
- **Test isolation**: E2E and screenshot runs already point `XDG_CONFIG_HOME`
  at a scratch directory. `e2e/wdio.conf.ts` additionally writes a
  `user-dirs.dirs` there that sets `XDG_DOCUMENTS_DIR` to a scratch folder,
  so even accepting the suggested location stays inside the sandbox. Specs
  type locations into the location field rather than using the native folder
  picker, which WebDriver cannot drive.

---

## §20 Tests and the key-derivation cost

**Decision**: `tests/support::TestDb` creates its database through the real
`db::create_database` with a fixed test passphrase and the **production**
cipher settings. No weakened test profile, so every integration test exercises
the real format. At about 0.34 s per open and 282 current tests, that is
roughly 100 s of CPU spread across the test threads, a few seconds of wall
time on the development machine. If the suite's wall time grows noticeably,
the fallback is a `TestDb` that creates one template database per test binary
and copies the file for each test. That keeps production settings and needs
no extra key derivation, because the copies share the salt (§3).

**Portability check (SC-001)**: the multi-OS run the spec defers stays
deferred (CI is disabled on purpose). Two checks stand in for it. A committed
fixture, `src-tauri/tests/fixtures/portable-v1.hoplodex`, made by an example
program with a known passphrase and holding a firearm, a photo and a
document, is opened and compared byte for byte by `portability_test.rs` on
whatever OS runs the tests. Any accidental change to the pinned settings
(§1) then fails the build, and on a macOS or Windows machine it is the
cross-platform check. The fixture is made on Linux, so the check is done by
running the Rust gates (`cargo clippy --all-targets`, `cargo fmt --check`,
`cargo test`) by hand on macOS and Windows before merge. That is also the
only place `platform/macos.rs` and `platform/windows.rs`, and every other
`cfg(target_os = "macos")` or `cfg(windows)` block, are compiled and linted
(constitution I): the container builds for Linux only, and a cross-target
`cargo clippy` from it would need a C toolchain and SDK for each target to
build the bundled SQLCipher. Walkthrough 2
in quickstart.md covers the other direction, a file made on macOS or Windows
opened on Linux. A second test asserts that the file's first 16 bytes
are a random salt and that no machine-local state (settings file, keyring) is
consulted to open it.

_Amended 2026-10-09: the parked workflow was deleted, unreviewed and never run; full CI isn't implemented yet ([#25](https://github.com/exodious/HoploDex/issues/25)). CodeQL and the dependency audit run as their own workflows (DEVELOPMENT.md, "Continuous integration")._

---

## §21 Constitution: the local-backup, test-isolation and licensing amendments

The constitution's Security & Data Handling section said "any network sync or
backup feature MUST be opt-in, off by default". The spec's first
clarification read this as covering backups that **leave the device**, and
asked for the wording to say so. This was planned as a PATCH amendment
before this feature merges, and was recorded in the plan's Complexity
Tracking as a deviation until then.

**Resolved by constitution 1.1.0 (2026-09-26).** The rule now covers features
that sync or back up data over a network or to a cloud service. A backup the
application writes only to local storage, in a location the user can see and
choose, may be on by default, and a local folder that another program syncs
elsewhere is the user's choice. The amendment was MINOR rather than PATCH,
because it also added a dependency vulnerability policy, the dependency audit
as a pull-request gate, and a whole-codebase AI-assisted security review as a
release gate. This feature's dependencies are held to the first two (plan,
Technical Context), and the release review is left to the first release
(spec, Clarifications 2026-09-26).

**Constitution 1.2.0 (2026-09-26)** added the testing rule that was left out
of 1.1.0, and a Licensing section.

- **Test isolation** (Testing Standards): tests and any tooling that runs the
  application must not read, write or delete the user's real database, the
  key or passphrase stored for it, or other real application data; they use
  throwaway locations and test passphrases, and seeding tools must refuse the
  real data directory. With this feature real data is no longer one file in
  the app-data directory, so the rule covers every database the user keeps,
  their backups, `machine.json` with its recent list, the suggested
  `<Documents>/HoploDex/` folder, and the `passphrase:<database_id>` keyring
  entries, as well as the pre-feature database and its `sqlcipher-key` entry
  (§10). Integration tests take every path as a parameter and create it in a
  temp directory, never resolving the real config or documents directory;
  keyring tests use the `mock-keyring` store; E2E and screenshot runs keep
  their scratch `XDG_*` directories and `user-dirs.dirs` (§19).
- **The human-testing seed** can no longer refuse one known directory,
  because the real databases may be anywhere. It uses an allow list instead
  (spec, Clarifications 2026-09-26): it writes only into a directory that is
  new or holds a `.hoplodex-sandbox` marker file the seed created there, and
  refuses any target that is, or lies inside, the real data, config or
  documents directory, resolved from the environment the seed was started
  with. It writes no keyring entry. The check lives in
  `examples/support/sandbox.rs`, included by `human_seed.rs` and by
  `tests/seed_sandbox_test.rs` through `#[path]`, so it is tested without
  shipping in the application. A blocklist of known real folders was
  rejected: it could not name a folder the user chose, and building one from
  the real `machine.json` would mean reading real application data.
- **Licensing**: every dependency this feature adds or promotes passes the
  license audit. All the new and promoted crates and npm packages declare
  MIT, Apache-2.0 or both (`gethostname` is Apache-2.0 only). The one bundled
  data file needing more than that is the ODC-BY word list (§18), accepted as
  a scoped exception. The third-party notices every release must carry are
  left to the first release, like the security review (spec, Clarifications
  2026-09-26); the ODC-BY attribution is recorded for them.

---

## §22 Changing the backup location: move, leave or delete the backups already made

Added 2026-09-27 for FR-026 as amended (spec, Clarifications 2026-09-27).

**Which backups are the database's**: only those at the current location.
Listing, restore, rotation and "delete all" already select files by folder
and `<id8>` (§7), so this needs no new state: a backup left at an old folder
simply stops being listed, and changing the location back to that folder
lists it again. Nothing records where backups used to be.

**Asking, with the existing warning pattern**: `update_backup_settings`
gains an optional `existingBackups: "move" | "leave" | "delete"`. When the
resolved folder changes (compared as §7's `same_path`, so choosing the
default folder's own path as a custom location is not a change) and the
choice is absent, the command saves nothing and refuses with one of:
- `BACKUPS_AT_OLD_LOCATION { folder, count, totalBytes }`: the old folder
  holds `count` backups of this database. The UI asks the question and sends
  the settings again with the choice.
- `OLD_BACKUP_LOCATION_UNAVAILABLE { folder }`: the old folder cannot be
  read: a custom folder that is missing (its drive unplugged, or a path from
  another computer) or that cannot be listed. The default folder sits next
  to a database that is open, and therefore reachable, so for it "missing"
  only means that no backup has been made yet. The UI warns, and **Continue** resends
  with `existingBackups: "leave"`.

No backups at the old folder means no question. This is the existing
"fail with a warning code, resend confirmed" pattern (CLAUDE.md,
Architecture), so the backend decides, from the files, whether to ask, and
the frontend never lists folders itself. A separate "count the backups"
command was rejected: the count could change between it and the save.

**Leave**: the new location is saved. Nothing else happens.

**Delete** (FR-026: "done as deleting all backups under FR-029"): the UI
first shows FR-029's destructive confirmation, then sends
`existingBackups: "delete"`, which is itself the confirmation, as
`takeOver: true` is for a take-over. The command runs the same code as
`delete_all_backups` (secure deletion, `backups_delete:progress`, stopping
between files at sleep), on the **old** folder, and saves the new location
only when every backup there was deleted. If one could not be deleted
(`BACKUPS_NOT_ALL_DELETED`, naming the files), or a sleep stopped it, the old
location is kept, so the ones remaining are still
the database's backups and "delete all backups" in the settings can finish
the job, which is what the stopped-deletion notice already tells the user
(FR-037). Saving the new location regardless was rejected: backups the
user asked to delete would quietly become files the application no longer
knows about.

**Move**, in this order:
1. **Preconditions, before anything is written**: the new folder passes
   §7's `check_location` (the default folder is made if missing; a missing
   custom one is refused with `BACKUP_LOCATION_UNAVAILABLE`), and it has
   free space for all the backups to be moved, their sizes summed, plus
   §4's 5% (`INSUFFICIENT_SPACE`). Backups whose name is already taken at
   the new folder are left out of both the move and the sum. The check does
   not try to tell whether the two folders share a drive: the spec asks for
   room for all of them, and a disk that full could not take the next
   backup either.
2. **Save the new location** (a collection change, through `session.write`,
   so the take-over check runs first). From here the new location is kept
   whatever happens (FR-026), which is why it is saved before the first file
   moves: a sleep closes the session at once, and nothing could be saved
   after it.
3. **Move each backup, oldest first**, as a long-running operation
   (`OperationKind::MoveBackups`: it pauses the idle clock and a sleep stops
   it, §13, §15):
   - A name already taken at the new folder: skipped and counted as left
     behind. Nothing is ever overwritten.
   - **Same drive**: `fs::hard_link(old, new)` then remove the old name.
     A hard link never replaces an existing file (it fails with
     `AlreadyExists`), is atomic, and moves no bytes, so the file's blocks
     are not freed and there is nothing to wipe.
   - **Otherwise** (the link fails: another drive, or a filesystem without
     hard links such as FAT or exFAT): copy it in 1 MiB chunks to
     `<new name>.partial` in the new folder (§3's `copy_chunked`, stoppable
     between chunks), flush it, **verify** it by reading it back and
     comparing it byte for byte with the original, then rename it to its
     final name with §7's `finalize` (which refuses a taken name), and only
     then remove the original by secure deletion (§12). A partial copy that
     is stopped or fails is removed, and its original kept.
   - Progress: `backups_move:progress { processed, total }` in bytes, the
     copy and the read-back each counting once, so `total` is twice the
     bytes to be copied. The estimate is §7's 50 MiB/s over that total, and
     it shows at once when over 1 s (FR-026, as FR-027).
4. **At the first failure** (the new drive fills or disappears despite the
   check, or an original cannot be read or removed), the move ends: the
   backups already moved stay at the new folder and the rest at the old one.
   The command still succeeds, since the settings were saved, and reports
   `leftBehind { count, folder }` with the reason. Carrying on after a
   failure was rejected: a failing destination usually fails every later
   file too, and the user is told what was left either way.

Moving removes nothing to meet `backup_keep_count`, even when the new
folder already holds backups of the database (left there by an earlier
change of location). The next completed backup rotates as usual (FR-025,
SC-007 as amended).

**Verifying without a key**: a backup keeps the passphrase that was current
when it was made, and the backend holds no passphrase (FR-007), so the copy
cannot be opened to check it. A byte-for-byte comparison with the original,
after the copy is flushed, proves the copy is the same file, which is what a
move must preserve. It does not prove the original was sound, but a move
must not change that either. Hashing both was rejected: it reads as much and
would need a hash crate for no gain.

**Stopped at sleep** (FR-037): the operation records how many backups it
has not yet moved. `report_stopped` turns that into the chooser notice
`operationStopped { operation: "moveBackups", leftBehindCount, folder }`:
"<n> backups of <name> are still in <folder>". The partial copy is removed
as the operation unwinds (FR-037's step 5), and its original is intact.

**A crash or forced quit mid-move**: before the first file, the move is
recorded in `machine.json` as `unfinishedBackupMove { databasePath,
databaseId, fromFolder, partialPath }`, with `partialPath` updated for each
copy and the record cleared when the move ends. At the next launch the sweep
that already handles an unfinished backup (§7) removes the partial file,
counts this database's backups still in `fromFolder` (by the cached id), and
leaves a `backupsLeftBehind { databasePath, folder, count }` notice when
there are any. A crash between a copy's rename and its original's removal
leaves the same backup in both folders. Nothing is lost, and the one in the
old folder is an unmanaged file like any other left behind.

**Folders are never removed**: an old default `HoploDex backups` folder
that the move empties is left in place. Removing folders the user may have
put other files in was rejected, and an empty folder does no harm.

**A lock during the move** (Ctrl/⌘+L): the move is a registered operation
like "delete all backups", so a lock treats it the same way. Its progress
view cannot be dismissed, so it shows no lock button (contracts/ui-databases.md
§0).

**Alternatives considered**: moving with a plain `fs::rename` on the same
drive (rejected: it replaces an existing file on Unix, which FR-026
forbids, and "same drive" cannot be told in advance without platform
code); moving in the background after the settings dialog closes (rejected:
the user must learn at once whether anything was left behind, and a second
long-running operation could not start meanwhile); rotating after a move
into a folder that already holds backups (rejected by the spec).
