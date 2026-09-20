// Mirrors src-tauri/src/commands/import_export.rs's wire shapes (camelCase),
// User Story 5.

import type { ListFirearmsInput } from "../browse/types";

export type SpreadsheetFormat = "csv" | "xlsx";

export interface ExportCollectionInput {
  format: SpreadsheetFormat;
  destinationFolder: string;
  scope: "all" | "filtered";
  filter?: ListFirearmsInput;
}

export interface ExportResult {
  spreadsheetPath: string;
  photosFolderPath: string;
  exportedFirearmCount: number;
  exportedPhotoCount: number;
}

export interface ImportCollectionInput {
  filePath: string;
  format: SpreadsheetFormat;
}

export interface RowError {
  row: number;
  message: string;
}

export interface ImportConflict {
  conflictId: string;
  row: number;
  existingFirearmId: number;
  /** False where FR-032 would block a duplicate record (FR-026). */
  duplicateAllowed: boolean;
  make: string;
  model: string;
  serialNumber: string | null;
}

export interface ImportResult {
  sessionId: string;
  importedCount: number;
  updatedCount: number;
  skippedCount: number;
  rowErrors: RowError[];
  /** Rows that imported but drew a warning (FR-032b). */
  warnings: RowError[];
  conflicts: ImportConflict[];
}

export type ConflictAction = "skip" | "overwrite" | "duplicate";

export interface ConflictResolution {
  conflictId: string;
  action: ConflictAction;
}

export interface ResolveImportConflictsInput {
  importSessionId: string;
  resolutions: ConflictResolution[];
  applyToRemaining?: ConflictAction;
}

export interface ResolveResult {
  resolvedCount: number;
  /** Conflicts the chosen action couldn't be applied to; still open. */
  unresolved: RowError[];
}
