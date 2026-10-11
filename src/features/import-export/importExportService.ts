import { invoke } from "../../services/tauriClient";
import type { SessionScope } from "../session/sessionScope";
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

export function getExportScope(
  scope: SessionScope,
  input: GetExportScopeInput,
): Promise<ExportScope> {
  return scope.invoke<ExportScope>("get_export_scope", { input });
}

export function exportCollection(
  scope: SessionScope,
  input: ExportCollectionInput,
): Promise<ExportResult> {
  return scope.invoke<ExportResult>("export_collection", { input });
}

export function importCollection(
  scope: SessionScope,
  input: ImportCollectionInput,
): Promise<ImportResult> {
  return scope.invoke<ImportResult>("import_collection", { input });
}

/** Stops the running import if it is still reading its files; the pending
 * `importCollection` then fails with `OPERATION_STOPPED`. Unscoped: it acts on
 * the running operation, and does nothing once rows are being saved
 * (contracts/tauri-commands.md `cancel_import`). */
export function cancelImport(): Promise<null> {
  return invoke<null>("cancel_import");
}

export function resolveImportConflicts(
  scope: SessionScope,
  input: ResolveImportConflictsInput,
): Promise<ResolveResult> {
  return scope.invoke<ResolveResult>("resolve_import_conflicts", { input });
}
