// Mirrors src-tauri/src/commands/import_export.rs's wire shapes (camelCase),
// User Story 5.

import type { ListFirearmsInput } from "../browse/types";
import type { MountedEntry, RecordRef } from "../mounts/types";

export type SpreadsheetFormat = "csv" | "xlsx";

export interface ExportCollectionInput {
  format: SpreadsheetFormat;
  destinationFolder: string;
  scope: "all" | "filtered";
  filter?: ListFirearmsInput;
}

/** `get_export_scope`'s input (specs/006-accessory-links FR-020). */
export interface GetExportScopeInput {
  scope: "all" | "filtered";
  filter?: ListFirearmsInput;
}

/** What an export of a scope writes, and what it must disclose (FR-020,
 * FR-021). */
export interface ExportScope {
  firearmCount: number;
  accessoryCount: number;
  /** 005 FR-020: a firearm in the scope has a classification. */
  includesRegistration: boolean;
  /** FR-021: the scope holds an accessory. */
  includesAccessories: boolean;
}

export interface ExportResult {
  spreadsheetPath: string;
  /** The second CSV file; null for a workbook, and when no accessory is
   * exported. */
  accessorySpreadsheetPath: string | null;
  photosFolderPath: string;
  exportedFirearmCount: number;
  exportedAccessoryCount: number;
  /** Firearm and accessory photos together. */
  exportedPhotoCount: number;
}

/** One file of an import: a CSV file or a workbook (FR-022). */
export interface ImportFile {
  filePath: string;
  format: SpreadsheetFormat;
}

export interface ImportCollectionInput {
  /** One file, or two: the firearm table and the accessory table. */
  files: ImportFile[];
}

/** Which table a report entry's row is in; `row` counts within it. */
export type ImportTable = "firearms" | "accessories";

export interface RowError {
  table: ImportTable;
  row: number;
  message: string;
}

export interface ImportConflict {
  conflictId: string;
  table: ImportTable;
  row: number;
  existingRecord: RecordRef;
  /** False where FR-032 would block a duplicate record (FR-026); always
   * true for an accessory. */
  duplicateAllowed: boolean;
  make: string | null;
  model: string | null;
  serialNumber: string | null;
  /** An accessory's kind; null for a firearm. */
  kindName: string | null;
  /** Issue #56: everything mounted on the record when the row would dispose
   * of it while it is active, so replacing it can ask which go with it.
   * Empty otherwise. */
  mounted: MountedEntry[];
}

/** specs/004-cartridges-action-types FR-025: a blank caliber worked out from
 * the row's cartridge. */
export interface DerivedCaliber {
  table: ImportTable;
  row: number;
  /** As recorded (after snapping). */
  cartridge: string;
  /** As recorded. */
  caliber: string;
  source: "catalog" | "guess";
}

/** FR-026, SC-007: a value changed to the spelling already in use. */
export interface SnappedValue {
  table: ImportTable;
  row: number;
  field: "make" | "model" | "cartridge" | "caliber" | "registrationForm" | "registeredTo";
  /** As in the sheet, trimmed. */
  sheetValue: string;
  recordedValue: string;
}

export interface ImportResult {
  sessionId: string;
  /** Both tables' rows. */
  importedCount: number;
  /** The accessory rows among `importedCount`. */
  importedAccessoryCount: number;
  updatedCount: number;
  skippedCount: number;
  rowErrors: RowError[];
  conflicts: ImportConflict[];
  /** specs/002-firearm-identification FR-009: rows whose original marks
   * match another active firearm's, imported anyway (US4-6); and
   * specs/006-accessory-links FR-023: mounts that couldn't be made. */
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
  /** For an `overwrite` that disposes of the record: the records of its
   * `mounted` list to dispose of with it, with the row's type, recipient and
   * date and no price. The rest are kept (issue #56). */
  withMounted?: RecordRef[];
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
