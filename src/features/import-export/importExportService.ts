import { invoke } from "../../services/tauriClient";
import type {
  ExportCollectionInput,
  ExportResult,
  ExportScope,
  GetExportScopeInput,
  ImportCollectionInput,
  ImportResult,
  ResolveImportConflictsInput,
  ResolveResult,
} from "./types";

export function getExportScope(input: GetExportScopeInput): Promise<ExportScope> {
  return invoke<ExportScope>("get_export_scope", { input });
}

export function exportCollection(input: ExportCollectionInput): Promise<ExportResult> {
  return invoke<ExportResult>("export_collection", { input });
}

export function importCollection(input: ImportCollectionInput): Promise<ImportResult> {
  return invoke<ImportResult>("import_collection", { input });
}

export function resolveImportConflicts(input: ResolveImportConflictsInput): Promise<ResolveResult> {
  return invoke<ResolveResult>("resolve_import_conflicts", { input });
}
