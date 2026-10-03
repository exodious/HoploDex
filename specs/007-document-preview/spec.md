# Feature Specification: Document Preview and Consent Before Opening Externally

**Feature Branch**: `007-document-preview`

**Created**: 2026-10-02

**Status**: Draft

**Input**: User description: GitHub issue #13, "Preview documents in the app, with consent before opening externally". It asks for attached documents to be viewable inside the application rather than in another program, for the user to be able to say which they want, and for opening a document in another program to take an extra step that tells the user the consequences first. The request and the issue's notes are reproduced under [Source Request](#source-request) so this spec stands on its own.

## Clarifications

### Session 2026-10-02

- Q: Once the user has set documents to open in another app, and confirmed the consequences when changing the setting, does each later open still ask for confirmation? → A: Once per session. With that setting, the first document opened in another app after the database is opened or unlocked asks for confirmation; later ones do not, until the database is closed, switched or locked, which is also when HoploDex deletes its copies. With the default setting ("Preview in HoploDex"), every "Open in another app…" asks (FR-012).
- Q: Where is the "how documents open" setting kept: on each computer for all its databases, on each computer for each database, or inside the database? → A: On each computer, for all its databases, with the computer's other settings (003 FR-013). The risk it accepts, readable copies on this computer's disk and other programs' records of them, belongs to the computer, so a confirmation given on one computer does not carry to another that the database is taken to (FR-011).
- Q: Which image types does the preview show? → A: TIFF as well as JPEG, PNG, GIF and WebP, since scanned receipts, appraisals and approval letters often arrive as TIFF. HEIC is still opened in another program (FR-001). *Superseded on 2026-10-03: only TIFF, since documents no longer take photo or other image types (FR-016).*
- Q: How does the preview show a TIFF that holds several pages? → A: All of its pages, with the same page indicator, page navigation and fit-width and fit-page zoom as a PDF, so a scan reads the same whichever format it was saved in (FR-005).
- Q: Is a CSV file shown as plain text or as a table? → A: As plain text, exactly as written, like any text file. It is not laid out in rows and columns, and nothing in it, formulas included, is interpreted (FR-005).
- Q: Can the user search for words inside the document being previewed? → A: No, not in this feature; the preview has no find. Instead, the collection search finds a firearm by the names of its attached documents (FR-015).
- Q: Which attachment filenames does the collection search match: documents only, or photos too? → A: Documents only. Document names are usually descriptive ("Form 4 approval.pdf"); photo names are mostly camera names such as "IMG_4512.jpg" and would only add noise (FR-015).
- Q: When a firearm is found only because one of its documents' names matched, does the search result say which document matched? → A: No. The firearm appears like any other match; the search shows no field it matched on, for document names as for the firearm's own text (FR-015).
- Q: Can a screen reader read the text of a PDF shown in the preview, though the user cannot select or copy it? → A: Yes. A PDF page's text is available to screen readers but cannot be selected or copied; an image or a scanned page with no text is announced by the document's name and page number (FR-005).

### Session 2026-10-03

- Q: Since 006 a document can belong to an accessory as well as a firearm. When previewing, do "previous" and "next document" move only through the documents of the record it belongs to, or also through those of records mounted on it? → A: Only that record's own documents: a firearm's on a firearm's record, an accessory's on an accessory's record, as the photo viewer does. Documents of records mounted on it are not included. Preview and "Open in another app…" work on an accessory's documents exactly as on a firearm's (FR-007, FR-008).
- Q: Does the Accessories page search also find an accessory by the filenames of its attached documents, as the collection search finds a firearm by its own? → A: Yes. Each page searches its own records' document names: the collection page a firearm's, the Accessories page an accessory's. Neither finds a record through the documents of records mounted on it, as neither finds one through the mounted records' own fields (006 FR-018) (FR-015).

### Session 2026-10-03 (security findings on issue #13)

Two security-review findings were posted on issue #13: opening an attachment of any type hands it to the operating system's default handler, which on Windows runs a program (High), and an HTML or SVG attachment opened in another program loads content from the network (Medium). Both note that a confirmation shown only in the application's own interface can be skipped by a compromised interface.

- Q: How is "Open in another app…" confirmed, so that a compromised interface can't skip the confirmation? → A: By the operating system's own dialog, shown by HoploDex's trusted side and naming the exact document. The interface cannot answer it. Changing the setting to "Open in another app" is confirmed the same way (FR-008, FR-012).
- Q: Which files may be attached as documents, and so previewed or handed to another program? → A: Documents only: receipts, service records, registration forms, bills of sale and other transfer records. That means PDF, TIFF (scanned paperwork), plain text, CSV, RTF, Word (DOC, DOCX) and spreadsheets (XLS, XLSX, ODT, ODS), each confirmed by its content. Photos (JPEG, PNG) are attached as photos, not documents. Other images, programs, scripts, installers, shortcuts, web pages, SVG and macro-enabled Office files are refused (FR-016). Only these types are ever handed to another program (FR-017).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Preview a Document Inside the Application (Priority: P1)

A collector opens a firearm's record and chooses its attached purchase receipt, a PDF. The receipt appears inside HoploDex, in a viewer over the record. They page through it, zoom in on the serial number printed on page 2, and close the viewer to return to the record. No other program was started, and no readable copy of the receipt was written to the computer's disk. They do the same with a scanned appraisal (a TIFF) and a plain-text note, and then with the receipt attached to the optic mounted on that firearm, from the optic's own record.

**Why this priority**: It is the request's main ask ("ideally it would be previewable within the program"). Today every document leaves the application as a decrypted file on disk, readable by any program, to be shown in whatever program the computer picks. Previewing inside the application keeps the most common documents (receipts, appraisals, registration approvals, manuals) under the database's protection while they are read.

**Independent Test**: Attach a multi-page PDF, a single-page and a multi-page TIFF, and a text file to a firearm, and some of them to an accessory; open each from its record and confirm it is shown inside the application with page and zoom controls as appropriate, that no other program starts, and that no file holding the document's content appears anywhere on disk while it is shown or after it is closed.

**Acceptance Scenarios**:

1. **Given** a firearm or an accessory with a 3-page PDF attached, **When** the user opens it from the record, **Then** it is shown inside the application window with its first page visible, a "page 1 of 3" indicator, controls to move between pages and to zoom, and the document's name and the date it was attached.
2. **Given** an attached single-page TIFF image, **When** the user opens it, **Then** it is shown inside the application fitted to the viewer, and the user can zoom in, zoom out and return to the fitted size.
3. **Given** an attached TIFF holding 4 scanned pages, **When** the user opens it, **Then** it is shown with a "page 1 of 4" indicator and the same page and zoom controls as a PDF.
4. **Given** an attached plain-text or CSV file, **When** the user opens it, **Then** its text is shown as plain text, with nothing in it interpreted as formatting, links or instructions.
5. **Given** a document is shown in the preview, **When** anything on the computer's disk is inspected (the application's own folders, the system's temporary folders and caches), **Then** no file holding the document's content exists, and none is left after the preview closes.
6. **Given** a firearm or an accessory with several documents, **When** the user is previewing one, **Then** they can move to that record's previous and next document without closing the preview, as in the photo viewer; **and Given** a firearm with an accessory mounted on it that has documents of its own, **Then** moving through the firearm's documents never reaches the accessory's.
7. **Given** a preview is open, **When** the user presses Escape or the close control, **Then** the preview closes and the record is shown as it was.
8. **Given** a preview is open, **When** the application locks (any cause), the database is closed or switched, the computer goes to sleep, or the application quits, **Then** the preview is closed and its content removed from the screen and from memory along with the rest of the collection's data.
9. **Given** a firearm with "2024 appraisal.pdf" attached, **When** the user searches the collection for "appraisal", **Then** that firearm is found; **and When** the document is deleted, **Then** the same search no longer finds it by that name; **and Given** an accessory with "Optic receipt.pdf" attached and mounted on a firearm, **When** the user searches the Accessories page for "receipt", **Then** the accessory is found, **and When** the user searches the collection for "receipt", **Then** the firearm is not found through it.

---

### User Story 2 - Open a Document in Another Program, Knowingly (Priority: P2)

The collector wants to print an appraisal, or open a Word document the application cannot preview. They choose "Open in another app…". Before anything is written to disk, the computer's own confirmation dialog explains what that means: a readable copy of the document is put on this computer where other programs and anyone using this computer account can read it; the other program may keep copies of its own (recent-files lists, autosaves, caches, cloud sync) that HoploDex cannot remove; the other program may connect to the internet if the document asks it to; and HoploDex deletes its own copy when the database closes or locks or the application quits, so the other program may lose the file then. The collector confirms, and the document opens in the computer's default program for its type. Had they cancelled, nothing would have been written.

**Why this priority**: The request requires that opening externally "has an extra step and notify the users of the potential consequences". It is also the only way to see a document type the preview cannot show, and the only way to print, edit or fill in a document, so it must remain available. Because a confirmation shown only inside the application could be skipped by a compromised interface, the confirmation is the operating system's own dialog (issue #13's security findings).

**Independent Test**: Choose "Open in another app…" on a PDF; confirm the consequences are shown and nothing is written to disk until the user confirms; cancel and confirm nothing was written; repeat and confirm, and check the document opens in the default program and that its copy is deleted when the database is closed.

**Acceptance Scenarios**:

1. **Given** any attached document, **When** the user chooses "Open in another app…", **Then** the operating system's confirmation dialog names the document and states the consequences (FR-009) before any copy is written.
2. **Given** that confirmation, **When** the user cancels, **Then** no copy of the document is written anywhere and no other program is started.
3. **Given** that confirmation, **When** the user confirms, **Then** the document opens in the computer's default program for its type, and its copy is handled exactly as today: deleted when the database is closed, switched or locked, when the application quits, and at the next launch after a crash (001 FR-035, 003 FR-022).
4. **Given** a document of a type the preview cannot show (for example a Word document or a spreadsheet), **When** the user opens it from the record, **Then** the application says it can't be previewed here and offers "Open in another app…", which asks for the same confirmation.
5. **Given** the user confirmed, **When** the computer has no program for that type, **Then** the application says so and deletes the copy it wrote at once.
6. **Given** a file that is not one of the document types (FR-016), such as a program (`.exe`), a script, a shortcut, a web page, an SVG, a JPEG photo, or a file named `.pdf` whose content is a web page, **When** the user attaches it as a document, by the file chooser or by dropping it, **Then** it is refused with a message saying which document types can be attached (and, for a photo, that it belongs under Photos), and nothing is stored.
7. **Given** a document stored before this feature whose type or content is not a document type (FR-017), **When** the user chooses "Open in another app…", **Then** the application refuses before showing the confirmation, writes nothing and starts nothing, and the document can still be deleted.

---

### User Story 3 - Choose How Documents Open (Priority: P3)

A collector who always prints their documents, or simply prefers their own PDF reader, sets the application to open documents in another program. Changing the setting shows the same consequences and asks them to confirm. From then on, opening a document hands it to the other program; the first one after each time the database is opened or unlocked asks for confirmation again, and later ones in that session don't. They can still preview any previewable document by choosing "Preview" explicitly, and can switch the setting back at any time.

**Why this priority**: The request says "the user should be able to say what it is they want". Stories 1 and 2 already give the user both choices for each document; this story only makes one of them the default, so it is the least essential.

**Independent Test**: Change the setting to open documents in another app; confirm the consequences are shown and the change takes effect only on confirmation; open a document and confirm it goes to the other program; choose "Preview" on a PDF and confirm it is previewed; set the setting back.

**Acceptance Scenarios**:

1. **Given** the default settings, **When** the user views how documents open, **Then** it is set to "Preview in HoploDex".
2. **Given** that setting, **When** the user changes it to "Open in another app", **Then** the operating system's confirmation dialog shows the consequences (FR-009) and the setting changes only if the user confirms; cancelling leaves it at "Preview in HoploDex".
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
- **A compromised interface** (for example through a flaw in the web view): it cannot confirm "Open in another app…" or the change of setting, because the confirmation is the operating system's dialog, not part of the interface; it cannot hand another program anything but a document type whose content matches (FR-017). Once the user has confirmed in a session with "Open in another app" set, it could open the database's other documents without asking again; they are all document types, which is the accepted limit of the once-per-session choice (FR-012).
- **A Word or spreadsheet file containing macros**: macro-enabled formats (DOCM, XLSM and their kin, or a DOCX or XLSX whose content is macro-enabled) are refused at attaching (FR-016). A legacy DOC or XLS can still hold macros; its copy is marked as coming from elsewhere where the operating system supports it, so the other program treats it as untrusted (FR-017).
- **A TIFF using a compression or layout the preview cannot decode** (for example an unusual fax encoding): treated like a damaged file; the preview says it can't be shown and offers "Open in another app…" (FR-006). If one page of a multi-page TIFF can't be decoded, the others are still shown and that page says it can't be shown.
- **A text file in an unusual encoding or containing binary data**: it is shown as best the text can be read, with unreadable characters replaced, and never as anything but plain text.
- **SVG, HTML and other types that can carry scripts**: never previewed and never attached as documents (FR-016); a file of such a type attached before this feature is not handed to another program either (FR-017).
- **Reading a long document without touching anything**: time spent reading without input counts as idle, as everywhere else (003 FR-034); scrolling, paging and zooming are input. The idle lock closes the preview like any other screen.
- **The other program still has the document open when the database closes or locks**: HoploDex deletes its copy as today; where the operating system refuses because the file is in use, the deletion is retried at the next launch (001 FR-035). The confirmation tells the user that the other program may lose the file.
- **Opening the same document in another app twice in a session**: with the default setting each open asks; with "Open in another app" only the session's first does (FR-012). Either way the one copy is reused or replaced rather than several being left.
- **The setting on another computer** (a database carried between computers): the setting belongs to each computer (FR-011), so on a computer where it was never changed, documents open in the preview and every external open asks.
- **Searching for a file extension or a common word** (for example "pdf" or "scan"): the collection search finds every firearm, and the Accessories page every accessory, with a document whose name contains it, as any search term matching many records does.
- **Several databases on one computer**: they share the computer's setting; the once-per-session confirmation is per database session, so switching to another database asks again.

## Requirements *(mandatory)*

### Functional Requirements

**Preview**

- **FR-001**: The application MUST be able to preview, inside its own window, attached documents of these types: PDF; TIFF; plain text and CSV. The other document types (FR-016: RTF, Word, spreadsheets) are not previewable. Whether a document is previewable MUST be decided by its recorded type and confirmed by its content, so a file whose content does not match its type is treated as unpreviewable (FR-006).
- **FR-002**: The preview MUST look and work the same on every supported operating system and MUST NOT depend on any program installed on the computer.
- **FR-003**: Previewing MUST NOT write any part of the document's decrypted content to disk: not to the application's own folders, the system's temporary folders, caches, or anywhere else. The decrypted content MUST be held in memory only while the preview shows it, and released when the preview closes, the user moves to another document, or the application locks, closes the database, sleeps or quits.
- **FR-004**: The preview MUST treat a document as inert content to be displayed. Nothing in a document may run (scripts, actions on opening), fetch anything from the network or the computer's files, follow a link, submit a form, open an embedded file, or start a program. Whatever a document contains, the preview MUST NOT be able to reach the application's data or commands beyond the one document it is showing.
- **FR-005**: The preview MUST offer, for a PDF or a TIFF holding more than one page: every page, an indicator of the current page and the page count, moving to the next, previous, first and last page, and zooming in, out, to fit the width and to fit the page; for a single-page TIFF: fitting to the viewer, zooming in and out, and actual size; for text, CSV included: the whole text exactly as written, wrapped, as plain text, with nothing in it (formulas included) interpreted and no layout into rows and columns. Every control MUST be usable from the keyboard. A PDF page's text MUST be available to screen readers without being selectable or copyable; an image, or a page with no text, MUST be announced by the document's name and, where it has pages, the page number (constitution III, WCAG 2.1 AA).
- **FR-006**: When a previewable type cannot be shown (damaged, content not matching its type, protected by its own password, or failing to render), the preview MUST say so in plain words, saying which of those it is where known, and offer "Open in another app…" (FR-008). It MUST NOT fall back to showing the content as another type.
- **FR-007**: The preview MUST be a viewer over the record the document belongs to, a firearm's or an accessory's (006 FR-007a), consistent with the photo viewer (constitution III): titled with the document's name, described with its kind, the date it was attached and, for a PDF or multi-page TIFF, its page count; moving to that record's previous and next document, and never to the documents of records mounted on it; offering "Open in another app…" and deleting the document (with the same confirmation as from the list); and closing with Escape or its close control, returning to the record unchanged.

**Opening in another program**

- **FR-008**: "Open in another app…" MUST be available for every attached document, from the record's document list and from the preview. Before any copy is written, it MUST show a confirmation that names the document and states the consequences (FR-009), with confirming and cancelling as the choices; cancelling MUST write nothing and start nothing. The confirmation MUST be shown by the operating system, from the application's trusted side rather than its interface, and only the user's answer in it may let a copy be written, so a compromised interface can neither show a different document's name nor confirm on the user's behalf.
- **FR-009**: The consequences shown MUST say, in plain words: (a) a readable, unprotected copy of the document will be put on this computer, where other programs and anyone using this computer account can read it; (b) the other program may keep its own copies or records of it (for example recent-files lists, autosaves, caches, or cloud sync) that HoploDex cannot find or delete; (c) the other program may connect to the internet if the document asks it to (for example to load a picture or follow a link), which can reveal that the document was opened; (d) HoploDex deletes its own copy when the database is closed or locked or the application quits, so the other program may lose access to the file then, and changes saved to that copy are not kept in the collection.
- **FR-010**: Once the user confirms, the document MUST open in the computer's default program for its type, and its copy MUST be handled as today (001 FR-010, FR-035; 003 FR-022, FR-037), readable only by the user's own account and, where the operating system supports it, marked as coming from elsewhere so the other program treats it as untrusted. If the computer has no program for the type, the application MUST say so and delete the copy at once.

**Choosing how documents open**

- **FR-011**: The user MUST be able to choose how documents open: "Preview in HoploDex" (the default) or "Open in another app". The setting MUST be kept on each computer, outside every database, and apply to all databases opened on that computer (003 FR-013); it does not travel with a database.
- **FR-012**: Changing the setting to "Open in another app" MUST first show the consequences (FR-009) in the operating system's confirmation, as FR-008 does, and change only if the user confirms there. Changing it back MUST NOT ask. While it is "Open in another app", opening a document MUST hand it to the other program as in FR-010, asking for the confirmation of FR-008 only for the first document opened in another app in each session, a session being the time from opening or unlocking a database until it is closed, switched or locked. With "Preview in HoploDex", every "Open in another app…" MUST ask.
- **FR-013**: Opening a document from a record MUST follow the setting: with "Preview in HoploDex", a previewable document is previewed and an unpreviewable one shows that it can't be previewed with "Open in another app…" (FR-006, FR-008); with "Open in another app", it is opened as FR-012 says. Whatever the setting, each previewable document MUST also offer "Preview", and each document "Open in another app…", so either is one choice away. The document list MUST show which documents can be previewed.

**Locking and data removal**

- **FR-014**: A preview MUST be closed and its content removed from the screen and from memory whenever the application removes the collection's data: a lock by any means, closing or switching the database, sleep (as step 1 of 003 FR-037), an operating-system shutdown, or quitting. It does not hold up any of these.

**Finding documents**

- **FR-015**: The collection search (001 FR-013) MUST also match the original filenames of each firearm's attached documents, and the Accessories page search (006 FR-018) those of each accessory's, in the same way each matches the record's own text, and within the same time budget (search within 500 ms at 10,000 firearms and 10,000 accessories, 006 FR-026). Each page matches only the documents of its own records: a firearm is not found through the documents of an accessory or firearm mounted on it, nor an accessory through those of a record mounted on it (006 FR-018). Attaching or deleting a document MUST update this at once. A record found by a document's name is shown like any other match, without saying which document matched. Photo filenames are not searched. The preview itself offers no search within a document.

**Which files are documents**

- **FR-016**: Attaching a document, by the file chooser or by dropping a file, MUST accept only these document types, each recognised by its name's extension and confirmed by its content: PDF (`.pdf`); TIFF (`.tif`, `.tiff`); plain text (`.txt`); CSV (`.csv`); RTF (`.rtf`); Word (`.doc`, `.docx`); spreadsheets (`.xls`, `.xlsx`); OpenDocument (`.odt`, `.ods`). A file whose content does not match its extension, a macro-enabled Office file (by extension or by content), a text or CSV file whose content is a web page, SVG or other markup, and every other type, photos (JPEG, PNG) included, MUST be refused with a message naming the document types, saying for a photo that it belongs under Photos; nothing is stored. A drop routes JPEG and PNG to Photos as today and refuses any other non-document file. The type recorded with a document MUST be the one its check found, not one supplied with the file. Adding a type later is a change to this list.
- **FR-017**: The application MUST hand a document to another program only if its recorded type and its content pass FR-016's check at the moment of opening. A document that fails (one stored before this feature, for example) MUST be refused with a plain message before the confirmation is shown, writing nothing and starting nothing. The copy MUST carry the extension of the type found, not one taken from the stored name, so the operating system cannot pick a program by a misleading name.

### Key Entities

- **Document Attachment** (stored as in 001 and 006): a file attached to a firearm or to an accessory (006 FR-007a), now only of a document type (FR-016), with the type its content check found recorded as its type. Whether it is previewable is derived from its recorded type and its content (FR-001); nothing new is stored on it. Its original filename becomes part of what the search covers for the record it belongs to: the collection search for a firearm's, the Accessories page search for an accessory's (FR-015).
- **Document opening setting**: how documents open on this computer, "Preview in HoploDex" or "Open in another app"; kept with the computer's other settings, not in any database (FR-011).
- **Session confirmation**: whether the user has confirmed the consequences in the current database session; held only while the database is open and forgotten at close, switch or lock (FR-012). It is never stored.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A PDF or a TIFF of up to 10 MB shows its first page within 1 second of the user opening it, on a collection of 10,000 items (the constitution's action budget). Larger documents show progress within 1 second.
- **SC-002**: After previewing one document of every previewable type, no file containing any of their content exists anywhere on disk, checked by searching the application's folders and the system's temporary and cache folders for each document's bytes.
- **SC-003**: Across a test set of hostile and malformed documents (PDFs with scripts, actions on opening, external links, remote images and fonts, embedded files and launch actions; truncated and corrupted files; files whose names misstate their type), none runs anything, makes a network request, reads another file, or ends the application; each is either shown inertly or reported as unpreviewable.
- **SC-004**: With the default setting, every opening of a document in another program is preceded by the confirmation; with "Open in another app", the first in every database session is; and in every cancelled confirmation nothing is written to disk.
- **SC-005**: A user can read any previewable attachment with a single choice from its firearm's or accessory's record, without any other program.
- **SC-006**: When the application locks while a preview is open, the preview's content is off the screen before the chooser is shown, in every case covered by the lock tests of feature 003.
- **SC-007**: Searching the collection for a word in a firearm's attached document's name finds that firearm, and searching the Accessories page for a word in an accessory's finds that accessory, each within the 500 ms search budget at 10,000 firearms and 10,000 accessories.
- **SC-008**: Across a test set of files that are not document types or whose content misstates their type (programs, scripts, shortcuts, installers, web pages, SVG, macro-enabled Office files, photos, and HTML named `.pdf`), every one is refused when attached, and none stored before this feature is ever handed to another program; no external open happens without the operating system's confirmation being answered "open".

## Assumptions

- **Which types are documents and which are previewed**: documents are the paperwork of a collection (receipts, service records, registration forms, bills of sale and other transfer records), so only document types can be attached (FR-016); photos are attached as photos. Of those, PDF, TIFF (the usual format of scanned paperwork), plain text and CSV are previewed. Not every supported system can display TIFF on its own, so FR-002 means the preview must not rely on it. Office and OpenDocument files and RTF are opened in another program; adding a type later is a change to FR-016's and FR-001's lists.
- **Trusted confirmation**: the confirmation before opening in another program is the operating system's dialog (FR-008), not one drawn by the application. That is a deliberate exception to the application's own dialog pattern (constitution III), made so a compromised interface cannot answer it (issue #13's security findings).
- **View only**: the preview does not print, annotate, edit, fill in forms, or save a copy elsewhere; those need another program and so go through "Open in another app…". Selecting and copying text from the preview, and searching within a document, are not offered (clarification of 2026-10-02), though a PDF's text is available to screen readers (FR-005); the collection search covers document names instead (FR-015).
- **A document's own password** is not asked for or stored; a protected PDF is opened in another program.
- **Links in documents** are shown but not followed. Opening a link in a browser would send data off the device and is out of scope.
- **The external path is unchanged**: where the copy goes, when it is deleted and how (secure deletion where supported) stay as 001 FR-035 and 003 FR-022 and FR-037 define them; this feature only puts a confirmation in front of it and makes the preview the default.
- **No change to stored records**: documents are stored as they are today; the only changes are which files can be attached and the type recorded with them (FR-016), that the firearm and accessory search indexes cover their own records' document names (FR-015), and the computer's new setting (FR-011). Documents of other types stored before this feature stay listed and can be deleted, but are neither previewed nor handed to another program (FR-017).
- **Security review**: rendering documents inside the application is new attack surface (malicious PDFs, what the preview can reach, the content security policy). The plan MUST record how FR-003 and FR-004 are met, and the release security review (issue #21) MUST cover the preview once built.
- **Design**: the preview and the document list's actions follow the existing photo viewer and dialogs (constitution III), and the preview is added to the screenshot walk.
- **Unreleased application**: any schema change is made in place.

## Relationship to Earlier Features

This feature extends `specs/001-firearms-inventory/`. It **amends**:

- **FR-010** ("attach arbitrary documents"): only document types can be attached as documents, and photos only as photos (FR-016).
- **FR-010** ("reopen them from the record"): reopening previews the document inside the application by default (FR-001, FR-013), and opening it in another program takes a confirmation (FR-008).
- **User Story 4, scenario 4** ("can be reopened from it"): as FR-010 above.
- **FR-013** (search): the search also matches the names of attached documents (FR-015).
- **FR-035** and **SC-010** are unchanged and still govern the copy made for another program. The clarification that "previewing documents inside the application, and warning the user before opening one externally, are a separate future feature" is resolved by this feature.

It also amends `specs/006-accessory-links/`:

- **FR-007a** (an accessory's documents "exactly as a firearm" has them, 001 FR-010) and **User Story 1, scenario 12** ("the receipt opens in the system's viewer"): an accessory's documents are previewed inside the application by default and opened in another program only after the confirmation, exactly as a firearm's (FR-007, FR-008, FR-013).
- **FR-018** (the Accessories page searches every recorded accessory field): it also matches the names of the accessory's attached documents (FR-015). Its rule that a firearm is not found through the records mounted on it is unchanged and extends to their documents.

It also touches `specs/003-database-protection-management/`: FR-022, FR-036 and FR-037 (deleting decrypted document copies) are unchanged, and a preview is part of the collection data removed from the screen at a lock (FR-014). 003 FR-013's list of settings kept on each computer gains how documents open (FR-011).

## Source Request

This feature was filed as GitHub issue #13, "Preview documents in the app, with consent before opening externally" (labels: new feature, needs spec, security). It is the second half of the original `open_document` item; the first half, deleting decrypted copies at exit, shipped as 001's FR-035.

**The request, verbatim:**

> Ideally it would be previewable within the program rather than opening an external viewer - but the user should be able to say what it is they want / open externally has an extra step and notify the users of the potential consequences.

**Notes recorded on the issue, with where this spec answers them:**

- *Why it needs its own spec:* "It's new user-facing behavior that goes beyond FR-010's 'reopen from the record': a preview component, a preference (preview in the app vs. open externally), and a consent step that tells the user the consequences before a decrypted copy leaves the app." → The preview (FR-001 to FR-007), the setting (FR-011 to FR-013) and the confirmation with its consequences (FR-008 to FR-010).
- *Security:* "Previewing PDFs and images in the webview needs its own security review: malicious PDFs, the CSP, and what the preview can reach. The release security review (#21) should cover it once built." → FR-003, FR-004, SC-002, SC-003, and the security review assumption.
- *Dependencies:* "Stands alone, so it can be specified at any point."

**Security findings posted on the issue (2026-10), with where this spec answers them:**

- *[High] Opening unrestricted attachments can execute Windows programs*: `open_document` writes any attachment's bytes and calls the operating system's default handler (`ShellExecuteExW` on Windows), so an attached program runs; a confirmation drawn only by the interface is bypassable by a compromised web view. Recommendation: restrict preview to validated inert formats, never send executable or script types to the opener, and require a trusted native decision for the exact attachment. → FR-008 (operating system confirmation), FR-012, FR-016 (document types only, by content), FR-017 (checked again at opening; the copy's extension from the type found), SC-008.
- *[Medium] Opening HTML/SVG attachments permits document-controlled network requests through an external viewer*: an HTML or SVG attachment opened in another program loaded a remote image without further consent, and HTML named `.pdf` was shown with a PDF label. Recommendation: disable active content and external loading in previews, validate actual formats, reject misleading type metadata, and require an explicit decision before handing an attachment to another program. → FR-004, FR-009 (c), FR-016 (HTML and SVG refused; content must match the name), FR-017, SC-003, SC-008. The web view's own navigation (issue #73) is a separate issue.
