import { invoke } from "../../services/tauriClient";
import type { Firearm } from "../firearms/types";
import type { DocumentDetail, DocumentSummary, PhotoSummary } from "./types";

export function listPhotos(firearmId: number): Promise<PhotoSummary[]> {
  return invoke<PhotoSummary[]>("list_photos", { firearmId });
}

export function addPhoto(
  firearmId: number,
  fileBytes: number[],
  originalFilename: string,
  mimeType: string,
): Promise<PhotoSummary> {
  return invoke<PhotoSummary>("add_photo", { firearmId, fileBytes, originalFilename, mimeType });
}

export function setThumbnailPhoto(firearmId: number, photoId: number): Promise<Firearm> {
  return invoke<Firearm>("set_thumbnail_photo", { firearmId, photoId });
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

export function listDocuments(firearmId: number): Promise<DocumentSummary[]> {
  return invoke<DocumentSummary[]>("list_documents", { firearmId });
}

export function addDocument(
  firearmId: number,
  fileBytes: number[],
  originalFilename: string,
  mimeType: string,
): Promise<DocumentSummary> {
  return invoke<DocumentSummary>("add_document", {
    firearmId,
    fileBytes,
    originalFilename,
    mimeType,
  });
}

export function getDocument(id: number): Promise<DocumentDetail> {
  return invoke<DocumentDetail>("get_document", { id });
}

/** Opens the document in the OS default app for its file type (FR-010). */
export function openDocument(id: number): Promise<void> {
  return invoke<void>("open_document", { id });
}

export function deleteDocument(id: number, confirmed: boolean): Promise<{ deleted: boolean }> {
  return invoke<{ deleted: boolean }>("delete_document", { id, confirmed });
}
