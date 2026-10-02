# Feature Specification: Document Preview and Consent Before Opening Externally

**Feature Branch**: `007-document-preview`

**Created**: 2026-10-02

**Status**: Draft

**Input**: User description: GitHub issue #13, "Preview documents in the app, with consent before opening externally". It asks for attached documents to be viewable inside the application rather than in another program, for the user to be able to say which they want, and for opening a document in another program to take an extra step that tells the user the consequences first. The request and the issue's notes are reproduced under [Source Request](#source-request) so this spec stands on its own.

## Clarifications

### Session 2026-10-02

- Q: Once the user has set documents to open in another app, and confirmed the consequences when changing the setting, does each later open still ask for confirmation? → A: Once per session. With that setting, the first document opened in another app after the database is opened or unlocked asks for confirmation; later ones do not, until the database is closed, switched or locked, which is also when HoploDex deletes its copies. With the default setting ("Preview in HoploDex"), every "Open in another app…" asks (FR-012).
- Q: Where is the "how documents open" setting kept: on each computer for all its databases, on each computer for each database, or inside the database? → A: On each computer, for all its databases, with the computer's other settings (003 FR-013). The risk it accepts, readable copies on this computer's disk and other programs' records of them, belongs to the computer, so a confirmation given on one computer does not carry to another that the database is taken to (FR-011).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Preview a Document Inside the Application (Priority: P1)

A collector opens a firearm's record and chooses its attached purchase receipt, a PDF. The receipt appears inside HoploDex, in a viewer over the record. They page through it, zoom in on the serial number printed on page 2, and close the viewer to return to the record. No other program was started, and no readable copy of the receipt was written to the computer's disk. They do the same with a photographed appraisal (a JPEG) and a plain-text note.

**Why this priority**: It is the request's main ask ("ideally it would be previewable within the program"). Today every document leaves the application as a decrypted file on disk, readable by any program, to be shown in whatever program the computer picks. Previewing inside the application keeps the most common documents (receipts, appraisals, registration approvals, manuals) under the database's protection while they are read.

**Independent Test**: Attach a multi-page PDF, a JPEG, a PNG and a text file to a firearm; open each from the record and confirm it is shown inside the application with page and zoom controls as appropriate, that no other program starts, and that no file holding the document's content appears anywhere on disk while it is shown or after it is closed.

**Acceptance Scenarios**:

1. **Given** a firearm with a 3-page PDF attached, **When** the user opens it from the record, **Then** it is shown inside the application window with its first page visible, a "page 1 of 3" indicator, controls to move between pages and to zoom, and the document's name and the date it was attached.
2. **Given** an attached JPEG, PNG, GIF or WebP image, **When** the user opens it, **Then** it is shown inside the application fitted to the viewer, and the user can zoom in, zoom out and return to the fitted size.
3. **Given** an attached plain-text or CSV file, **When** the user opens it, **Then** its text is shown as plain text, with nothing in it interpreted as formatting, links or instructions.
4. **Given** a document is shown in the preview, **When** anything on the computer's disk is inspected (the application's own folders, the system's temporary folders and caches), **Then** no file holding the document's content exists, and none is left after the preview closes.
5. **Given** a firearm with several documents, **When** the user is previewing one, **Then** they can move to the firearm's previous and next document without closing the preview, as in the photo viewer.
6. **Given** a preview is open, **When** the user presses Escape or the close control, **Then** the preview closes and the record is shown as it was.
7. **Given** a preview is open, **When** the application locks (any cause), the database is closed or switched, the computer goes to sleep, or the application quits, **Then** the preview is closed and its content removed from the screen and from memory along with the rest of the collection's data.

---

### User Story 2 - Open a Document in Another Program, Knowingly (Priority: P2)

The collector wants to print an appraisal, or open a Word document the application cannot preview. They choose "Open in another app…". Before anything is written to disk, the application explains what that means: a readable copy of the document is put on this computer where other programs and anyone using this computer account can read it; the other program may keep copies of its own (recent-files lists, autosaves, caches, cloud sync) that HoploDex cannot remove; and HoploDex deletes its own copy when the database closes or locks or the application quits, so the other program may lose the file then. The collector confirms, and the document opens in the computer's default program for its type. Had they cancelled, nothing would have been written.

**Why this priority**: The request requires that opening externally "has an extra step and notify the users of the potential consequences". It is also the only way to see a document type the preview cannot show, and the only way to print, edit or fill in a document, so it must remain available.

**Independent Test**: Choose "Open in another app…" on a PDF; confirm the consequences are shown and nothing is written to disk until the user confirms; cancel and confirm nothing was written; repeat and confirm, and check the document opens in the default program and that its copy is deleted when the database is closed.

**Acceptance Scenarios**:

1. **Given** any attached document, **When** the user chooses "Open in another app…", **Then** a confirmation names the document and states the consequences (FR-009) before any copy is written.
2. **Given** that confirmation, **When** the user cancels, **Then** no copy of the document is written anywhere and no other program is started.
3. **Given** that confirmation, **When** the user confirms, **Then** the document opens in the computer's default program for its type, and its copy is handled exactly as today: deleted when the database is closed, switched or locked, when the application quits, and at the next launch after a crash (001 FR-035, 003 FR-022).
4. **Given** a document of a type the preview cannot show (for example a Word document or a HEIC photo), **When** the user opens it from the record, **Then** the application says it can't be previewed here and offers "Open in another app…", which asks for the same confirmation.
5. **Given** the user confirmed, **When** the computer has no program for that type, **Then** the application says so and deletes the copy it wrote at once.

---

### User Story 3 - Choose How Documents Open (Priority: P3)

A collector who always prints their documents, or simply prefers their own PDF reader, sets the application to open documents in another program. Changing the setting shows the same consequences and asks them to confirm. From then on, opening a document hands it to the other program; the first one after each time the database is opened or unlocked asks for confirmation again, and later ones in that session don't. They can still preview any previewable document by choosing "Preview" explicitly, and can switch the setting back at any time.

**Why this priority**: The request says "the user should be able to say what it is they want". Stories 1 and 2 already give the user both choices for each document; this story only makes one of them the default, so it is the least essential.

**Independent Test**: Change the setting to open documents in another app; confirm the consequences are shown and the change takes effect only on confirmation; open a document and confirm it goes to the other program; choose "Preview" on a PDF and confirm it is previewed; set the setting back.

**Acceptance Scenarios**:

1. **Given** the default settings, **When** the user views how documents open, **Then** it is set to "Preview in HoploDex".
2. **Given** that setting, **When** the user changes it to "Open in another app", **Then** the consequences (FR-009) are shown and the setting changes only if the user confirms; cancelling leaves it at "Preview in HoploDex".
3. **Given** the setting is "Open in another app", **When** the user opens a document from a record, **Then** it is opened in the default program for its type; the first such open after the database was opened or unlocked asks for confirmation (FR-009), and later opens in the same session do not.
4. **Given** the setting is "Open in another app" and a document was already opened in another app this session, **When** the database is locked and unlocked, closed and reopened, or switched, **Then** the next document opened asks for confirmation again.
5. **Given** the setting is "Open in another app", **When** the user chooses "Preview" on a previewable document, **Then** it is previewed inside the application.
6. **Given** the setting is "Open in another app", **When** the user changes it back to "Preview in HoploDex", **Then** it changes without a confirmation, and documents open in the preview again.

---

### Edge Cases

- **A large document** (for example a 300-page scanned PDF of 150 MB): the preview shows progress until the first page is ready, shows pages as they become ready, stays responsive, and can be closed at any time; closing it releases the memory it used.
- **A damaged file, or one whose content is not what its name says** (a file named `.pdf` that is not a PDF): the preview does not try to show it as something else; it says the document can't be previewed and offers "Open in another app…" with its confirmation.
- **A password-protected PDF**: the preview says the document is protected by its own password and can't be previewed here, and offers "Open in another app…". The application does not ask for or keep a document's own password.
- **A PDF that contains scripts, links, form fields, embedded files, or references to outside images or fonts**: the visible pages are shown; nothing in the document runs, nothing is fetched from the network or from the computer's files, links are not followed, forms cannot be submitted, and embedded files are not opened (FR-004).
- **A crafted document meant to attack a viewer**: the worst outcome allowed is that the preview fails and says the document can't be previewed. It must not reach the application's data, commands or other documents, start a program, or bring the application down (FR-004, SC-003).
- **A text file in an unusual encoding or containing binary data**: it is shown as best the text can be read, with unreadable characters replaced, and never as anything but plain text.
- **SVG, HTML and other types that can carry scripts**: never previewed, even though some can be displayed by a web page; they are offered only through "Open in another app…".
- **Reading a long document without touching anything**: time spent reading without input counts as idle, as everywhere else (003 FR-034); scrolling, paging and zooming are input. The idle lock closes the preview like any other screen.
- **The other program still has the document open when the database closes or locks**: HoploDex deletes its copy as today; where the operating system refuses because the file is in use, the deletion is retried at the next launch (001 FR-035). The confirmation tells the user that the other program may lose the file.
- **Opening the same document in another app twice in a session**: with the default setting each open asks; with "Open in another app" only the session's first does (FR-012). Either way the one copy is reused or replaced rather than several being left.
- **The setting on another computer** (a database carried between computers): the setting belongs to each computer (FR-011), so on a computer where it was never changed, documents open in the preview and every external open asks.
- **Several databases on one computer**: they share the computer's setting; the once-per-session confirmation is per database session, so switching to another database asks again.

## Requirements *(mandatory)*

### Functional Requirements

**Preview**

- **FR-001**: The application MUST be able to preview, inside its own window, attached documents of these types: PDF; JPEG, PNG, GIF and WebP images; plain text and CSV. Every other type is not previewable. Whether a document is previewable MUST be decided by its recorded type and confirmed by its content, so a file whose content does not match its type is treated as unpreviewable (FR-006).
- **FR-002**: The preview MUST look and work the same on every supported operating system and MUST NOT depend on any program installed on the computer.
- **FR-003**: Previewing MUST NOT write any part of the document's decrypted content to disk: not to the application's own folders, the system's temporary folders, caches, or anywhere else. The decrypted content MUST be held in memory only while the preview shows it, and released when the preview closes, the user moves to another document, or the application locks, closes the database, sleeps or quits.
- **FR-004**: The preview MUST treat a document as inert content to be displayed. Nothing in a document may run (scripts, actions on opening), fetch anything from the network or the computer's files, follow a link, submit a form, open an embedded file, or start a program. Whatever a document contains, the preview MUST NOT be able to reach the application's data or commands beyond the one document it is showing.
- **FR-005**: The preview MUST offer, for a PDF: every page, an indicator of the current page and the page count, moving to the next, previous, first and last page, and zooming in, out, to fit the width and to fit the page; for an image: fitting to the viewer, zooming in and out, and actual size; for text: the whole text, wrapped, as plain text. Every control MUST be usable from the keyboard.
- **FR-006**: When a previewable type cannot be shown (damaged, content not matching its type, protected by its own password, or failing to render), the preview MUST say so in plain words, saying which of those it is where known, and offer "Open in another app…" (FR-008). It MUST NOT fall back to showing the content as another type.
- **FR-007**: The preview MUST be a viewer over the firearm's record, consistent with the photo viewer (constitution III): titled with the document's name, described with its kind, the date it was attached and, for a PDF, its page count; moving to the firearm's previous and next document; offering "Open in another app…" and deleting the document (with the same confirmation as from the list); and closing with Escape or its close control, returning to the record unchanged.

**Opening in another program**

- **FR-008**: "Open in another app…" MUST be available for every attached document, from the record's document list and from the preview. Before any copy is written, it MUST show a confirmation that names the document and states the consequences (FR-009), with confirming and cancelling as the choices; cancelling MUST write nothing and start nothing.
- **FR-009**: The consequences shown MUST say, in plain words: (a) a readable, unprotected copy of the document will be put on this computer, where other programs and anyone using this computer account can read it; (b) the other program may keep its own copies or records of it (for example recent-files lists, autosaves, caches, or cloud sync) that HoploDex cannot find or delete; (c) HoploDex deletes its own copy when the database is closed or locked or the application quits, so the other program may lose access to the file then, and changes saved to that copy are not kept in the collection.
- **FR-010**: Once the user confirms, the document MUST open in the computer's default program for its type, and its copy MUST be handled as today (001 FR-010, FR-035; 003 FR-022, FR-037). If the computer has no program for the type, the application MUST say so and delete the copy at once.

**Choosing how documents open**

- **FR-011**: The user MUST be able to choose how documents open: "Preview in HoploDex" (the default) or "Open in another app". The setting MUST be kept on each computer, outside every database, and apply to all databases opened on that computer (003 FR-013); it does not travel with a database.
- **FR-012**: Changing the setting to "Open in another app" MUST first show the consequences (FR-009) and change only if the user confirms. Changing it back MUST NOT ask. While it is "Open in another app", opening a document MUST hand it to the other program as in FR-010, asking for the confirmation of FR-008 only for the first document opened in another app in each session, a session being the time from opening or unlocking a database until it is closed, switched or locked. With "Preview in HoploDex", every "Open in another app…" MUST ask.
- **FR-013**: Opening a document from a record MUST follow the setting: with "Preview in HoploDex", a previewable document is previewed and an unpreviewable one shows that it can't be previewed with "Open in another app…" (FR-006, FR-008); with "Open in another app", it is opened as FR-012 says. Whatever the setting, each previewable document MUST also offer "Preview", and each document "Open in another app…", so either is one choice away. The document list MUST show which documents can be previewed.

**Locking and data removal**

- **FR-014**: A preview MUST be closed and its content removed from the screen and from memory whenever the application removes the collection's data: a lock by any means, closing or switching the database, sleep (as step 1 of 003 FR-037), an operating-system shutdown, or quitting. It does not hold up any of these.

### Key Entities

- **Document Attachment** (unchanged from 001): a file attached to a firearm. Whether it is previewable is derived from its recorded type and its content (FR-001); nothing new is stored on it.
- **Document opening setting**: how documents open on this computer, "Preview in HoploDex" or "Open in another app"; kept with the computer's other settings, not in any database (FR-011).
- **Session confirmation**: whether the user has confirmed the consequences in the current database session; held only while the database is open and forgotten at close, switch or lock (FR-012). It is never stored.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A PDF of up to 10 MB shows its first page, and an image of up to 10 MB is shown, within 1 second of the user opening it, on a collection of 10,000 items (the constitution's action budget). Larger documents show progress within 1 second.
- **SC-002**: After previewing one document of every previewable type, no file containing any of their content exists anywhere on disk, checked by searching the application's folders and the system's temporary and cache folders for each document's bytes.
- **SC-003**: Across a test set of hostile and malformed documents (PDFs with scripts, actions on opening, external links, remote images and fonts, embedded files and launch actions; truncated and corrupted files; files whose names misstate their type), none runs anything, makes a network request, reads another file, or ends the application; each is either shown inertly or reported as unpreviewable.
- **SC-004**: With the default setting, every opening of a document in another program is preceded by the confirmation; with "Open in another app", the first in every database session is; and in every cancelled confirmation nothing is written to disk.
- **SC-005**: A user can read any previewable attachment with a single choice from the firearm's record, without any other program.
- **SC-006**: When the application locks while a preview is open, the preview's content is off the screen before the chooser is shown, in every case covered by the lock tests of feature 003.

## Assumptions

- **Which types are previewed**: PDF, the four image types every supported system's built-in display handles alike (JPEG, PNG, GIF, WebP), and plain text and CSV, which together cover the receipts, appraisals, approval letters, manuals and notes the collection holds. HEIC and TIFF photos, Office and OpenDocument files, and RTF are opened in another program; adding a type later is a change to FR-001's list.
- **View only**: the preview does not print, annotate, edit, fill in forms, or save a copy elsewhere; those need another program and so go through "Open in another app…". Selecting and copying text from the preview is not offered.
- **A document's own password** is not asked for or stored; a protected PDF is opened in another program.
- **Links in documents** are shown but not followed. Opening a link in a browser would send data off the device and is out of scope.
- **The external path is unchanged**: where the copy goes, when it is deleted and how (secure deletion where supported) stay as 001 FR-035 and 003 FR-022 and FR-037 define them; this feature only puts a confirmation in front of it and makes the preview the default.
- **No change to stored records**: documents are stored as they are today and nothing in the database changes; the only new stored value is the computer's setting (FR-011).
- **Security review**: rendering documents inside the application is new attack surface (malicious PDFs, what the preview can reach, the content security policy). The plan MUST record how FR-003 and FR-004 are met, and the release security review (issue #21) MUST cover the preview once built.
- **Design**: the preview and the document list's actions follow the existing photo viewer and dialogs (constitution III), and the preview is added to the screenshot walk.
- **Unreleased application**: any schema change is made in place.

## Relationship to Earlier Features

This feature extends `specs/001-firearms-inventory/`. It **amends**:

- **FR-010** ("reopen them from the record"): reopening previews the document inside the application by default (FR-001, FR-013), and opening it in another program takes a confirmation (FR-008).
- **User Story 4, scenario 4** ("can be reopened from it"): as FR-010 above.
- **FR-035** and **SC-010** are unchanged and still govern the copy made for another program. The clarification that "previewing documents inside the application, and warning the user before opening one externally, are a separate future feature" is resolved by this feature.

It also touches `specs/003-database-protection-management/`: FR-022, FR-036 and FR-037 (deleting decrypted document copies) are unchanged, and a preview is part of the collection data removed from the screen at a lock (FR-014). 003 FR-013's list of settings kept on each computer gains how documents open (FR-011).

## Source Request

This feature was filed as GitHub issue #13, "Preview documents in the app, with consent before opening externally" (labels: new feature, needs spec, security). It is the second half of the original `open_document` item; the first half, deleting decrypted copies at exit, shipped as 001's FR-035.

**The request, verbatim:**

> Ideally it would be previewable within the program rather than opening an external viewer - but the user should be able to say what it is they want / open externally has an extra step and notify the users of the potential consequences.

**Notes recorded on the issue, with where this spec answers them:**

- *Why it needs its own spec:* "It's new user-facing behavior that goes beyond FR-010's 'reopen from the record': a preview component, a preference (preview in the app vs. open externally), and a consent step that tells the user the consequences before a decrypted copy leaves the app." → The preview (FR-001 to FR-007), the setting (FR-011 to FR-013) and the confirmation with its consequences (FR-008 to FR-010).
- *Security:* "Previewing PDFs and images in the webview needs its own security review: malicious PDFs, the CSP, and what the preview can reach. The release security review (#21) should cover it once built." → FR-003, FR-004, SC-002, SC-003, and the security review assumption.
- *Dependencies:* "Stands alone, so it can be specified at any point."
