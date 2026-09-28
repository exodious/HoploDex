# Contract: Databases, Locking and Backups UI

The application's external interface is its screens. This contract fixes
what the user sees and can do for this feature. Shared components only
(constitution III): `Dialog`, `ConfirmDialog`, `TextField`, `Field`,
`Checkbox`, `Select`, `ProgressBar`, `Button`, `Toast`, `Disclosure`.
Requirement IDs refer to [spec.md](../spec.md). Wording in quotes is the
intended text. Small copy edits during implementation are fine, but the
facts each text states are required.

## 0. New and changed shared components

- **`PassphraseField`** (new, `src/components/`): a password input used
  wherever a passphrase is typed (open, take over, create, change, restore,
  save to keyring: nine fields in six places), so it goes in the shared set. Rules (FR-007):
  - `type="password"`, with a toggle button that reads "Show" ("Show
    passphrase" to assistive technology) and, while the passphrase shows,
    "Hide" ("Hide passphrase"), in the niter colour;
    `autocomplete="current-password"` or `"new-password"`;
    `spellcheck="false"`, `autocapitalize="off"`.
  - **Uncontrolled**: the value is read from the input's ref on submit, and
    the input is reset right after. It is never put in React state, context or
    any store.
  - It resets when `system:clear-passphrase-fields` arrives, whether or not
    a database is open.
  - With `strength` set, it shows the **strength hint** below: a five-step
    meter with a text label ("Very weak" … "Very strong"), zxcvbn's
    suggestion, and "Longer is stronger: several unrelated words make a good
    passphrase." The hint never blocks (FR-003). The zxcvbn module is loaded
    lazily (research §18).
  - Errors use the standard field-error slot. The minimum-length message is
    "Use at least 12 characters." The confirmation message is "The
    passphrases don't match." A new passphrase that is the current one:
    "Choose a passphrase different from the current one."
  - **Checked as typed** (`usePassphraseChecks`), wherever a passphrase is
    set (create, change): each keystroke reads the fields afresh and keeps
    only the verdict, never a value. The form's button is enabled only while
    every field is filled in, the new passphrase is long enough (and, for a
    change, differs from the current one) and the confirmation matches.
    Problems show after a 400 ms pause in typing and clear the moment they
    are put right. Any confirmation that differs from the passphrase is a
    mismatch, one a character short included: the pause is what lets the
    typing finish. The length message waits for the confirmation to be
    started. The same checks run again on submit.
- **`Dialog`**, while a database is open, has a lock button ("Lock now",
  lock icon, `title` "Lock now (Ctrl+L)") in its header beside the close
  button, since a dialog covers the top bar's (§4). It does what Ctrl/⌘+L
  does. It isn't shown while nothing is open, or where the close button
  isn't (a dialog that can't be dismissed).
- **Naming a database.** Where only the open database can be meant (its
  dialogs, notes and statements), the text says "the database" or "this
  database" and doesn't name it: the window's title does (§4), and a dialog's
  title uses its menu item's words. Where a sentence must name one (the
  chooser and its notices, the closing and fault screens, a database restored
  from the chooser, and a confirmation that replaces or deletes), the name is
  in curly quotes, as policy names and nicknames are: "HoploDex locked “Main
  collection”." Below, `<name>` in a sentence stands for the quoted name.
- **`ConfirmDialog`** gains an optional third action (`alternativeLabel`,
  `onAlternative`) for the save / discard / cancel question (§6). No screen
  builds its own three-button prompt.
- **`Menu`** (new, `src/components/`, on `@radix-ui/react-dropdown-menu`):
  the database menu (§4). It gives the menu roles and arrow-key navigation
  (WCAG 2.1 AA) that a popover of buttons does not.

## 1. Database chooser (FR-020, FR-021, FR-012, US2-3)

A full-window screen that replaces the app shell whenever no database is
open: at launch, after a lock or a close, and after a take-over. No
collection data, top-bar tabs or counts are shown.

- **Header**: the HoploDex brand. The theme toggle stays available.
- **Notices** (from `get_chooser_state` and `session:closed`), above the list,
  one line each, dismissible:
  - locked: "HoploDex locked <name>." (for `lockedByUser`, `idle`, `sleep`,
    `screenLocked`). Idle adds " after <n> minutes without use".
  - stopped operation: "The computer went to sleep while <a backup | the
    passphrase change | a restore | an import | an export> was running, so it
    was stopped. <name> is as it was before it started." For an import:
    "<n> rows were imported before it stopped. You can import the file
    again; rows already imported will be found as matches." For a deletion
    of all backups: "The computer went to sleep while the backups of <name>
    were being deleted, so it was stopped. <n> were deleted; the rest are
    still there. You can delete them in its Database settings."
    For a move of backups: "The computer went to sleep while the backups
    of <name> were being moved, so it was stopped. <n> are still in
    <folder>, and HoploDex no longer manages them there. The new backup
    location is kept." (FR-037)
  - backups left behind (a move cut short by a crash, research §22): "The
    backups of <name> were not all moved. <n> are still in <folder>, and
    HoploDex no longer manages them there."
  - In both, one backup reads "1 is still in <folder>, and HoploDex no
    longer manages it there."
  - pending changes lost: "Unsaved changes could not be kept when <name>
    locked."
  - backup failed: "<name> was not backed up: <the backup location is not
    available | there is not enough space there | the backup was
    interrupted | its file could not be reached>. Its changes will be backed up at the next close." with a
    **Change backup location…** action (for an unavailable or full
    location), which opens the database's backup settings (§7) right after
    the next successful open; once chosen, the notice reads "Its backup
    settings will open when you open <name>."
  - taken over: "<name> was taken over on another computer, so HoploDex
    stopped saving to it here and closed it."
- **Recent databases** list, most recent first. Each row shows the name
  (large), the folder (secondary text, middle-truncated with the full path in
  a tooltip and the accessible name), and "Opens without a passphrase on this
  computer" when the passphrase is saved. The selected row is the most recent
  at launch, or the database just closed or locked (FR-033).
  - **Unavailable** rows (file missing) are dimmed with "Not found at this
    location" and offer **Locate…** (file picker) and **Remove from list**.
  - Every row offers **Remove from list** in its overflow, with the note "The
    database file is not deleted." (FR-012, US2-5).
- **Selected row, passphrase not saved**: an inline form opens in the row, a
  `PassphraseField` labelled "Passphrase for <name>", focused, with **Open**
  (primary). Below it, a `Checkbox` **Remember on this computer** (off),
  shown as disabled with "Not available: this computer has no keyring
  service." when the keyring is unavailable (FR-019). Ticking it opens the
  FR-017 confirmation (§8) first, and the box stays unticked unless the user
  confirms.
- **Selected row, passphrase saved**: **Open** (primary), no prompt. After a
  lock the selected row shows the same (US6-8).
- **Opening** (SC-003, constitution IV): within 100 ms of **Open** being
  pressed (or Enter in the field), and before the command returns, the row
  shows its busy state: **Open** reads "Opening…" with the shared `Button`'s
  `pending` state (spinner, `aria-busy`), the field, the checkbox and the other rows
  are disabled, and the live region says "Opening <name>…". Key derivation
  alone takes about a third of a second, so the busy state is shown at once,
  not after a delay. The same applies to **Take over** after its
  confirmation. On failure the row returns to its normal state with the
  error below.
- Page actions: **Create a new database…** (§2) and **Open another database
  file…** (a native open dialog filtered to `*.hoplodex`, with an "All files"
  choice). The chosen file becomes the selected row and asks for its
  passphrase.
- **No databases known** (first run): the list is replaced by a short
  welcome: "HoploDex keeps your collection in an encrypted database file
  that only your passphrase opens." with the two page actions as large
  buttons.

### Open failures (inline, in the selected row)

| Code | Text | Actions |
|---|---|---|
| `PASSPHRASE_INCORRECT` | "That passphrase didn't open <name>. Either the passphrase is wrong, or the file isn't a HoploDex database or is damaged." | the field stays for another try (FR-006); **Restore from a backup…** when `backupsAvailable` |
| — with `savedPassphraseFailed` | "The saved passphrase no longer opens <name>. Enter its passphrase; the saved copy will be updated." | prompt shown |
| `DATABASE_IN_USE` | "<name> is open in another copy of HoploDex, on this computer or another one. Close it there first." | — |
| `DATABASE_NEWER_VERSION` | "<name> was last used by a newer version of HoploDex. Update HoploDex to open it. The file has not been changed." | — |
| `DATABASE_DAMAGED` | "<name> is damaged and can't be opened." | **Restore from a backup…** when `backupsAvailable` |
| `DATABASE_NOT_FOUND` | "<name> is no longer at this location." | **Locate…**, **Remove from list** |
| `DATABASE_UNREADABLE` | "HoploDex can't read <name>: it doesn't have permission to open the file, or the drive or network holding it isn't available. The file has not been changed." | the row stays selected so **Open** can be tried again |
| `DATABASE_OPEN_ELSEWHERE` | "<name> is marked as open on **<machineName>** since <local date and time>. It may still be open there, may not have been closed properly, or its latest changes may not have synced to this computer yet." | **Go back**, **Take over…** |

**Take over…** opens a destructive `ConfirmDialog` (US2-10, FR-032):
title "Take over <name>?"; description "Only do this if <machineName> no
longer has <name> open, or if it crashed. If it still has it open, or its
latest changes haven't synced here yet, those changes can be lost."; a
`PassphraseField` "Passphrase for <name>", focused, with the hint "Enter it
again to confirm the take-over."; confirm label "Take over". The passphrase
that found the marker was read once and cleared like any other (FR-007), and
nothing holds it while the user decides, so typing it again is part of the
confirmation. An empty field keeps the dialog open with "Enter the
passphrase to take it over." Confirming reads and clears the field, closes
the dialog, and resends the open with `takeOver: true`, the row showing its
busy state; a wrong passphrase is then refused in the row like any other.

### Storage that can't be reached while open (FR-032, research §6)

When a save fails with `DATABASE_UNAVAILABLE`, the form or dialog that was
saving shows the error in its standard error slot, keeping the input:
"HoploDex can't reach <path>. Nothing already saved was lost. Close the
database and open it again once the drive or network is back." It is not
the take-over notice, and the session stays open until the user closes it.
After that close, the chooser shows the backup-failed notice with the reason
"its file could not be reached", and the row is marked unavailable if the
file is still missing.

## 2. Create a new database (FR-003, FR-004, FR-008, FR-009, FR-024)

A `Dialog` (`size="lg"`), titled "Create a database". Fields, in the usual
`hd-form-grid` layout:

1. **Name**: `TextField`, default "My collection" (from `suggested.name`).
   Help: "This is also the file name."
2. **Folder**: a `TextField` holding the path (typed or pasted) plus a
   **Choose…** button (native folder picker). The default is
   `suggested.folder`. Help shows the resulting path: "Saved as
   <folder>/<name>.hoplodex".
3. **Passphrase**: `PassphraseField` with `strength`.
4. **Confirm passphrase**: `PassphraseField`.
5. **Backups** (read-only disclosure, FR-024), a short panel: "Backups are on.
   When <name> locks or HoploDex quits after a change, HoploDex saves a copy in
   <folder>/HoploDex backups, at most once a day, keeping the latest 5.
   Each backup holds the whole collection and opens with the passphrase you
   had when it was made. You can change this in the database settings."
6. **Acknowledgement** (FR-004): a required `Checkbox`: "I have stored this
   passphrase somewhere safe. If it is forgotten, nobody, including HoploDex,
   can open this database or recover the collection." The create button is
   enabled only once it is ticked.

Footer: **Cancel**, **Create database** (primary). Within 100 ms of
**Create database** being pressed it shows "Creating…" in the shared `Button`'s `pending` state, and the fields and **Cancel** are disabled until the command
returns (constitution IV). On success the new
database opens (the app shell appears), and the **disk-encryption note**
(§10) is shown once.

## 3. Unlocking after a lock (FR-033, US6-1, US6-2, US6-8)

Nothing special: the chooser (§1) with the locked database selected, its
passphrase field focused (or **Open** focused when the passphrase is saved),
and the "locked" notice. The whole collection tree was unmounted, so nothing
from it is in the DOM.

## 4. The database menu (top bar)

At the left of `hd-topbar__tools`: a button labelled **Database** with a
chevron (`aria-haspopup="menu"`), the same whichever database is open,
opening a `Menu`:

- **Lock now** (shortcut Ctrl+L / ⌘L, available from anywhere, including
  inside dialogs): the one way to close the database from here. It leads to
  the chooser, which opens it again or another one. (A separate close would
  do the same: every form that can hold unsaved input is a dialog, which
  covers the menu, so it could never ask the §6 question either.)
- separator
- **Database settings…** (§7)
- **Change passphrase…** (§8)
- **Restore from a backup…** (§9)
- separator
- **About databases and security**: opens the guide (§11)

Beside it, a lock icon button, "Lock now" (`aria-label`), for the one-click
lock; every dialog has one too (§0). None needs confirmation (FR-035).

An item ending in "…" asks for something more before it acts, as buttons
do across the app; **Lock now** acts at once and the guide only informs.

The open database's name is the window's title: "<name> — HoploDex" (the
name unquoted, as window titles are), and "HoploDex" while none is open. Its
file's full path is in Database settings (§7).

## 5. Closing screen (FR-027, SC-005)

On `session:closing` the app shell is replaced by a centred panel:
"Closing <name>…". When `backup:progress.showNow` is true, or the close is
still running 1 s after it started, it shows "Backing up <name>…" with a
determinate `ProgressBar` (bytes; `aria-valuenow`), and a **Skip this
backup** button (secondary) with the note "Its changes will be backed up
next time." The lock or quit waits for it (FR-027). The panel appears
within 100 ms of the close starting.

## 6. Unsaved changes when quitting (FR-010, US2-4a)

When the user quits (the window's close button, or quitting from the OS)
while a firearm or policy form has unsaved input, a `ConfirmDialog`
with a third action appears. Title: "Save changes to <label>?". Buttons:
**Save changes** (primary), **Discard changes** (alternative; destructive
style), **Cancel**. Saving runs the form's own submit. If it fails validation
the dialog closes and the form shows its errors, and nothing closes. A lock
never shows this (FR-033).

## 7. Database settings (FR-024, FR-026, FR-029, FR-034, FR-036, FR-038, FR-017)

A standard `Dialog` titled "Database settings", its description giving the
database file's full path ("The database file is <path>"), with three
sections in this order, each a titled fieldset. Settings apply with **Save** in the
footer, like every other form.

**Backups**
- `Checkbox` **Make automatic backups**
- **Keep the latest**: a number field (`hd-field--third`), with "backups"
  after it
- **Location**: shows the resolved path and "(next to the database)" for
  the default, plus **Change…** (folder picker) and **Use the default**. An
  unavailable custom location shows "Not available on this computer" (FR-027).
  A changed location is saved with **Save**, and its backups dealt with
  then (below).
- Statements (FR-029), as a short list:
  - "Each backup is a complete copy of the collection."
  - "A backup opens only with the passphrase you had when it was made."
  - "Firearms you delete stay in earlier backups until those backups are
    removed."
  - "Backups on the same disk as the database don't protect against losing
    that disk."
  - "Old backups are deleted securely, as far as this computer allows (see
    About databases and security)." The guide's name is a link that opens
    it (§11, FR-030).
- Actions: **Restore from a backup…** (§9) and **Delete all backups…**
  (destructive `ConfirmDialog`, "Delete all <n> backups of <name>? They are
  deleted securely where this computer supports it. This can't be undone.")

**Changing the location** (FR-026, US3-4a, US3-4b, research §22). **Save**
sends the backup settings first, and saves nothing more until they are
through, so the lock settings wait too:
- `BACKUPS_AT_OLD_LOCATION`: a `Dialog` titled "Backups at the old
  location" (`ExistingBackupsDialog`), over the settings: "<count> backups
  of the database (<size>) are in <old folder>.", then a `ChoiceCards` radio
  group labelled "What should happen to them?", **Move them to the new
  location** first and selected:
  - **Move them to the new location**: "They'll be in <new folder>."
  - **Leave them where they are**: "HoploDex will no longer list, restore
    or delete them. They still open directly as a database, and choosing
    <old folder> again makes them this database's backups again."
  - **Delete them**: "They're deleted securely, as far as this computer
    allows."

  Footer: **Change location** (primary) and **Cancel**. **Delete them**
  first asks FR-029's destructive `ConfirmDialog` ("Delete all <n> backups
  of <name>? They are deleted securely where this computer supports it.
  This can't be undone.") and returns to the question if that is cancelled.
  **Cancel** saves nothing: the question closes, the location goes back to
  the saved one, and the other fields keep their input.
- `OLD_BACKUP_LOCATION_UNAVAILABLE`: a `ConfirmDialog` (not destructive
  styling) titled "The old backup location isn't available": "<old folder>
  can't be reached from this computer, so any backups there can't be moved
  or deleted from here. If you continue, they stay there, and HoploDex no
  longer manages them." Confirm label **Continue**, which resends with
  **Leave**; **Cancel** as above.
- **While moving or deleting**, the question's body is replaced by a
  `ProgressBar` ("Moving the backups…" in bytes, "Deleting the backups…" in
  files), shown at once when `showNow` is set and otherwise after 1 s, as on
  the closing screen (§5). It can't be dismissed, so it has no close or lock
  button (§0).
- **Refused before anything is written**, shown in the question above its
  footer, the question left open so another choice can be made:
  `INSUFFICIENT_SPACE`: "Moving the backups needs <size> free in <new
  folder>; <available> is free. Nothing has been changed."
  `BACKUP_LOCATION_UNAVAILABLE`: "<new folder> isn't available. Nothing has
  been changed."
- **Afterwards**, the settings dialog closes as after any **Save**, and a
  `Toast` says what happened: "The backups were moved to <new folder>." /
  "The backups at <old folder> were deleted." / nothing extra for **Leave**.
  When some were left behind the result stays in the settings dialog
  instead, as a warning banner above its footer, since the location is
  saved: "<moved> backups were moved. <n> are still in <old folder>
  <because a backup with the same name is already in <new folder> | because
  <new folder> became unavailable | because there wasn't enough space
  there | because of an error>. HoploDex no longer manages them there."
  (for one: "1 backup was moved", "1 is still in", "manages it there").
- `BACKUPS_NOT_ALL_DELETED`: the question shows "<n> backups couldn't be
  deleted, so the backup location wasn't changed. They are still this
  database's backups:" (for one: "1 backup couldn't be deleted, … It is
  still this database's backup:") above a list of the paths, with **Close**
  alone in the footer, which returns to the settings with the location put
  back as for **Cancel**.

**Locking**
- `Checkbox` **Lock after a period without use**. When checked, **after**
  a `Select` of 1, 2, 5, 10, 15, 30, 60, 120 or 240 minutes (`hd-field--third`)
  is enabled. Help: "Also locks when the computer goes to sleep. Turning
  this off stops both." (FR-034, FR-037)
- `Checkbox` **Lock when the computer's screen locks** (off by default).
  Where unsupported, it is disabled with "Not available: this computer
  doesn't tell applications when the screen locks." (FR-038)
- Statement: "Locking closes the database: its data is cleared from memory,
  opened document copies are deleted, a backup is made if one is due, and
  it's released for other computers."
- When the passphrase is saved on this computer (FR-036): "Because the
  passphrase is saved on this computer, anyone using this computer account
  can reopen the database after it locks."

**This computer** (acts at once, not with **Save**)
- Shows the current state: "The passphrase is remembered on this computer:
  the database opens without asking for it." with **Forget saved
  passphrase**, or "Not remembered: the database asks for its passphrase each
  time it opens." with **Remember the passphrase on this computer…**, which
  opens the FR-017 confirmation (§8) with a `PassphraseField` "Passphrase" in
  it. A wrong one: "That isn't this database's passphrase."
  A passphrase that doesn't open the database keeps the confirmation open
  with the error on the field.
- Unavailable-keyring text as in §1.

## 8. Passphrase dialogs

**Remember-passphrase confirmation** (FR-017, US5-1): a `ConfirmDialog`
(not destructive styling, confirm label "Remember passphrase"), titled
"Remember the passphrase of <name>?" from the chooser and "Remember this
database's passphrase?" from the settings: "Anyone
who can use this computer account, or its keyring while it's unlocked,
will be able to open <name | the database> without knowing the passphrase. On a shared
account this defeats the passphrase. Locking will no longer need the
passphrase on this computer, though it still clears the collection from
memory, deletes opened document copies, and releases the database for
other computers."

**Change passphrase** (FR-015, FR-016, US4): a `Dialog` titled "Change
passphrase", with **Current
passphrase**, **New passphrase** (with strength) and **Confirm new
passphrase**, then the footer **Change passphrase**, enabled once the
fields pass the checks of §0. While it runs, the body
is replaced by a `ProgressBar` with its phase label ("Making a copy with the
new passphrase…", "Checking the new copy…", "Replacing the database…") and
the dialog cannot be dismissed. On completion the body says: "The passphrase
has been changed. The previous file was deleted securely, as far as
this computer allows (see About databases and security). Backups and copies
made before now still open with the old passphrase." When the old file
remains: "The previous file could not be deleted. It is at <path>, and it
opens with the old passphrase." `INSUFFICIENT_SPACE`: "Changing the
passphrase needs <size> free on the database's drive; <available> is free."

## 9. Restore from a backup (FR-028, US3-5, US3-6)

A `Dialog` (`size="lg"`) titled "Restore from a backup", or "Restore <name>
from a backup" for a damaged database restored from the chooser (US3-6),
where the text below names it for "the database":
1. **Choose a backup**: a radio list of `list_backups` entries, newest first
   ("25 September 2026, 14:30 — 212 MB"). An empty or unavailable folder
   says so, and names the folder: "There are no backups of this database in
   <folder>." / "The backup folder <folder> isn't available on this
   computer."
2. **Passphrase for this backup**: a `PassphraseField`, with the note "Enter
   the passphrase the database had on <date>. After restoring, it opens with
   that passphrase."
3. Statement: "Before restoring, the database is backed up as it is now, so
   you can undo this by restoring that backup." For a damaged database: "The
   damaged file will be kept next to it, renamed."
4. Footer: **Restore** (a destructive `ConfirmDialog` step: "Replace <name>
   with the backup from <date>?", described "The backup takes its place,
   after the database is backed up as it is now." or, for a damaged one,
   "The backup takes its place, and the damaged file is kept beside it.").

Refusals before anything is written (FR-028), shown in the dialog above
the footer, with the dialog left open:
- `INSUFFICIENT_SPACE`: "Restoring needs <size> free on <drive or folder>;
  <available> is free. Nothing has been changed."
- `BACKUP_LOCATION_UNAVAILABLE`: "The database can't be backed up
  first, because <the backup location is not available | there is not enough
  space there>. Nothing has been changed." with **Change backup location…**
  (opens §7's Backups section).
- A "before restoring" backup that fails or is stopped while running: "The
  database couldn't be backed up, so the restore was cancelled.
  Nothing has been changed."

A wrong passphrase for the backup is shown on its field: "That passphrase
didn't open the backup from <date>."

While it runs: progress as in §8 ("Copying the backup…", "Checking the
backup…", "Backing up the current database…", "Replacing the database…"),
and the dialog has no close button and ignores Escape. Afterwards the
database is open, and a notice says it now opens with the passphrase from <date> (US3-5).

## 10. Notes shown once in the collection

Both are dismissible banners at the top of the collection page, in the
existing notice style:
- **Disk encryption** (FR-008), after creation until dismissed: "Your
  collection is encrypted with your passphrase. For extra protection, also
  turn on your computer's disk encryption: BitLocker on Windows, FileVault on
  macOS, or LUKS on Linux." Link: **Why?** (guide §11), at the end of the
  sentence. Each note is only as wide as its text, which wraps at about 72
  characters, so no empty stretch of bar separates the text from its
  dismiss button.
- **Restored** (FR-028), once after a restore: "The database was restored
  from a backup and now opens with the passphrase it had on <date>." For a damaged
  database it adds "The damaged file was kept as <path>."
- **Backup opened directly** (research §9): "This is a backup of <backupOfName>
  made on <date>. Changes here aren't part of <backupOfName>. It's still in the
  backup folder, where it may be removed when older backups are cleared: move
  the file elsewhere to keep it."

## 11. Guide: "About databases and security" (FR-030)

A `Dialog` with headed sections, in the style of feature 002's origin guide:
passphrases (why length matters; there is no recovery; a copied file is
protected only by the passphrase); saving the passphrase on this computer;
locking; backups (where they go, what they hold, the passphrase they open
with, deleted records staying in older backups, same-disk risk, cloud-synced
folders keeping their own copies); secure deletion (best effort: SSDs,
journaling and copy-on-write filesystems, snapshots and cloud folders can
keep old data; an old copy is still protected by its old passphrase);
using a database from more than one computer (one at a time, the open marker,
take-over); and whole-disk encryption. The text states the defaults (5
backups kept, a HoploDex backups folder next to the database, a lock after
10 minutes, no lock at a screen lock). With a database open, the locking
and backup sections each end with "How this database is set up": each setting's
value with its default beside it, "10 (default: 5)", or "(default)" when
unchanged, a changed row marked at its left. It is reachable from the database menu,
the disk-encryption note, and "see About databases and security" links in
the change-passphrase and backup texts.

## 12. Export dialog wording (FR-031)

The export dialog's description and any "backup" wording become: "Exports
the collection to a spreadsheet. The file is **not encrypted**: anyone who
can open it can read it. For encrypted backups of the whole database,
see Database settings." The feature 001 SC-005 wording is amended to match.

## 13. Pending changes at open (FR-039, US6-6)

After an open that reports `pendingChanges`, and before the collection can
be used, a non-dismissable `Dialog`: "Unsaved changes to <label>". Body:
"The database locked on <date, time> while you were editing <label>. Your changes
were kept." Buttons: **Resume editing** (primary): navigates to the record
or policy and opens its form with the draft as unsaved input. **Discard
changes**: a destructive `ConfirmDialog`. When `resumable` is false, the body
adds "<label> no longer exists, so these changes can only be discarded." and
only **Discard changes** is offered.

A draft's values come from the database, perhaps from another computer, so
the form takes only the fields it has, and only those of the same kind (a
string for a string); every other field starts as it would without the
draft. A draft short of fields still opens.

**A screen that fails to render** never leaves a blank window. Inside an
open database the collection is replaced by a full-window panel laid out like
the closing screen (§5): "<name> couldn't be shown", saying that everything
saved is safe and unsaved changes are lost, with **Close the database**
(focused), a normal close. The session stays mounted, so the window's close button
still quits. Anything else that fails shows "HoploDex stopped working" with
**Quit HoploDex**, which, like the window's close button, quits through
`quit_application`.

## 14. Accessibility and focus

- The chooser, closing screen and pending-changes dialog set focus on their
  primary control, and announce their message through an `aria-live="polite"`
  region.
- The lock notice and all errors are text, never colour alone.
- Ctrl/⌘+L works from any focus. When a lock replaces the view, focus moves
  to the chooser's passphrase field (or its **Open** button).

## 15. Screenshot walk additions (`e2e/screenshots/screens.e2e.ts`)

New stable names, each in light and dark: `14-chooser`, `15-chooser-first-run`,
`16-create-database`, `17-open-elsewhere`, `18-closing-backup`,
`19-database-settings`, `20-change-passphrase`, `21-restore-backup`,
`22-pending-changes`, `23-database-guide`, `24-disk-encryption-note`,
`25-unsaved-changes` (the save / discard / cancel prompt of §6, which adds a
third action to the shared `ConfirmDialog`), `26-database-guide-settings`
(the guide scrolled to "How this database is set up", §11),
`27-backup-location-change` (the move / leave / delete question of §7).
Existing
screens are unchanged apart from the database menu in the top bar and the
export wording.
