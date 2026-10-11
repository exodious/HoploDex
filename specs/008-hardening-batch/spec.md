# Feature Specification: Session Isolation, Safe Import, Browser Controls and Pending Changes from Another Version

**Feature Branch**: `008-hardening-batch`

**Created**: 2026-10-10

**Status**: Draft

**Input**: User description: "#66, #69, #40, #78". Four GitHub issues taken together: #69, collection data and unfinished work in the interface outliving the database they belong to (security, Medium); #66, reading a spreadsheet for import with no limits, so that a crafted file can end the application (security, Medium); #40, the web view's own context menu and its reload and navigation shortcuts in the main window (polish); and #78, pending changes kept by another version of HoploDex, which can only be discarded and don't say which version kept them (enhancement). The issues and the decisions recorded on them are reproduced under [Source Request](#source-request) so this spec stands on its own.

## Clarifications

### Session 2026-10-10

- Q: Should the menu kept for text fields and selected text be the operating system's own editing menu as it comes (which on macOS also offers Look Up, Translate, Search With Google and Share for selected text), or trimmed to editing and copying items only? → A: Trimmed. It offers only editing and copying (cut, copy, paste, delete, select all, undo and redo where the system has them) and the spelling suggestions and emoji the system offers for editing; nothing that sends the text to another program or the internet (Look Up, Translate, web search, Share, Services, Speech), and none of the web view's own items (FR-011).

### Session 2026-10-11

- Q: If the user chooses "Resume editing" but the record's form can't open while the database is still open, should the pending changes dialog come back at once, or should the user carry on with the changes kept only for the next open? → A: The dialog comes back at once. The user is told the form couldn't open, the pending changes dialog is shown again (Resume editing, Close the database, Discard changes), and the collection stays blocked until they choose; if the session ends first, the changes are offered at the next open (FR-003).
- Q: Should the backend refuse only changes begun in a session that has since ended, or reads too? → A: Reads too. Every request about the collection, read or change, names the session it was begun in, and the trusted side refuses any request of an ended session, changing nothing and returning nothing (FR-004).
- Q: When a session ends, should every notification raised within its collection be removed, or only those whose text names collection content? → A: Every notification raised within the collection belongs to its session and is removed when it ends, and late ones from that session are never shown; notices from the session layer and the database chooser (lock, close, backup, take-over) are kept (FR-005).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Each Database's Data Stays With Its Own Session (Priority: P1)

A collector keeps two databases, their own collection and one they manage for a relative. They look at a firearm's photos in their own database, lock it, and open the relative's. Every photo, name, field value, notification and file name they see belongs to the relative's database; nothing from their own collection appears, even where the two databases happen to give a record or a photo the same internal number. Nothing they started in their own database (adding a batch of photos, a draft they chose to resume, a document being loaded) finishes inside the relative's.

**Why this priority**: It is a disclosure between databases that needs no attacker and no race: today, viewing photo *n* in one database and then opening another with a photo *n* shows the first database's thumbnail in the second (#69). A database is a deliberate boundary (feature 003: each has its own passphrase, and a lock clears its data from memory, 003 FR-033), and the interface must honour it as the backend does. A late write landing in the wrong database also corrupts records.

**Independent Test**: Create two test databases whose firearms, accessories, photos and documents have the same internal identifiers; in the first, view thumbnails, start a batch of attachments, resume a draft whose record fails to load, and raise notifications; lock or switch to the second; confirm that nothing of the first is shown in the second and nothing begun in the first is written to the second.

**Acceptance Scenarios**:

1. **Given** database A with a photo shown as a firearm's thumbnail, and database B whose own photo has the same internal identifier, **When** the user locks A (or closes it, switches, or restores a backup) and opens B, **Then** B's list shows B's own thumbnail, never A's.
2. **Given** a thumbnail, photo or document of A still loading when A is locked or closed, **When** it finishes loading after B is open, **Then** it is thrown away: it is neither shown in B nor kept for later.
3. **Given** the user chose "Resume editing" for A's pending changes and the record's form did not open (for example, the record failed to load), **When** A is locked, closed or switched and B is opened, **Then** no form in B is filled with A's changes, and A's changes are offered again the next time A is opened.
4. **Given** the user chose "Resume editing" for A's pending changes and the record's form did not open, **When** A is still open, **Then** the user is told the form couldn't open and the pending changes dialog is shown again at once (Resume editing, Close the database, Discard changes); the collection can't be used until they choose, as while any pending changes wait.
5. **Given** the user is adding several photos or documents to a record in A, **When** A is locked, closed or switched part-way through, **Then** the files not yet added are not added anywhere, files already added stay in A, and nothing is added to any record of B even if B has a record with the same internal identifier.
6. **Given** notifications raised within A's collection are showing (whether or not they name its records or files), **When** A is locked, closed or switched, **Then** they are removed at once, a notification produced afterwards by work begun in A is never shown, and the notice saying why A's session ended (for example an idle lock or a backup failure at close) is still shown.
7. **Given** any request to read or change the collection that was begun while A was open, **When** it reaches the application after A's session ended (A closed, locked, switched, restored, or reopened since), **Then** the application refuses it, changing nothing and returning nothing, whichever database is open then.

---

### User Story 2 - Import a Spreadsheet Without Risk to the Application (Priority: P2)

A collector is given a spreadsheet of firearms by someone else, or downloads one, and imports it. Whatever the file contains, HoploDex either reads it or says plainly why it can't, and the collector can cancel at any point while it is being read. A file built to exhaust the computer's memory, or to crash the program that reads it, is refused; it never ends HoploDex, never makes the computer unresponsive, and never stops the database from locking.

**Why this priority**: Importing is the one place HoploDex reads a complex file the user may have received from someone else. Today a crafted workbook can end the application outright (a size declared inside the file is trusted, #66), and a large or highly compressed one can exhaust memory while the database is held, so a lock or sleep waits on it. It needs the user to be persuaded to import the file, so it ranks after Story 1, which needs nothing.

**Independent Test**: Import a set of crafted and oversized files (a workbook declaring an impossible number of shared strings, a highly compressed workbook that expands enormously, files with millions of rows, very wide rows, huge cells and hundreds of sheets, a CSV with one enormous field); confirm each is refused with a message naming what is wrong, that the application keeps running and its memory use stays bounded, that cancelling stops the reading promptly, and that a lock during the reading isn't held up. Then import the application's own export of a full-size collection and confirm it still imports.

**Acceptance Scenarios**:

1. **Given** a workbook whose internal data declares a count or size far beyond what it holds (for example an impossible number of shared strings), **When** the user imports it, **Then** it is refused as unreadable, with a message naming the file, and the application keeps running.
2. **Given** a file larger than the import size limit, or one whose content expands beyond the limit when unpacked, **When** the user imports it, **Then** it is refused before it is fully read, with a message naming the file and saying it is too large to import, and nothing is imported.
3. **Given** a file with more rows, more columns, more sheets or longer cell text than the import limits allow, **When** the user imports it, **Then** it is refused with a message naming the file (and the sheet, for a workbook) and which limit it passed, and nothing is imported.
4. **Given** a file is being read for import, **When** the user cancels, **Then** the reading stops promptly, nothing is imported, and the user is told the import was cancelled.
5. **Given** a file is being read for import, **When** the user locks the application, the idle lock is reached, or the computer goes to sleep or shuts down, **Then** the lock or close goes ahead without waiting for the reading to finish, and the import is stopped as a cancelled one.
6. **Given** the application's own export of a collection at the largest supported size (10,000 firearms and 10,000 accessories, with every text field at its longest), as one workbook or two CSV files, **When** the user imports it, **Then** it is read within the limits and imports as today.

---

### User Story 3 - No Browser Controls in the Main Window (Priority: P3)

A collector right-clicks a blank area, a row in the collection or a button, and nothing happens, as in any desktop application. Right-clicking a text field still offers cut, copy and paste, and right-clicking selected text still offers copy. Pressing F5 or Ctrl+R (⌘R on a Mac) by habit, or the mouse's back button, no longer reloads the window and throws away the half-filled form they were typing into.

**Why this priority**: Reloading is reachable by accident from the keyboard, the mouse and the context menu, and loses whatever the user has typed into an open form. The browser menu's other items (Back, Forward, Print, Save as) have no place in the application. It affects everyday use but discloses nothing, so it ranks after the two security issues.

**Independent Test**: In a release build on each operating system, right-click a blank area, a table row, a button, a text field and a selection; press each reload and navigation key and the mouse's back and forward buttons with a form open and input typed; confirm no browser menu appears except where editing or copying, the window never reloads or navigates, and the typed input is still there.

**Acceptance Scenarios**:

1. **Given** the main window of a release build, **When** the user right-clicks a blank area, a table row, a card, a button or an image, **Then** no menu opens.
2. **Given** a text field or text area, **When** the user right-clicks it, **Then** a menu offering cut, copy, paste and select all opens, with the system's spelling suggestions and emoji where it offers them, and nothing else.
3. **Given** text selected anywhere in the window, **When** the user right-clicks, **Then** a menu offering copy opens, and nothing that sends the text elsewhere (on macOS, no Look Up, Translate, Search With Google or Share).
4. **Given** a form open with input typed, **When** the user presses F5, Ctrl/⌘+R, Ctrl/⌘+Shift+R, Ctrl+F5, Alt+← or Alt+→ (⌘[ or ⌘] on a Mac), the keyboard's Back or Forward key, or the mouse's back or forward button, **Then** the window neither reloads nor navigates, and the typed input is unchanged.
5. **Given** a release build, **When** the user presses the web view's own shortcuts for printing the page, saving the page, viewing its source, finding in the page or opening developer tools, **Then** none of them acts; the application's own shortcuts ("/" to search, Ctrl/⌘+L to lock, Escape to close a dialog) and the editing shortcuts (cut, copy, paste, undo, redo, select all) work as before.
6. **Given** a development build, **When** the developer right-clicks or presses a reload key, **Then** the web view's own menu and reload work, so inspecting elements and reloading during development still work.

---

### User Story 4 - Pending Changes Kept by Another Version (Priority: P4)

A collector uses HoploDex on two computers. On the first, running HoploDex 1.2.0, the database locks while they are part-way through editing a firearm, and their input is kept as pending changes. They open the database on the second computer, which runs a different version whose firearm form has changed. HoploDex tells them their changes were kept by HoploDex 1.2.0 and can only be resumed there, and lets them close the database without discarding anything. Back on the first computer they open it and resume editing.

**Why this priority**: Today the user's only way past the dialog is to throw their input away, though the version that kept it could still resume it, and nothing tells them which version that was (#78). It costs the user typed input, but only when two versions of HoploDex share a database, which is rare before the first release; so it ranks last.

**Independent Test**: With a test database whose pending changes were kept by a version (or a form version) different from the running one, open it and confirm the dialog names the version that kept them, offers closing the database and discarding, and no resume; close the database and confirm the pending changes are still there at the next open; open it with the matching version and confirm they resume.

**Acceptance Scenarios**:

1. **Given** a database whose pending changes were kept by a HoploDex version whose form for those changes differs from this version's, **When** the user opens it, **Then** the pending changes dialog says which version kept them (for example "They were kept by HoploDex 1.2.0. Open the database with that version to resume them, or discard them here.") and offers **Close the database** and **Discard changes**, but not **Resume editing**.
2. **Given** that dialog, **When** the user chooses **Close the database**, **Then** the database is closed normally (with its automatic backup if one is due, which, as always, does not contain the pending changes) and the chooser is shown; the pending changes are left exactly as they were.
3. **Given** the database was closed that way, **When** it is next opened by the version that kept the changes, **Then** they are offered with **Resume editing**, and resuming fills the form with them.
4. **Given** that dialog, **When** the user chooses **Discard changes**, **Then** they are asked to confirm first, as today, and the changes are removed only if they confirm.
5. **Given** a database whose pending changes can be resumed (the same form version), or whose record no longer exists, **When** the user opens it, **Then** the dialog also offers **Close the database**, which leaves the changes in place as in scenario 2.
6. **Given** a database whose pending changes were kept by a version whose form is the same as this one's, **When** the user opens it with this version, **Then** the changes can be resumed, as today, whichever version kept them.

---

### Edge Cases

- **The same database closed and reopened** (a lock and unlock, or a restore from a backup): it is a new session. Thumbnails, drafts, notifications and unfinished work of the earlier session are not carried into it any more than into another database (FR-001); what is shown is loaded again from the database.
- **Work that finishes during the close itself** (an attachment that completes while the closing screen is shown): either it is written before the close (it was accepted before the session ended) or it is refused (FR-004); it is never written after a new session has begun.
- **A slow photo or document load and a fast switch**: the load's result is dropped when it arrives; the new session's list loads its own (FR-002).
- **A resumed draft for a record deleted meanwhile on another computer** (a take-over between the dialog and the form opening): the form can't open; the user is told, and the draft is kept as pending changes to be offered, and discarded, at the next open (FR-003).
- **A workbook that is a valid archive but not a spreadsheet**, or a spreadsheet saved in an older or unusual format: refused as unreadable, as today, with the file named.
- **A file that is within every limit but slow to read** (for example a large workbook on slow storage): the reading shows progress, can be cancelled, and doesn't hold up a lock (FR-007, FR-008).
- **A file of two tables where one table is within the limits and the other is not**: the whole import is refused, so it never imports one table without the other (FR-006).
- **A file on a network share or removable drive that disappears while being read**: refused as unreadable, with the file named, as any read failure.
- **Right-clicking a text field that is read-only or disabled**: the copy menu is offered when it has selected text (FR-011); nothing otherwise.
- **Right-clicking inside the PDF preview**: unchanged from feature 007 (the PDF surface already removes its viewer's menu items that don't belong); this feature is about the main window.
- **Dragging a link or text onto the window**: out of scope here; the window's drop handling is as today (files only, CLAUDE.md "File drops"), and #73 covers the web view's navigation.
- **Pending changes kept by a version that recorded no version** (a database made before this feature): the dialog says "another version of HoploDex", as today, still offering **Close the database** (FR-016).
- **Pending changes kept by a different build of the same version number** (a development build whose form version differs): the dialog says they were kept by "a different build of HoploDex <version>".
- **A newer version kept the changes**: the dialog names it in the same way; the user may need to update HoploDex on this computer to resume them. Whether the newer or the older version, the wording names it without guessing which is newer.
- **Closing from the pending dialog when the computer can't make the backup** (the backup folder unreachable): the close behaves as any normal close does then (003 FR-027); the pending changes are untouched either way.

## Requirements *(mandatory)*

### Functional Requirements

**Each database session owns its data and its work (#69)**

- **FR-001**: A database session lasts from opening or unlocking a database until it is closed, locked (by any means), switched, restored from a backup or taken over by another computer. When a session ends, the interface MUST at once forget everything of that session's collection it holds outside what is on screen: images (thumbnails, photos), document content, record names and field values, file names, resumed drafts and the collection's notifications (FR-005). This extends to the interface what 003 FR-033 requires of the collection's data in memory. Nothing kept by one session may be shown in or used by another, whether of the same database or another.
- **FR-002**: Any data loaded for a session (an image, a list, a record, a document, a notification) that arrives after the session ended MUST be discarded without being shown, kept or cached. Wherever the interface keeps loaded data for reuse within a session, it MUST be tied to that session, so that two databases (or two sessions of one) whose records, photos or documents share an internal identifier can never be confused.
- **FR-003**: Pending changes the user chose to resume (003 FR-039) MUST fill only the form they belong to, in the session in which they were resumed. Until that form has opened with them, they MUST NOT be lost: they stay the database's pending changes. If the form can't be opened while the session lasts (the record fails to load, for example), the user MUST be told and the pending changes dialog MUST be shown again at once, with the collection blocked until the user resumes, closes the database or discards (003 FR-039, FR-017), so the kept input can't go stale behind edits made meanwhile. If the session ends before the form opens, they are offered again at the next open of that database (clarification of 2026-10-11).
- **FR-004**: Every request the interface makes about a database's collection, whether it reads (a list, a record, an image, a document) or changes it, MUST name the session in which it was begun and MUST be served only in that session. The application's trusted side MUST refuse, changing nothing and returning nothing of any database, every such request made on behalf of a session that has ended, whichever database is open when it arrives (clarification of 2026-10-11). Work that makes several changes in turn (adding several photos or documents, for example) MUST stop at the end of its session: what was added before stays, and nothing more is added anywhere.
- **FR-005**: Every notification raised within a session's collection belongs to that session, whatever its text: it MUST be removed when that session ends, and any notification produced afterwards by that session's work MUST NOT be shown. Notices raised by the session layer or the database chooser (about a lock, a close, a backup or a take-over) are not the collection's and are kept, so the user still sees why the session ended (clarification of 2026-10-11).

**Reading a spreadsheet for import (#66)**

- **FR-006**: Reading a file for import (001 FR-019, 006 FR-022) MUST be bounded by limits on: the file's size; the size of its content once unpacked; the number of sheets; the number of rows in a table; the number of columns in a row; and the length of the text in a cell. A file that passes any limit MUST be refused as a whole, before anything is imported, with a message naming the file (and the sheet, for a workbook) and which limit it passed. The limits MUST be well above anything the application's own export writes for a collection of the largest supported size (10,000 firearms and 10,000 accessories, every text field at its longest, constitution IV) so that such an export always imports.
- **FR-007**: No file, however crafted, may end the application, exhaust the computer's memory, or make the application unresponsive. Sizes and counts declared inside a file MUST NOT be trusted to size what the application sets aside to read it: a file whose declared sizes don't match its content, or that can't be read within the limits, MUST be refused as unreadable with the file named. This holds for every format import accepts, including parts of a file read by third-party components.
- **FR-008**: The reading of a file MUST be cancellable from the moment it starts, stopping promptly when the user cancels (SC-004), and MUST NOT hold up the database: while a file is being read, a lock (by any means), sleep, shutdown or quitting goes ahead without waiting for the reading to finish, and the import is then stopped as cancelled. Progress MUST be shown while the file is read, as it is while rows are imported (constitution IV).
- **FR-009**: Rows already validated and imported are unchanged in behaviour: per-row errors, conflicts, matching and the imported result stay as 001, 002, 004, 005 and 006 define them. Only files that pass a limit or can't be read are refused as a whole.

**Browser controls in the main window (#40)**

- **FR-010**: In a release build, right-clicking in the main window MUST open no menu, except as FR-011 says. In particular, the web view's own navigation, reload, print, save, view-source and inspect items MUST never be offered.
- **FR-011**: Right-clicking a text field, a text area or other editable text MUST open a menu for editing it (at least cut, copy, paste and select all, where each applies), and right-clicking while text is selected MUST open a menu offering copy. These menus MUST offer only editing and copying (cut, copy, paste, delete, select all, undo and redo, where each applies and the system has it) and the spelling suggestions and emoji the operating system offers for editing. They MUST NOT offer anything that sends the text to another program or the internet (Look Up, Translate, web search, Share, Services, Speech and the like), nor any of FR-010's items (clarification of 2026-10-10).
- **FR-012**: In a release build, the keys and buttons that reload the window or move through its history MUST do nothing: F5, Ctrl/⌘+R, Ctrl/⌘+Shift+R, Ctrl+F5 and Shift+F5, Alt+← and Alt+→, ⌘[ and ⌘] on macOS, the keyboard's Back, Forward and Refresh keys, and the mouse's back and forward buttons. The web view's own shortcuts for printing the page, saving it, viewing its source, finding in it and opening developer tools MUST do nothing too. Typed input in any open form MUST survive every one of them.
- **FR-013**: The application's own shortcuts and the editing shortcuts (cut, copy, paste, undo, redo, select all, and moving and selecting within text) MUST keep working everywhere they work today, in fields and dialogs included.
- **FR-014**: A development build MUST keep the web view's own menu and reload, so that inspecting the interface and reloading it during development work as today.

**Pending changes kept by another version (#78)**

- **FR-015**: Each database MUST record, inside it, the version of HoploDex that last saved it. Saving means any change to the collection, the database's settings or its pending changes, so keeping pending changes at a lock or shutdown (003 FR-039) records the version that kept them. Opening, closing, locking without unsaved input, making a backup and marking which computer has the database open MUST NOT change it, so that opening a database with another version and closing it without saving anything leaves the record naming the version that last saved.
- **FR-016**: When pending changes can't be resumed because their form differs from this version's (003 FR-039), the pending changes dialog MUST name the version recorded under FR-015 ("They were kept by HoploDex 1.2.0"), say that the changes can be resumed by opening the database with that version, and say that discarding removes them. Where the recorded version is this version's number but the form differs, it MUST say a different build of that version kept them; where no version is recorded, it MUST say another version of HoploDex kept them. Whether the changes can be resumed MUST still be decided by the form's own version, so changes kept by another version whose form is the same are resumed as today.
- **FR-017**: The pending changes dialog MUST offer **Close the database** in every case (resumable, kept by another version, or record no longer exists). It MUST close the database as a normal close does (003 FR-010, with the automatic backup if one is due, FR-025), show the chooser, and leave the pending changes exactly as they were, so that the next open, by any version, offers them again. The backup made by that close MUST NOT contain the pending changes, as no backup does. **Discard changes** MUST still ask to confirm first.

### Key Entities

- **Database session**: the time a database is open, from opening or unlocking it until it is closed, locked, switched, restored or taken over (FR-001). Every piece of collection data the interface holds, and every request it makes to read or change the collection, belongs to one session (FR-002, FR-004). It is never stored.
- **Last-saved version**: inside each database, the version of HoploDex that last saved it (FR-015). Set whenever the database is saved, including when pending changes are kept; untouched by opening, closing, locking without unsaved input and backups. Read when pending changes from another version are offered (FR-016).
- **Import limits**: the bounds on a file read for import (FR-006): its size, its size once unpacked, its sheets, rows, columns and cell text. Fixed in the application, not set by the user.
- **Pending changes** (003 FR-039, unchanged in what they hold): now also left untouched when the user closes the database from their dialog (FR-017), and kept until the resumed form has actually opened with them, so a form that can't open brings their dialog back (FR-003).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Across scripted two-database scenarios in which every internal record, photo and document identifier of one database also exists in the other (thumbnails viewed, loads delayed past a lock, a resumed draft whose record fails to load, a batch of attachments interrupted, notifications showing), switching, locking, restoring or reopening shows none of the first database's content in the second, applies none of its changes there, and answers no request begun in the first session with the second's data: zero cross-session items in every scenario.
- **SC-002**: Across a test set of crafted and oversized import files (impossible declared counts, a compressed workbook that expands at least a thousandfold, more rows, columns, sheets or cell text than the limits, a CSV with one enormous field, truncated and corrupted files), the application never ends, each file is refused with a message naming it within 5 seconds of the import starting, and the application's memory use during the reading of any one file rises by no more than 512 MB.
- **SC-003**: The application's own export of a collection of 10,000 firearms and 10,000 accessories, with every text field at its longest, as a workbook and as two CSV files, imports without being refused by any limit.
- **SC-004**: Cancelling the reading of a file, or locking the application while a file is read, takes effect within 1 second, at any point during the reading.
- **SC-005**: On each supported operating system, in a release build, right-clicking each of a blank area, a table row, a button, an image and a dialog opens no menu; right-clicking a text field and selected text opens the editing or copy menu, with no item outside FR-011's list; and with input typed into a form, each key and button of FR-012 leaves the window unreloaded and the input intact: 100% of the cases tested.
- **SC-006**: A database whose pending changes were kept by another version can be opened, closed from the pending changes dialog and reopened by the version that kept them, which resumes the changes with every field as it was kept: no typed input lost, and the dialog names the version in 100% of the cases where one is recorded.

## Assumptions

- **One feature for four issues**: the four issues are specified together at the owner's request. They touch separate parts of the application and each story can be built and tested alone; the plan may order them freely.
- **Unreleased application**: recording the last-saved version (FR-015) adds to the database's layout, which is changed in place (CLAUDE.md). Databases made before this feature must be created again; nothing is migrated, and the developer's own databases are not touched.
- **The version is recorded on save, not on open**: the owner's note on #78 asks for the version to be kept in the database, independent of the pending changes. Recording it only when something is saved (FR-015) keeps it naming the version that kept the pending changes, since nothing else can be saved while they wait (003 FR-039). Showing the last-saved version elsewhere (for example in the chooser or the database settings), or warning when a newer version last saved a database, is left to the post-1.0 versioning policy (#24).
- **Whether pending changes resume** stays decided by the form's version, not the application's (FR-016): a newer HoploDex whose form is unchanged can still resume them, as today.
- **"Close the database" in every case** (FR-017): #78 suggests it may also suit the case where the record no longer exists. It is offered there too, so the dialog never forces a decision the user isn't ready to make; discarding is still the only way to clear changes no version can resume.
- **Import limits are fixed**: their values are chosen at planning time against the largest supported collection (FR-006, SC-003) and are not a setting. How the reading is protected (for example correcting or replacing the component that reads workbooks, or reading the file in a separate confined process, as the TIFF preview does, 007) is decided at planning time; a choice that adds a native dependency or a separately built program is offered to the owner first.
- **Browser shortcuts beyond #40's list**: printing, saving the page, viewing source, find in page and developer tools (FR-012) act on the window itself, have no place in the application, and in the case of print and save-as are among the items #40 names in WebView2's menu; they are turned off with the reload keys. The window's zoom stays as it is today.
- **The kept menu is trimmed on each operating system** (FR-011): removing items from the web view's own menu takes work specific to each system, as feature 007's PDF surface did; where a system won't let an item be removed, the plan says so and the owner decides before that system's build ships it.
- **Development builds** keep the web view's menu and reload (FR-014), as #40 proposes.
- **Testing**: the context menu itself can't be opened by the end-to-end tests' input, so the menu's absence is asserted on the events the window receives and checked by hand on each operating system, as #40's acceptance says; the reload and navigation keys are covered by end-to-end tests. The two-database scenarios (SC-001) and the crafted files (SC-002) are automated tests on real databases (constitution II).
- **Security review**: FR-001 to FR-008 change how collection data and an untrusted file are handled; the plan records how they meet the Security & Data Handling Constraints, and the pull request carries the security review the project requires.

## Relationship to Earlier Features

- **003** (`specs/003-database-protection-management/`): FR-033's "collection data … cleared from memory" is extended to what the interface holds (FR-001 to FR-005). FR-039 (pending changes) gains the last-saved version and the dialog's **Close the database** (FR-015 to FR-017); `contracts/ui-databases.md` §13 is amended. The database's recorded state (its key entities) gains the last-saved version.
- **001** and **006**: import (001 FR-019, FR-020; 006 FR-022) is bounded by limits, cancellable while reading, and no longer holds up a lock while reading (FR-006 to FR-009). Row-level behaviour is unchanged.
- **007** (`specs/007-document-preview/`): the PDF surface's own menu and key handling are unchanged; this feature does the same for the main window (FR-010 to FR-014), as the comment on #40 records.

## Source Request

This feature was filed as four GitHub issues, specified together at the owner's request (`/speckit-specify #66, #69, #40, #78`). Each is reproduced verbatim, with the comments recorded on it and where this spec answers it.

### #69: "[Medium] Frontend: plaintext state and pending operations survive database boundaries" (label: security)

> **Severity:** Medium
> **Area:** Frontend (React)
> **Commit:** 4e9e00b339fa37d3ff57f20212f35661891c9a04
> **Location(s):**
>
> - [`src/features/browse/FirearmThumbnail.tsx:L12-L79`](https://github.com/exodious/HoploDex/blob/4e9e00b339fa37d3ff57f20212f35661891c9a04/src/features/browse/FirearmThumbnail.tsx#L12-L79)
> - [`src/features/session/usePendingDraft.ts:L161-L211`](https://github.com/exodious/HoploDex/blob/4e9e00b339fa37d3ff57f20212f35661891c9a04/src/features/session/usePendingDraft.ts#L161-L211)
> - [`src/features/session/SessionProvider.tsx:L54-L57`](https://github.com/exodious/HoploDex/blob/4e9e00b339fa37d3ff57f20212f35661891c9a04/src/features/session/SessionProvider.tsx#L54-L57)
> - [`src/features/session/SessionProvider.tsx:L147-L155`](https://github.com/exodious/HoploDex/blob/4e9e00b339fa37d3ff57f20212f35661891c9a04/src/features/session/SessionProvider.tsx#L147-L155)
> - [`src/features/session/SessionProvider.tsx:L263-L274`](https://github.com/exodious/HoploDex/blob/4e9e00b339fa37d3ff57f20212f35661891c9a04/src/features/session/SessionProvider.tsx#L263-L274)
> - [`src/features/app/AppShell.tsx:L65-L101`](https://github.com/exodious/HoploDex/blob/4e9e00b339fa37d3ff57f20212f35661891c9a04/src/features/app/AppShell.tsx#L65-L101)
> - [`src/features/firearms/FirearmRecordPage.tsx:L83-L118`](https://github.com/exodious/HoploDex/blob/4e9e00b339fa37d3ff57f20212f35661891c9a04/src/features/firearms/FirearmRecordPage.tsx#L83-L118)
> - [`src/features/media/DocumentList.tsx:L60-L99`](https://github.com/exodious/HoploDex/blob/4e9e00b339fa37d3ff57f20212f35661891c9a04/src/features/media/DocumentList.tsx#L60-L99)
> - [`src/features/media/PhotoGallery.tsx:L64-L108`](https://github.com/exodious/HoploDex/blob/4e9e00b339fa37d3ff57f20212f35661891c9a04/src/features/media/PhotoGallery.tsx#L64-L108)
> - [`src/App.tsx:L14-L21`](https://github.com/exodious/HoploDex/blob/4e9e00b339fa37d3ff57f20212f35661891c9a04/src/App.tsx#L14-L21)
> - [`src/components/Toast.tsx:L15-L29`](https://github.com/exodious/HoploDex/blob/4e9e00b339fa37d3ff57f20212f35661891c9a04/src/components/Toast.tsx#L15-L29)
> - [`src/features/app/AppShell.tsx:L245-L260`](https://github.com/exodious/HoploDex/blob/4e9e00b339fa37d3ff57f20212f35661891c9a04/src/features/app/AppShell.tsx#L245-L260)
> - [`src-tauri/src/commands/photos.rs:L235-L255`](https://github.com/exodious/HoploDex/blob/4e9e00b339fa37d3ff57f20212f35661891c9a04/src-tauri/src/commands/photos.rs#L235-L255)
> - [`src-tauri/src/commands/documents.rs:L210-L230`](https://github.com/exodious/HoploDex/blob/4e9e00b339fa37d3ff57f20212f35661891c9a04/src-tauri/src/commands/documents.rs#L210-L230)
> - [`src-tauri/src/db/migrations/0001_initial.sql:L401-L402`](https://github.com/exodious/HoploDex/blob/4e9e00b339fa37d3ff57f20212f35661891c9a04/src-tauri/src/db/migrations/0001_initial.sql#L401-L402)
>
> ### Description
>
> The React collection is remounted when a database changes, but several plaintext objects and asynchronous operations outlive it:
>
> - A module-global thumbnail cache is keyed only by database-local photo ID and never cleared. Late requests populate it even after cancellation.
> - A module-global resumed draft is matched only by form kind/mode/version and record ID, and cleared only after the editor mounts. A failed or delayed record load leaves it across lock/switch.
> - Attachment batch loops retain the previous owner ID and continue awaiting/issuing writes after unmount; no generation accompanies those writes.
> - Toasts live outside `SessionProvider` and can retain record names/filenames or receive late notifications after locking.
>
> These are grouped as missing session ownership/revocation of frontend data and work.
>
> ### Impact
>
> Viewing photo ID n in database A, locking A, then opening B with photo ID n displays A’s thumbnail in B without reopening A. A resumed A draft whose editor never mounted can similarly populate B’s matching editor, exposing private fields and potentially saving them into B. Delayed attachment reads/batches can insert files selected for A into B if B opens with the same owner ID before the next write. Toasts provide a shorter disclosure window. These paths do not require XSS. Draft disclosure requires its editor not to mount, for example after a failed record load; that window can persist indefinitely. Attachment writes require B to open before delayed work continues. Thumbnail disclosure needs no race.
>
> ### Evidence
>
> ```ts
> const photoThumbnailCache = new Map<number, string>();
> const cached = photoThumbnailCache.get(thumbnailPhotoId);
> // ...
> photoThumbnailCache.set(thumbnailPhotoId, url);
> if (!cancelled) setSrc(url);
> ```
> ```ts
> let resumed: Draft | null = null;
> // matches checks formVersion, kind, mode and targetId only
> ```
> Closing changes React state and remounts the collection; it does not reload these modules.
>
> Verified by source review; no exploit was written or executed.
>
> ### Recommendation
>
> Give every open database an explicit session generation. Own plaintext caches, resumed drafts, notifications and pending tasks inside that lifetime and revoke them synchronously on lock/close/switch/restore. Check the captured generation before both caching responses and issuing later writes; enforce the expected generation at the backend for mutations. Use database identity plus record identity where caching is appropriate. Add two-database tests with colliding IDs, failed/delayed draft loading and delayed attachment reads.
>
> ### References
>
> [CWE-488: Exposure to wrong session](https://cwe.mitre.org/data/definitions/488.html); [CWE-362](https://cwe.mitre.org/data/definitions/362.html).

→ Story 1; FR-001 to FR-005; SC-001. The recommended session generation is the database session of FR-001; backend enforcement of it, for reads as well as changes, is FR-004.

### #66: "[Medium] Backend: unbounded spreadsheet parsing can abort the application" (label: security)

> **Severity:** Medium
> **Area:** Backend (Rust/Tauri)
> **Commit:** 4e9e00b339fa37d3ff57f20212f35661891c9a04
> **Location(s):**
>
> - [`src-tauri/src/services/spreadsheet.rs:L811-L860`](https://github.com/exodious/HoploDex/blob/4e9e00b339fa37d3ff57f20212f35661891c9a04/src-tauri/src/services/spreadsheet.rs#L811-L860)
> - [`src-tauri/src/commands/import_export.rs:L1845-L1854`](https://github.com/exodious/HoploDex/blob/4e9e00b339fa37d3ff57f20212f35661891c9a04/src-tauri/src/commands/import_export.rs#L1845-L1854)
> - [`src-tauri/src/commands/import_export.rs:L2157-L2178`](https://github.com/exodious/HoploDex/blob/4e9e00b339fa37d3ff57f20212f35661891c9a04/src-tauri/src/commands/import_export.rs#L2157-L2178)
> - [`src-tauri/Cargo.toml:L102-L107`](https://github.com/exodious/HoploDex/blob/4e9e00b339fa37d3ff57f20212f35661891c9a04/src-tauri/Cargo.toml#L102-L107)
>
> ### Description
>
> CSV import collects all rows before table validation. XLSX import materializes all sheets and their cell strings without byte, decompression, row or cell budgets. Parsing occurs before the row-loop cancellation check while the import holds the session write guard.
>
> A concrete small-metadata crash path also exists in locked `calamine 0.36.0`: `read_shared_strings` parses the XLSX `uniqueCount` attribute as `usize` and calls `self.strings.reserve(n)` without a limit. The exact downloaded crate source was inspected. An out-of-range allocation capacity panics; HoploDex release builds use `panic = "abort"`.
>
> ### Impact
>
> Importing an attacker-supplied workbook can abort the desktop process; large or compressed input can exhaust memory and keep session operations waiting. CSV also permits unbounded allocation. The attacker must persuade the user to import a file, or already control IPC. No malicious workbook was constructed and no resource-exhaustion test was run.
>
> ### Evidence
>
> ```rust
> for record in reader.records() {
>     // ...
>     data.push(record.iter().map(|cell| unprotect_csv_cell(cell).to_string()).collect());
> }
> ```
> The calamine source’s `uniqueCount` branch directly calls `self.strings.reserve(n)` during workbook loading, before HoploDex can validate its tables.
>
> Verified by source review; no exploit was written or executed.
>
> ### Recommendation
>
> Introduce file, decompressed-byte, sheet, row, cell and text limits and cancellation during parsing. Do parsing outside the database session lock. Patch/upgrade the parser to validate untrusted allocation counts and return bounded errors; merely checking compressed file size does not address the metadata path. Consider a constrained parsing process for formats whose allocation behavior cannot be bounded.
>
> ### References
>
> [CWE-400: Uncontrolled resource consumption](https://cwe.mitre.org/data/definitions/400.html); [Rust Vec::reserve panic contract](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.reserve); [calamine 0.36.0 source revision](https://github.com/tafia/calamine/blob/2872ac1c7c02d03fc8549239f5ce629f6b08e54a/src/xlsx/mod.rs#L337-L355).

→ Story 2; FR-006 to FR-009; SC-002 to SC-004. Parsing outside the session lock is FR-008; how the parser is protected is left to the plan (Assumptions).

### #40: "Suppress the browser's context menu and reload/navigation shortcuts; keep copy/paste" (label: polish)

> ## Problem
>
> Right-clicking anywhere in the app opens the system webview's default browser menu. The app doesn't handle `contextmenu` anywhere, so on Linux (WebKitGTK) a right-click on a blank area offers **Back, Forward, Stop, Reload**. None of these belong in a desktop app. **Reload** is actively harmful: it restarts the frontend and throws away any input typed into an open form. Windows (WebView2: Back, Refresh, Print, Save as) and macOS (WKWebView: Reload) have the same problem.
>
> Keyboard and mouse shortcuts reach the same navigation actions:
> - **F5 / Ctrl+R (⌘R on macOS)** reload the page.
> - **Alt+← / Alt+→** and the mouse's back/forward buttons navigate history. The app doesn't use history routing, so these probably do nothing today, but they should be blocked anyway.
>
> ## What should stay
>
> The menu is useful in two places, and both should keep the native menu:
> - **Text inputs, textareas and contenteditable elements:** Cut, Copy, Paste, Delete, Select All, emoji and spelling.
> - **Selected text anywhere:** Copy.
>
> ## Proposed fix
>
> 1. At app startup, add one `contextmenu` listener that calls `preventDefault()` unless the target is inside an editable element or there's a non-empty text selection:
>
>    ```ts
>    document.addEventListener("contextmenu", (e) => {
>      const t = e.target as HTMLElement;
>      const editable = t.closest("input, textarea, [contenteditable='true']");
>      const hasSelection = !!window.getSelection()?.toString();
>      if (!editable && !hasSelection) e.preventDefault();
>    });
>    ```
>
> 2. Add a `keydown` handler that blocks F5, Ctrl/⌘+R, Ctrl/⌘+Shift+R, Alt+←/→ and the BrowserBack/BrowserForward keys. It should also block the mouse back/forward buttons (`mouseup`/`auxclick` with `button` 3 or 4).
> 3. Keep the default menu and reload in dev builds (`import.meta.env.DEV`) so Inspect Element and hot reload still work.
>
> ## Acceptance
>
> - Right-clicking a blank area, a table row or a button opens no menu.
> - Right-clicking a text field opens the native Cut/Copy/Paste menu.
> - Right-clicking selected text opens a menu with Copy.
> - F5, Ctrl/⌘+R and Alt+← don't reload the page or navigate away, so typed form input survives.
> - An E2E spec covers the reload and navigation keys (regression test, constitution II). WebDriver can't open the native menu, so for the menu itself the spec asserts on `defaultPrevented` of dispatched `contextmenu` events.
>
> ## Out of scope / later
>
> App-specific right-click menus, such as right-clicking a firearm row to get Edit or Dispose, could be built later with Radix `ContextMenu`, which fits with the existing components built on Radix.

**Comment recorded on the issue (owner, at feature 007's pull request #80):**

> ## Feature 007 (PR #80) handles this in the PDF surface only
>
> The child web view that shows a PDF (`specs/007-document-preview/`, research.md §8, §10) suppresses the browser's own menu and keys:
> - **Context menu:** none on macOS (it offers Open in Preview), Copy and Select All on Linux, Copy only on Windows.
> - **Shortcuts:** Ctrl/⌘+S, P and O are cancelled in every frame, and on Windows WebView2's browser accelerator keys and developer tools are off. Reload and history navigation can't leave the document either: the surface's navigation allowlist admits only its own document.
>
> **The main window is unchanged.** Right-click, F5, Ctrl/⌘+R and Alt+←/→ there behave as this issue describes, so everything above is still to do.
>
> ---
> _Generated by [Claude Code](https://claude.ai/code)_

→ Story 3; FR-010 to FR-014; SC-005.

### #78: "Pending changes from another HoploDex version: name the version and allow closing without discarding" (label: enhancement)

> ## Problem
>
> When a database is opened whose pending changes (the open form's unsaved input, kept at a lock or OS shutdown, FR-039) were saved by a different version of HoploDex, the pending changes dialog says only:
>
> > The database locked on … while you were editing …. Your changes were kept. They were kept by another version of HoploDex, so they can only be discarded.
>
> The only button is **Discard changes**. The dialog can't be dismissed, so the user has to throw the changes away to get into the collection, even though the version that saved them could still resume them.
>
> ## What it should do
>
> 1. **Say which version saved them.** Name the HoploDex version the changes were kept by (for example "They were kept by HoploDex 1.2.0"), so the user knows which version to open the database with to get them back.
> 2. **Let the user close without doing anything.** Add a button that closes the database and leaves the pending changes in place, back to the database chooser, so they can open it with that version and resume there. Discarding stays available and still asks first.
>
> ## Notes
>
> - `pending_changes` stores only a per-form `form_version` (`src-tauri/src/db/migrations/0001_initial.sql`), not the app version, so the app version has to be recorded when the changes are written (`session/pending.rs` `write_pending`) and returned in `PendingSummary`.
> - The resumable check is in the frontend: `canResume` in `src/features/session/PendingChangesDialog.tsx` compares the kept `formVersion` with the current form's `FORM_VERSION`.
> - Closing has to leave `pending_changes` untouched, so the next open (by any version) offers them again. Check that the normal close and its automatic backup are allowed while `pending_unresolved` is set, and that the backup still doesn't carry the pending changes.
> - The same close option may also suit the "no longer exists" case, but there the changes can't be resumed by any version, so discard is the only real outcome.
> - Contract to update: `specs/003-*/contracts/ui-databases.md` §13.

**Comment recorded on the issue (owner):**

> The database should record somewhere inside it independent of pending changes what version of the app it was last saved with, and the app should check there. Related but not a duplicate of #24

→ Story 4; FR-015 to FR-017; SC-006. The owner's comment is FR-015 (the version recorded in the database, independent of the pending changes) and FR-016 (the dialog reads it from there).
