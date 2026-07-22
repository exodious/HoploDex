import { invoke } from "../../services/tauriClient";
import type {
  ExportCollectionInput,
  ExportResult,
  ImportCollectionInput,
  ImportResult,
  ResolveImportConflictsInput,
  ResolveResult,
} from "./types";

export function exportCollection(input: ExportCollectionInput): Promise<ExportResult> {
  return invoke<ExportResult>("export_collection", { input });
}

export function importCollection(input: ImportCollectionInput): Promise<ImportResult> {
  return invoke<ImportResult>("import_collection", { input });
}

export function resolveImportConflicts(input: ResolveImportConflictsInput): Promise<ResolveResult> {
  return invoke<ResolveResult>("resolve_import_conflicts", { input });
}
