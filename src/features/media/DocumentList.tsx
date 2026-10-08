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
import type { ViewerMessage } from "./DocumentPreview";
import { documentKindLabel } from "./documentKind";
import { useDocumentOpening } from "./documentOpening";
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
  // What the name does follows this computer's setting (FR-012); "Preview" and
  // "Open in another app…" stay on the row either way.
  const nameOpensExternally = useDocumentOpening() === "external";
  const [documents, setDocuments] = useState<DocumentSummary[] | null>(null);
  const [adding, setAdding] = useState(0);
  const [opening, setOpening] = useState<number | null>(null);
  const [deleting, setDeleting] = useState<DocumentSummary | null>(null);
  const [types, setTypes] = useState<DocumentType[]>([]);
  // What the viewer's footer says in place of a toast while a PDF is shown (ui contract §2).
  const [viewerMessage, setViewerMessage] = useState<ViewerMessage | null>(null);
  // The document the viewer shows, by id: a reload of the list can't move it.
  const [viewing, setViewing] = useState<number | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  // The system's file chooser gives the window no input while it is open
  // (research.md §15).
  useEffect(() => (inputRef.current ? pauseIdleForFileInput(inputRef.current) : undefined), []);

  /** Reloads the list. `then` runs with it in the same batch of updates as
   * the list is set, so a viewer's position follows it without a frame between. */
  async function load(then?: (list: DocumentSummary[]) => void): Promise<DocumentSummary[]> {
    try {
      const list = await mediaService.listDocuments(owner);
      setDocuments(list);
      then?.(list);
      return list;
    } catch (e) {
      notify(failureMessage(e, "Documents couldn't be loaded."), "error");
      setDocuments([]);
      return [];
    }
  }

  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [owner.kind, owner.id]);

  // The labels come from the backend's table, with no list of the app's own (FR-016).
  // A drop waits for the same load (`typesLoad`), so it is routed by the types and not
  // by an empty list; a failed load is `[]`, and rows then show their extension, which
  // is what a pre-007 row shows anyway.
  const typesLoad = useRef<Promise<DocumentType[]>>(Promise.resolve([]));
  useEffect(() => {
    const loading = mediaService.listDocumentTypes().catch((): DocumentType[] => []);
    typesLoad.current = loading;
    void loading.then(setTypes);
  }, []);

  // The viewer's message goes with the viewer.
  useEffect(() => {
    if (viewing === null) setViewerMessage(null);
  }, [viewing]);

  /** Says what a toast would say, except for a message the viewer causes while it shows
   * a PDF: the PDF surface is a separate web view that a toast can't be drawn over and
   * would hide, so the viewer's footer says it (ui contract §2). `shown` is the document
   * the viewer shows once the message is said. */
  function say(text: string, error: boolean, shown?: DocumentSummary | null) {
    if (shown?.previewKind === "pdf" && shown.previewAvailable) {
      setViewerMessage({ text, error });
    } else {
      notify(text, error ? "error" : "success");
    }
  }

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
   * and anything else is refused before any command (ui contract §6). It waits for
   * the document types; if they couldn't be loaded, nothing can be refused by
   * extension here, so every non-photo goes to the backend, whose content check is
   * the authority and which refuses what isn't a document type. */
  async function routeDrop(paths: string[]) {
    const known = await typesLoad.current;
    const isDocument = (path: string) =>
      known.length === 0 ? !isPhotoPath(path) : isDocumentPath(path, known);
    for (const path of paths) {
      if (!isPhotoPath(path) && !isDocument(path)) notify(refusal(fileName(path)), "error");
    }
    await addPaths(paths.filter(isDocument));
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
    // The deletion is the viewer's when it is open, and its message then follows
    // the viewer to the document it moves on to.
    const fromViewer = viewing !== null;
    const shownNow = documents?.find((d) => d.id === viewing) ?? null;
    try {
      await mediaService.deleteDocument(doc.id, true);
      const position = documents?.findIndex((d) => d.id === doc.id) ?? -1;
      const after = (list: DocumentSummary[]) => list[Math.min(position, list.length - 1)] ?? null;
      // From the viewer: on to the next document, or the one before it, or close.
      const list = await load((fresh) => {
        if (fromViewer) setViewing(after(fresh)?.id ?? null);
      });
      say(`Deleted ${doc.originalFilename}.`, false, fromViewer ? after(list) : null);
    } catch (e) {
      say(failureMessage(e, "The document couldn't be deleted."), true, shownNow);
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

  const dragging = useFileDrop(
    (path) => !isPhotoPath(path),
    (paths) => void routeDrop(paths),
  );
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
                {nameOpensExternally && !doc.openable ? (
                  // Nothing to open: plain text with the reason, like the disabled button.
                  <span className="hd-doc__name hd-doc__name--plain" title={NOT_OPENABLE}>
                    {doc.originalFilename}
                  </span>
                ) : (
                  <button
                    type="button"
                    className="hd-doc__name"
                    onClick={() => (nameOpensExternally ? void open(doc) : setViewing(doc.id))}
                    title={
                      nameOpensExternally
                        ? `Open ${doc.originalFilename} in another app`
                        : `Preview ${doc.originalFilename}`
                    }
                  >
                    {doc.originalFilename}
                  </button>
                )}
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
          onIndexChange={(next) => {
            setViewerMessage(null);
            setViewing(documents[next].id);
          }}
          onClose={() => setViewing(null)}
          onDelete={setDeleting}
          onPdfEnded={onPdfEnded}
          message={viewerMessage}
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
