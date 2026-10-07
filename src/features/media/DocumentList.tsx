import { useEffect, useRef, useState } from "react";
import { pauseIdleForFileInput } from "../session/useIdleActivity";
import type { ChangeEvent } from "react";
import { Button, ConfirmDialog, Icon, useToast } from "../../components";
import { fileToByteArray } from "../../lib/bytes";
import { formatDate } from "../../lib/dates";
import { CommandFailure } from "../../services/tauriClient";
import type { RecordRef } from "../mounts/types";
import * as mediaService from "./mediaService";
import type { DocumentSummary } from "./types";
import { fileName, isDocumentPath } from "./filePaths";
import { useFileDrop } from "./useFileDrop";
import "./media.css";

export interface DocumentListProps {
  /** The firearm or accessory the documents belong to (006 FR-007a). */
  owner: RecordRef;
}

function failureMessage(e: unknown, fallback: string): string {
  return e instanceof CommandFailure ? e.message : fallback;
}

function kindLabel(doc: DocumentSummary): string {
  if (doc.mimeType === "application/pdf") return "PDF";
  if (doc.mimeType.startsWith("image/")) return "Image";
  const extension = doc.originalFilename.split(".").pop();
  return extension && extension !== doc.originalFilename ? extension.toUpperCase() : "File";
}

/** Documents attached to a firearm or an accessory — receipts, appraisals, manuals — that
 * open in the computer's default app for their type (US4, FR-010). */
export function DocumentList({ owner }: DocumentListProps) {
  const notify = useToast();
  const [documents, setDocuments] = useState<DocumentSummary[] | null>(null);
  const [adding, setAdding] = useState(0);
  const [opening, setOpening] = useState<number | null>(null);
  const [deleting, setDeleting] = useState<DocumentSummary | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  // The system's file chooser gives the window no input while it is open
  // (research.md §15).
  useEffect(() => (inputRef.current ? pauseIdleForFileInput(inputRef.current) : undefined), []);

  async function load() {
    try {
      setDocuments(await mediaService.listDocuments(owner));
    } catch (e) {
      notify(failureMessage(e, "Documents couldn't be loaded."), "error");
      setDocuments([]);
    }
  }

  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [owner.kind, owner.id]);

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
        failure ??= failureMessage(e, `${name} couldn't be attached.`);
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

  async function open(doc: DocumentSummary) {
    setOpening(doc.id);
    try {
      await mediaService.openDocument(doc.id);
    } catch (e) {
      notify(failureMessage(e, `${doc.originalFilename} couldn't be opened.`), "error");
    } finally {
      setOpening(null);
    }
  }

  async function remove(doc: DocumentSummary) {
    try {
      await mediaService.deleteDocument(doc.id, true);
      await load();
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

  const dragging = useFileDrop(isDocumentPath, (paths) => void addPaths(paths));
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
            Drop receipts, appraisals, or manuals here, or{" "}
            <span className="hd-link">choose files</span>
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
                  onClick={() => open(doc)}
                  title={`Open ${doc.originalFilename}`}
                >
                  {doc.originalFilename}
                </button>
                <span className="hd-doc__meta">
                  {kindLabel(doc)} · added {formatDate(doc.createdAt.slice(0, 10))}
                </span>
              </div>
              <Button
                size="sm"
                variant="ghost"
                icon="open"
                pending={opening === doc.id}
                onClick={() => open(doc)}
              >
                Open
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
