import { useEffect, useRef, useState } from "react";
import { pauseIdleForFileInput } from "../session/useIdleActivity";
import type { ChangeEvent } from "react";
import { Button, ConfirmDialog, Icon, useToast } from "../../components";
import { fileToByteArray } from "../../lib/bytes";
import { formatDate } from "../../lib/dates";
import { CommandFailure } from "../../services/tauriClient";
import type { RecordRef } from "../mounts/types";
import * as mediaService from "./mediaService";
import type { DocumentSummary, DocumentType, PdfEndReason } from "./types";
import { DocumentPreview } from "./DocumentPreview";
import { documentKindLabel } from "./documentKind";
import { documentAccept, fileName, isDocumentPath, isPhotoPath } from "./filePaths";
import { useFileDrop } from "./useFileDrop";
import "./media.css";

export interface DocumentListProps {
  /** The firearm or accessory the documents belong to (006 FR-007a). */
  owner: RecordRef;
}

function failureMessage(e: unknown, fallback: string): string {
  return e instanceof CommandFailure ? e.message : fallback;
}

const PDF_OFF = "PDFs can't be previewed on this computer.";
const NOT_OPENABLE = "This can't be opened: it is not a document type.";

/** A dropped file that isn't a document type (ui contract §6). */
const refusal = (name: string) =>
  `${name} wasn't attached. Documents can be PDF, TIFF, text, CSV, RTF, Word, spreadsheet or OpenDocument files.`;

/** What the meta line adds after the kind and date, where it applies
 * (contract §1; FR-013: the list shows which documents can be previewed). */
function metaSuffix(doc: DocumentSummary): string {
  if (!doc.openable) return " · can't be opened: not a document type";
  if (doc.previewKind === null) return " · opens in another app";
  if (!doc.previewAvailable) return " · can't be previewed on this computer";
  return "";
}

/** Documents attached to a firearm or an accessory — receipts, appraisals, manuals — that
 * open in the computer's default app for their type (US4, FR-010), and that the viewer
 * previews inside HoploDex (007). */
export function DocumentList({ owner }: DocumentListProps) {
  const notify = useToast();
  const [documents, setDocuments] = useState<DocumentSummary[] | null>(null);
  const [adding, setAdding] = useState(0);
  const [opening, setOpening] = useState<number | null>(null);
  const [deleting, setDeleting] = useState<DocumentSummary | null>(null);
  const [types, setTypes] = useState<DocumentType[]>([]);
  // The document the viewer shows, by id: a reload of the list can't move it.
  const [viewing, setViewing] = useState<number | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  // The system's file chooser gives the window no input while it is open
  // (research.md §15).
  useEffect(() => (inputRef.current ? pauseIdleForFileInput(inputRef.current) : undefined), []);

  /** Reloads the list. `then` runs with it in the same batch of updates as
   * the list is set, so a viewer's position follows it without a frame between. */
  async function load(then?: (list: DocumentSummary[]) => void) {
    try {
      const list = await mediaService.listDocuments(owner);
      setDocuments(list);
      then?.(list);
    } catch (e) {
      notify(failureMessage(e, "Documents couldn't be loaded."), "error");
      setDocuments([]);
    }
  }

  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [owner.kind, owner.id]);

  // The labels come from the backend's table, with no list of the app's own (FR-016).
  useEffect(() => {
    mediaService.listDocumentTypes().then(setTypes, () => {
      // Rows then show their extension, which is what a pre-007 row shows anyway.
    });
  }, []);

  /** Attaches each document in turn, carrying on past one that fails so a
   * bad file doesn't cost the rest of a batch. */
  async function addAll(pending: { name: string; add: () => Promise<unknown> }[]) {
    if (pending.length === 0) return;
    setAdding(pending.length);
    let added = 0;
    let failure: string | null = null;
    for (const { name, add } of pending) {
      try {
        await add();
        added += 1;
      } catch (e) {
        // The backend's message says a photo isn't a document type; here it can say where it goes.
        failure ??=
          e instanceof CommandFailure && e.code === "DOCUMENT_TYPE_NOT_ALLOWED" && isPhotoPath(name)
            ? `${name} is a photo. Add it under Photos instead.`
            : failureMessage(e, `${name} couldn't be attached.`);
      }
    }
    setAdding(0);
    await load();
    if (added > 0) notify(`Attached ${added} ${added === 1 ? "document" : "documents"}.`);
    if (failure) notify(failure, "error");
  }

  async function addFiles(files: File[]) {
    await addAll(
      files.map((file) => ({
        name: file.name,
        add: async () => mediaService.addDocument(owner, await fileToByteArray(file), file.name),
      })),
    );
  }

  async function addPaths(paths: string[]) {
    await addAll(
      paths.map((path) => ({
        name: fileName(path),
        add: () => mediaService.addDocumentFromPath(owner, path),
      })),
    );
  }

  /** Routes a drop: documents attach, photos are Photos' (so nothing is said),
   * and anything else is refused before any command (ui contract §6). */
  function routeDrop(paths: string[]) {
    const documents = paths.filter((path) => isDocumentPath(path, types));
    for (const path of paths) {
      if (!isPhotoPath(path) && !isDocumentPath(path, types))
        notify(refusal(fileName(path)), "error");
    }
    void addPaths(documents);
  }

  async function open(doc: DocumentSummary) {
    setOpening(doc.id);
    try {
      // `opened` is false when the user declined the native question: nothing is said.
      const { opened } = await mediaService.openDocument(doc.id);
      if (opened) notify(`Opened ${doc.originalFilename} in another app.`);
    } catch (e) {
      notify(failureMessage(e, `${doc.originalFilename} couldn't be opened.`), "error");
    } finally {
      setOpening(null);
    }
  }

  async function remove(doc: DocumentSummary) {
    try {
      await mediaService.deleteDocument(doc.id, true);
      const position = documents?.findIndex((d) => d.id === doc.id) ?? -1;
      // From the viewer: on to the next document, or the one before it, or close.
      await load((list) =>
        setViewing((now) =>
          now === null ? null : (list[Math.min(position, list.length - 1)]?.id ?? null),
        ),
      );
      notify(`Deleted ${doc.originalFilename}.`);
    } catch (e) {
      notify(failureMessage(e, "The document couldn't be deleted."), "error");
    }
  }

  function handleInput(event: ChangeEvent<HTMLInputElement>) {
    const files = Array.from(event.target.files ?? []);
    event.target.value = "";
    if (files.length > 0) void addFiles(files);
  }

  const viewIndex =
    viewing === null || !documents ? -1 : documents.findIndex((d) => d.id === viewing);

  /** The backend closed the PDF surface and turned PDF preview off here, so PDFs
   * show as not previewable (contracts/tauri-commands.md, `preview:pdf-ended`). */
  function onPdfEnded(reason: PdfEndReason) {
    if (reason === "copyCaught" || reason === "noViewer") void load();
  }

  const dragging = useFileDrop((path) => !isPhotoPath(path), routeDrop);
  const accept = documentAccept(types);
  const pick = () => inputRef.current?.click();

  return (
    <section className="hd-panel hd-dropzone-host" aria-labelledby="documents-title">
      <header className="hd-panel__head">
        <h2 className="hd-panel__title" id="documents-title">
          Documents
          {documents && documents.length > 0 && (
            <span className="hd-panel__count hd-num">{documents.length}</span>
          )}
        </h2>
        <Button size="sm" icon="plus" pending={adding > 0} onClick={pick}>
          {adding > 0 ? `Attaching ${adding}…` : "Attach"}
        </Button>
        <input
          ref={inputRef}
          type="file"
          multiple
          accept={accept}
          className="hd-sr-only"
          tabIndex={-1}
          aria-label="Attach documents"
          onChange={handleInput}
        />
      </header>

      {documents && documents.length === 0 && (
        <button type="button" className="hd-dropzone hd-dropzone--compact" onClick={pick}>
          <Icon name="file" size={22} />
          <span>
            Drop receipts, bills of sale, registration forms or service records here (PDF, TIFF,
            text, Word or spreadsheet), or <span className="hd-link">choose files</span>
          </span>
        </button>
      )}

      {documents && documents.length > 0 && (
        <ul className="hd-docs">
          {documents.map((doc) => (
            <li key={doc.id} className="hd-doc">
              <Icon name="file" className="hd-doc__icon" />
              <div className="hd-doc__text">
                <button
                  type="button"
                  className="hd-doc__name"
                  onClick={() => setViewing(doc.id)}
                  title={`Preview ${doc.originalFilename}`}
                >
                  {doc.originalFilename}
                </button>
                <span className="hd-doc__meta">
                  {documentKindLabel(doc, types)} · added {formatDate(doc.createdAt.slice(0, 10))}
                  {metaSuffix(doc)}
                </span>
              </div>
              {doc.previewKind !== null && (
                <Button
                  size="sm"
                  variant="ghost"
                  icon="eye"
                  disabled={!doc.previewAvailable}
                  title={doc.previewAvailable ? undefined : PDF_OFF}
                  onClick={() => setViewing(doc.id)}
                >
                  Preview
                </Button>
              )}
              <Button
                size="sm"
                variant="ghost"
                icon="open"
                pending={opening === doc.id}
                disabled={!doc.openable}
                title={doc.openable ? undefined : NOT_OPENABLE}
                onClick={() => open(doc)}
              >
                Open in another app…
              </Button>
              <Button
                size="sm"
                variant="ghost"
                icon="trash"
                aria-label={`Delete ${doc.originalFilename}`}
                onClick={() => setDeleting(doc)}
              />
            </li>
          ))}
        </ul>
      )}

      {documents && documents.length > 0 && (
        <button type="button" className="hd-dropzone hd-dropzone--compact" onClick={pick}>
          <span>
            Drop more files here, or <span className="hd-link">choose files</span>
          </span>
        </button>
      )}

      {dragging && (
        <div className="hd-dropzone-overlay" aria-hidden>
          Drop to attach
        </div>
      )}

      {documents && viewIndex >= 0 && (
        <DocumentPreview
          documents={documents}
          index={viewIndex}
          documentTypes={types}
          onIndexChange={(next) => setViewing(documents[next].id)}
          onClose={() => setViewing(null)}
          onDelete={setDeleting}
          onPdfEnded={onPdfEnded}
        />
      )}

      <ConfirmDialog
        open={deleting != null}
        onOpenChange={(isOpen) => !isOpen && setDeleting(null)}
        title="Delete this document?"
        description={`${deleting?.originalFilename ?? "The document"} will be permanently removed from this record.`}
        confirmLabel="Delete document"
        onConfirm={() => (deleting ? remove(deleting) : undefined)}
      />
    </section>
  );
}
