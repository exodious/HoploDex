# Contract: UI (feature 009 deltas)

A delta against 003's `contracts/ui-databases.md` and 001's policy form. In
the text, "<name>" is the database's name in curly quotes, as the chooser
writes it ("“Main collection”"), and "<version>" is the build's version
("0.2.0"). Dates and times are shown as the rest of the app shows them
(`formatDateTime`).

Every dialog here uses the shared `Dialog` or `ConfirmDialog`, follows the One
Enter rule (CLAUDE.md), and is reachable and operable by keyboard alone
(WCAG 2.1 AA, constitution III).

## 1. The upgrade question (FR-010)

When `open_database` answers `UPGRADE_NEEDED`, the chooser shows a
non-destructive `ConfirmDialog`:

- **Title**: "Upgrade <name> for HoploDex <version>?"
- **Description**:
  - "<name> was made by an earlier version of HoploDex. To open it, this
    version needs to upgrade it."
  - "First, HoploDex saves a backup of it as it is now, in <backupFolder>."
  - "After the upgrade, earlier versions of HoploDex can open that backup,
    but not <name> itself."
- **Buttons**: **Upgrade** (primary, focused when it opens) and **Cancel**.
- **Cancel**, Escape or closing the dialog sends `cancel_upgrade` and
  returns to the chooser with the row selected and no message. The database
  is closed untouched.
- **A sleep or screen lock** (`system:clear-passphrase-fields`) closes the
  question as Cancel, since the backend has dropped the waiting database.
- The row shows its busy state while the question is up, as during an open.

## 2. Upgrade progress (FR-016)

When **Upgrade** is chosen, the question closes and a `Dialog` titled
"Upgrading <name>" takes its place.

- **When it shows**: at once when the first `upgrade:progress` has
  `showNow`; otherwise only if the upgrade is still running a second after
  **Upgrade** was chosen. Until it shows, the row's busy state is the
  feedback (within 100 ms, constitution IV).
- **Phase `backingUp`**:
  - the line "Saving a backup of <name>…";
  - a `ProgressBar` of `processed` / `total`;
  - **Cancel**, the dialog's only button.

  Cancel sends `cancel_upgrade`. The dialog then shows "Stopping…" until the
  command returns, and closes back to the chooser with no message.
- **Phase `upgrading`**:
  - the line "Upgrading <name>. This can't be stopped once it has started.";
  - an indeterminate `ProgressBar`, or a step count when there is more than
    one step;
  - no buttons, and Escape does nothing.
- **The dialog has no close button in either phase.**
- **Accessibility**: the phase line is a live region (`aria-live="polite"`),
  so a screen reader hears the change from saving to upgrading.

## 3. The backup can't be saved at its location (FR-012)

On `BACKUP_LOCATION_UNAVAILABLE`, `INSUFFICIENT_SPACE` or
`UPGRADE_BACKUP_FAILED` from `upgrade_database`, a `Dialog` titled "Save the
backup somewhere else?" shows:

- **Text**: "HoploDex couldn't save the backup it makes before upgrading
  <name> in <folder>: <reason>. You can save this one backup in a folder you
  choose instead, or cancel. <name> isn't upgraded without a backup." The
  reason is one of:
  - `missing`: "the folder isn't there; it may be on a drive that isn't
    connected";
  - `notWritable`: "HoploDex isn't allowed to save files there";
  - `insufficientSpace`, or `INSUFFICIENT_SPACE`: "there isn't enough free
    space (it needs <required>; <available> is free)";
    _Amended 2026-10-17: `BACKUP_LOCATION_UNAVAILABLE` carries no sizes, so
    with `insufficientSpace` the parenthesis is left out; `INSUFFICIENT_SPACE`
    has them._
  - `UPGRADE_BACKUP_FAILED`: "saving it failed part way".
- **Buttons**: **Choose a folder…** (primary, focused) and **Cancel**.
- **Choose a folder…** opens the OS folder picker. Choosing one sends
  `upgrade_database { awaitingId, backupFolder }`, and §2's progress follows.
  A refusal there shows this dialog again, naming the chosen folder.
  Dismissing the picker leaves this dialog up.
- **Cancel** sends `cancel_upgrade` and returns to the chooser.

## 4. The upgrade failed (FR-015)

An `UPGRADE_FAILED` from `upgrade_database` closes the progress dialog. The
selected row shows, in the existing open-failure slot (003 ui-databases.md
§1, "Open failures"):

> HoploDex couldn't upgrade <name>. It is as it was before, and hasn't been
> opened. The backup saved before the upgrade is <backupPath>.

The row stays selected, so **Open** can be tried again. A `DISK_FULL` uses the
existing `DISK_FULL` text.

## 5. After the upgrade: the note (FR-017)

A note shown once, in the notes area that heads the Collection, Accessories
and Insurance pages (003 ui-databases.md §10, amended), and dismissible like
the others:

> <name> was upgraded for HoploDex <version>. A backup of it from before the
> upgrade is <backupPath>. Earlier versions of HoploDex can open that backup,
> but not the upgraded database.

When `atBackupLocation` is false, it adds: "That folder isn't your backup
location, so the backup isn't in the database's list of backups."

After a restore that upgraded (§6), the note reads:

> The backup was made by an earlier version of HoploDex, so <name> was
> upgraded for HoploDex <version> as it was restored. Earlier versions can
> still open the backup it came from, <backupPath>.

It is shown with the existing restored note, which comes first.

## 6. Restoring a backup in an older layout (FR-019; amends 003 ui-databases.md §9)

- **The check comes first.** Pressing **Restore** with a passphrase typed
  sends `check_backup` before the confirmation step. The button shows its
  busy state meanwhile.
  - A wrong passphrase is shown at the field, as today: "That passphrase
    didn't open the backup from <date>." No confirmation appears.
  - `DATABASE_NEWER_VERSION` is shown above the footer: "This backup was made
    by a newer version of HoploDex, so this version can't restore it."
- **The confirmation** (the existing destructive `ConfirmDialog`) adds, when
  `upgradeTo` is set, after its description: "This backup was made by an
  earlier version of HoploDex, so it is upgraded for HoploDex <version> as it
  is restored. The backup itself stays as it is, and earlier versions can
  still open it." Confirming sends `restore_backup` with `upgrade: true`.
- **Progress** gains the phase line "Upgrading the restored copy…".
- **`UPGRADE_FAILED`** is shown above the footer, with the dialog left open:
  "The backup couldn't be upgraded for this version, so nothing was restored.
  The database is as it was, and a backup of it was saved before the restore
  began."
  _Amended 2026-10-17: the text is now "The backup couldn't be upgraded for
  this version, so nothing was restored. The database is as it was." The
  copy is upgraded before the current database is saved (`upgrading` comes
  before `savingCurrent`), so no "before restoring" backup exists yet._

## 7. Backup lists and the "backup opened directly" note (FR-013)

- **Restore dialog list**: a backup made before an upgrade reads "15 January
  2027, 09:30 — 212 MB — before upgrading to HoploDex 0.2.0".
- **Chooser row details** (003 FR-040) are unchanged. The count includes it.
- **The "backup opened directly" note** (003 ui-databases.md §10), when
  `beforeUpgradeTo` is set, reads: "This is a backup of <backupOfName> made on
  <date>, before it was upgraded to HoploDex <beforeUpgradeTo>. Changes here
  aren't part of <backupOfName>. It's still in the backup folder: move the
  file elsewhere to keep it." A backup made before an upgrade isn't removed
  when older backups are cleared while it is the latest one (FR-014), so the
  "may be removed" clause is left out.

## 8. A database from a newer version (FR-018; amends 003 ui-databases.md §1)

The `DATABASE_NEWER_VERSION` open failure becomes:

> <name> was last used by a newer version of HoploDex, which upgraded it.
> Update HoploDex to open it. The file has not been changed. A backup saved
> before that upgrade opens in this version: look for one whose name ends in
> “before” and a version number, in <backupFolder>.

Without `details.backupFolder`, the folder is "the “HoploDex backups” folder
next to the database, or the backup location you chose for it".

## 9. Policy names (FR-008)

The policy form shows the backend's `fieldErrors.name` at its **Name** field,
in the form's standard field-error pattern, with focus moved to the field, as
for every field error. No other field changes. The form's hint for **Name**
gains: "Each policy needs its own name."

## 10. Screenshot walk additions (`e2e/screenshots/screens.e2e.ts`)

The walk sets `HOPLODEX_E2E_UPGRADE_PHASE_SECONDS` for the upgrade's screens,
so each phase stays up long enough to capture (research.md §14).

- the upgrade question (§1), from the stored 0.1.0 database (research.md
  §14);
- the progress dialog in each phase (§2);
- the "Save the backup somewhere else?" dialog (§3), with the backup location
  missing;
- the upgraded note (§5);
- the restore confirmation with the upgrade sentence (§6);
- the policy form with the duplicate-name error (§9).
