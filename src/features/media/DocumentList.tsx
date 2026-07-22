import { useEffect, useState } from "react";
import { Button, ConfirmDialog } from "../../components";
import { bytesToDataUrl, fileToByteArray } from "../../lib/bytes";
import { CommandFailure } from "../../services/tauriClient";
import * as mediaService from "./mediaService";
import type { DocumentSummary } from "./types";

export interface DocumentListProps {
  firearmId: number;
}

/** Document attachment (e.g. a PDF receipt/appraisal) and reopen control
 * for a firearm (US4, FR-010). */
export function DocumentList({ firearmId }: DocumentListProps) {
  const [documents, setDocuments] = useState<DocumentSummary[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<number | null>(null);

  async function refresh() {
    try {
      setDocuments(await mediaService.listDocuments(firearmId));
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "Failed to load documents.");
    }
  }

  useEffect(() => {
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [firearmId]);

  async function handleFileSelected(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    setError(null);
    try {
      const bytes = await fileToByteArray(file);
      await mediaService.addDocument(
        firearmId,
        bytes,
        file.name,
        file.type || "application/octet-stream",
      );
      await refresh();
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "Failed to attach this document.");
    }
  }

  async function handleReopen(id: number) {
    setError(null);
    try {
      const detail = await mediaService.getDocument(id);
      const url = bytesToDataUrl(detail.fileBytes, detail.mimeType);
      window.open(url, "_blank");
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "Failed to reopen this document.");
    }
  }

  async function handleDelete(id: number) {
    setError(null);
    try {
      await mediaService.deleteDocument(id, true);
      await refresh();
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "Failed to delete this document.");
    } finally {
      setDeletingId(null);
    }
  }

  return (
    <div>
      <h3>Documents</h3>
      {error && (
        <p className="hd-field__error" role="alert">
          {error}
        </p>
      )}
      {documents.length === 0 && <p>No documents attached.</p>}
      <ul>
        {documents.map((doc) => (
          <li key={doc.id}>
            {doc.originalFilename}{" "}
            <Button variant="secondary" onClick={() => handleReopen(doc.id)}>
              Reopen
            </Button>{" "}
            <Button variant="danger" onClick={() => setDeletingId(doc.id)}>
              Delete
            </Button>
          </li>
        ))}
      </ul>

      <input type="file" onChange={handleFileSelected} aria-label="Attach document" />

      <ConfirmDialog
        open={deletingId != null}
        onOpenChange={(open) => !open && setDeletingId(null)}
        title="Delete this document?"
        description="This permanently removes the document. This cannot be undone."
        confirmLabel="Delete"
        onConfirm={() => {
          if (deletingId != null) void handleDelete(deletingId);
        }}
      />
    </div>
  );
}
