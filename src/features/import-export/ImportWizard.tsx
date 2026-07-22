import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Button, ProgressBar, TextField } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import { ImportConflictResolver } from "./ImportConflictResolver";
import * as importExportService from "./importExportService";
import type { ImportResult, SpreadsheetFormat } from "./types";

export interface ImportWizardProps {
  onClose: () => void;
  /** Called once new/updated records exist, so the caller can refresh its
   * browse list and value summary. */
  onImported: () => void;
}

function formatForPath(filePath: string): SpreadsheetFormat {
  return filePath.toLowerCase().endsWith(".xlsx") ? "xlsx" : "csv";
}

/** Import firearm records from a spreadsheet (US5, FR-019/020). Per-row
 * validation failures are reported without discarding successful rows;
 * rows matching an existing record are handed to ImportConflictResolver. */
export function ImportWizard({ onClose, onImported }: ImportWizardProps) {
  const [filePath, setFilePath] = useState("");
  const [importing, setImporting] = useState(false);
  const [result, setResult] = useState<ImportResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [progressKey, setProgressKey] = useState(0);

  async function handleChooseFile() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Spreadsheet", extensions: ["csv", "xlsx"] }],
    });
    if (typeof selected === "string") setFilePath(selected);
  }

  async function handleImport() {
    if (!filePath) {
      setError("Choose a spreadsheet file first.");
      return;
    }
    setError(null);
    setResult(null);
    setImporting(true);
    setProgressKey((k) => k + 1);
    try {
      const imported = await importExportService.importCollection({
        filePath,
        format: formatForPath(filePath),
      });
      setResult(imported);
      if (imported.conflicts.length === 0) {
        onImported();
      }
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "Import failed.");
    } finally {
      setImporting(false);
    }
  }

  if (result && result.conflicts.length > 0) {
    return (
      <ImportConflictResolver
        sessionId={result.sessionId}
        conflicts={result.conflicts}
        onResolved={() => {
          setResult(null);
          onImported();
          onClose();
        }}
      />
    );
  }

  return (
    <div>
      <h2>Import collection</h2>
      <p>Imports firearm records (without photographs) from a spreadsheet.</p>

      <TextField label="File path" value={filePath} onChange={(e) => setFilePath(e.target.value)} />
      <Button variant="secondary" onClick={handleChooseFile} disabled={importing}>
        Choose file…
      </Button>

      {importing && (
        <ProgressBar
          key={progressKey}
          eventName="import_collection:progress"
          label="Import progress"
        />
      )}

      {error && (
        <p className="hd-field__error" role="alert">
          {error}
        </p>
      )}

      {result && (
        <div>
          <p>
            Imported {result.importedCount} new record(s). {result.rowErrors.length} row(s) failed.
          </p>
          {result.rowErrors.length > 0 && (
            <ul>
              {result.rowErrors.map((rowError) => (
                <li key={rowError.row} role="alert">
                  Row {rowError.row}: {rowError.message}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      <div className="hd-dialog__actions">
        <Button variant="secondary" onClick={onClose} disabled={importing}>
          Close
        </Button>
        <Button variant="primary" onClick={handleImport} disabled={importing}>
          Import
        </Button>
      </div>
    </div>
  );
}
