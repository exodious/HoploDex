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

/** specs/004-cartridges-action-types FR-025: a blank caliber worked out from
 * the row's cartridge. */
export interface DerivedCaliber {
  row: number;
  /** As recorded (after snapping). */
  cartridge: string;
  /** As recorded. */
  caliber: string;
  source: "catalog" | "guess";
}

/** FR-026, SC-007: a value changed to the spelling already in use. */
export interface SnappedValue {
  row: number;
  field: "make" | "model" | "cartridge" | "caliber";
  /** As in the sheet, trimmed. */
  sheetValue: string;
  recordedValue: string;
}

export interface ImportResult {
  sessionId: string;
  importedCount: number;
  updatedCount: number;
  skippedCount: number;
  rowErrors: RowError[];
  conflicts: ImportConflict[];
  /** specs/002-firearm-identification FR-009: rows whose original marks
   * match another active firearm's, imported anyway (US4-6). */
  warnings: RowError[];
  /** Rows imported or awaiting a decision, never failed ones. */
  derivedCalibers: DerivedCaliber[];
  /** Rows imported or awaiting a decision, never failed ones. */
  snappedValues: SnappedValue[];
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
  /** As `ImportResult.warnings`, for rows saved by an `overwrite` or
   * `duplicate` resolution. */
  warnings: RowError[];
}
