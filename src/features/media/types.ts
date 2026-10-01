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

export interface DocumentSummary {
  id: number;
  owner: RecordRef;
  originalFilename: string;
  mimeType: string;
  createdAt: string;
}
