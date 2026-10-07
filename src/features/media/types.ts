// Mirrors src-tauri/src/models/photo.rs and
// src-tauri/src/models/document_attachment.rs's wire shapes (camelCase),
// User Story 4. specs/006-accessory-links: both belong to an owner, a
// firearm or an accessory.

import type { RecordRef } from "../mounts/types";

export interface PhotoSummary {
  id: number;
  owner: RecordRef;
  originalFilename: string;
  mimeType: string;
  thumbnailBytes: number[];
  sortOrder: number;
  createdAt: string;
}

export type PreviewKind = "pdf" | "tiff" | "text";

export interface DocumentSummary {
  id: number;
  owner: RecordRef;
  originalFilename: string;
  /** Always the canonical type found by the content check (FR-016). */
  mimeType: string;
  createdAt: string;
  /** From the recorded type; the content is confirmed at `open_preview`. */
  previewKind: PreviewKind | null;
  /** False for a PDF while PDF preview is off on this computer (FR-003a). */
  previewAvailable: boolean;
  /** False only for rows stored before 007 with a non-document type. */
  openable: boolean;
}

/** One kind of document the app keeps; `listDocumentTypes` gives them in table order. One entry
 * per canonical type, so Word and Spreadsheet each appear twice (.doc/.docx, .xls/.xlsx). */
export interface DocumentType {
  label: string;
  /** Lowercase, no dot. */
  extensions: string[];
  /** The canonical type recorded. */
  mimeType: string;
  previewKind: PreviewKind | null;
}

/** Points (1/72 in), at the TIFF's DPI (200 if absent). */
export interface PageSize {
  width: number;
  height: number;
}

export type PreviewInfo =
  | { previewId: number; documentId: number; kind: "pdf" }
  | { previewId: number; documentId: number; kind: "tiff"; pages: PageSize[] }
  | { previewId: number; documentId: number; kind: "text"; text: string };

/** Logical px, relative to the window's content area. */
export interface SurfaceBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export type PdfEndReason = "copyCaught" | "noViewer" | "failed";

export type DocumentOpening = "preview" | "external";
