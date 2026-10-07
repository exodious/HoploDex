import { invoke, listen } from "../../services/tauriClient";
import type { RecordRef } from "../mounts/types";
import type {
  DocumentSummary,
  DocumentType,
  PdfEndReason,
  PhotoSummary,
  PreviewInfo,
  SurfaceBounds,
} from "./types";

export function listPhotos(owner: RecordRef): Promise<PhotoSummary[]> {
  return invoke<PhotoSummary[]>("list_photos", { owner });
}

export function addPhoto(
  owner: RecordRef,
  fileBytes: number[],
  originalFilename: string,
  mimeType: string,
): Promise<PhotoSummary> {
  return invoke<PhotoSummary>("add_photo", { owner, fileBytes, originalFilename, mimeType });
}

/** Adds a photo from a file on disk — what a drop onto the window delivers. */
export function addPhotoFromPath(owner: RecordRef, path: string): Promise<PhotoSummary> {
  return invoke<PhotoSummary>("add_photo_from_path", { owner, path });
}

export function setThumbnailPhoto(
  owner: RecordRef,
  photoId: number,
): Promise<{ thumbnailPhotoId: number }> {
  return invoke<{ thumbnailPhotoId: number }>("set_thumbnail_photo", { owner, photoId });
}

export function deletePhoto(photoId: number, confirmed: boolean): Promise<{ deleted: boolean }> {
  return invoke<{ deleted: boolean }>("delete_photo", { photoId, confirmed });
}

export function getPhotoThumbnail(photoId: number): Promise<number[]> {
  return invoke<number[]>("get_photo_thumbnail", { photoId });
}

/** Full-resolution original, delivered as raw bytes (binary IPC). */
export function getPhotoOriginal(photoId: number): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("get_photo_original", { photoId });
}

export function listDocuments(owner: RecordRef): Promise<DocumentSummary[]> {
  return invoke<DocumentSummary[]>("list_documents", { owner });
}

/** The document types the backend keeps, in table order: the picker's `accept` and the drop
 * router's document test read this; the backend's content check stays the authority. */
export function listDocumentTypes(): Promise<DocumentType[]> {
  return invoke<DocumentType[]>("list_document_types");
}

/** The type recorded is the one the backend's content check finds, so none is sent. */
export function addDocument(
  owner: RecordRef,
  fileBytes: number[],
  originalFilename: string,
): Promise<DocumentSummary> {
  return invoke<DocumentSummary>("add_document", { owner, fileBytes, originalFilename });
}

/** Attaches a document from a file on disk — what a drop onto the window delivers. */
export function addDocumentFromPath(owner: RecordRef, path: string): Promise<DocumentSummary> {
  return invoke<DocumentSummary>("add_document_from_path", { owner, path });
}

/** Hands the document to the OS default app for its type, after the native
 * confirmation (FR-010; contracts/tauri-commands.md `open_document`).
 * `opened` is false when the user declined the confirmation. */
export function openDocument(id: number): Promise<{ opened: boolean }> {
  return invoke<{ opened: boolean }>("open_document", { id });
}

export function deleteDocument(id: number, confirmed: boolean): Promise<{ deleted: boolean }> {
  return invoke<{ deleted: boolean }>("delete_document", { id, confirmed });
}

/** Opens the in-app preview of a document; replaces any preview already open
 * (contracts/tauri-commands.md `open_preview`). */
export function openPreview(documentId: number): Promise<PreviewInfo> {
  return invoke<PreviewInfo>("open_preview", { documentId });
}

/** Places the PDF surface over the page area, and shows or hides it. */
export function setPreviewBounds(
  previewId: number,
  bounds: SurfaceBounds,
  visible: boolean,
): Promise<void> {
  return invoke<void>("set_preview_bounds", { previewId, bounds, visible });
}

/** Gives the PDF surface the keyboard focus (F6). */
export function focusPreview(previewId: number): Promise<void> {
  return invoke<void>("focus_preview", { previewId });
}

/** One TIFF page as PNG bytes (binary IPC) at `widthPx` pixels wide. */
export function renderPreviewPage(
  previewId: number,
  page: number,
  widthPx: number,
): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("render_preview_page", { previewId, page, widthPx });
}

/** Closes the preview. Idempotent: a stale id is ignored. */
export function closePreview(previewId: number): Promise<void> {
  return invoke<void>("close_preview", { previewId });
}

/** The four preview events carry the id of the preview they are about; a
 * listener hears only its own. Each returns the function that unsubscribes. */
function onPreviewEvent<T extends { previewId: number }>(
  event: string,
  previewId: number,
  handler: (payload: T) => void,
): () => void {
  return listen<T>(event, (payload) => {
    if (payload.previewId === previewId) handler(payload);
  });
}

export function onPdfReady(previewId: number, handler: () => void): () => void {
  return onPreviewEvent("preview:pdf-ready", previewId, () => handler());
}

export function onPdfEnded(previewId: number, handler: (reason: PdfEndReason) => void): () => void {
  return onPreviewEvent<{ previewId: number; reason: PdfEndReason }>(
    "preview:pdf-ended",
    previewId,
    ({ reason }) => handler(reason),
  );
}

/** Escape was pressed in the PDF surface. */
export function onPreviewEscape(previewId: number, handler: () => void): () => void {
  return onPreviewEvent("preview:escape", previewId, () => handler());
}

/** F6 was pressed in the PDF surface; the main web view has the focus again. */
export function onPreviewFocusChrome(previewId: number, handler: () => void): () => void {
  return onPreviewEvent("preview:focus-chrome", previewId, () => handler());
}
