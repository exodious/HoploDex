import { invoke } from "../../services/tauriClient";
import type { RecordRef } from "../mounts/types";
import type { DocumentSummary, DocumentType, PhotoSummary } from "./types";

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

/** Opens the document in the OS default app for its file type (FR-010). */
export function openDocument(id: number): Promise<void> {
  return invoke<void>("open_document", { id });
}

export function deleteDocument(id: number, confirmed: boolean): Promise<{ deleted: boolean }> {
  return invoke<{ deleted: boolean }>("delete_document", { id, confirmed });
}
