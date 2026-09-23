// Mirrors src-tauri/src/models/photo.rs and
// src-tauri/src/models/document_attachment.rs's wire shapes (camelCase),
// User Story 4.

export interface PhotoSummary {
  id: number;
  firearmId: number;
  originalFilename: string;
  mimeType: string;
  thumbnailBytes: number[];
  sortOrder: number;
  createdAt: string;
}

export interface DocumentSummary {
  id: number;
  firearmId: number;
  originalFilename: string;
  mimeType: string;
  createdAt: string;
}
