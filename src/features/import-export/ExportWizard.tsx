import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Button, ProgressBar, Select, TextField } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import * as importExportService from "./importExportService";
import type { ExportResult, SpreadsheetFormat } from "./types";

export interface ExportWizardProps {
  /** Whether a search/group filter is currently active in BrowsePage — lets
   * the user choose to export just what's currently filtered (Edge Case:
   * "export while a filter is active" per contracts/tauri-commands.md). */
  hasActiveFilter: boolean;
  onClose: () => void;
}

/** Export the collection to a spreadsheet plus a sibling photos folder
 * (US5, FR-018). A single action: pick format/destination/scope, then
 * watch progress until the files are written. */
export function ExportWizard({ hasActiveFilter, onClose }: ExportWizardProps) {
  const [format, setFormat] = useState<SpreadsheetFormat>("csv");
  const [destinationFolder, setDestinationFolder] = useState("");
  const [scope, setScope] = useState<"all" | "filtered">("all");
  const [exporting, setExporting] = useState(false);
  const [result, setResult] = useState<ExportResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [progressKey, setProgressKey] = useState(0);

  async function handleChooseFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") setDestinationFolder(selected);
  }

  async function handleExport() {
    if (!destinationFolder) {
      setError("Choose a destination folder first.");
      return;
    }
    setError(null);
    setResult(null);
    setExporting(true);
    setProgressKey((k) => k + 1);
    try {
      const exported = await importExportService.exportCollection({
        format,
        destinationFolder,
        scope,
      });
      setResult(exported);
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "Export failed.");
    } finally {
      setExporting(false);
    }
  }

  return (
    <div>
      <h2>Export collection</h2>
      <p>
        Exports every recorded field to a spreadsheet, plus a sibling folder of your photos in their
        original format. Files leave the device unencrypted once exported.
      </p>

      <Select
        label="Format"
        value={format}
        onValueChange={(value) => setFormat(value as SpreadsheetFormat)}
        options={[
          { value: "csv", label: "CSV" },
          { value: "xlsx", label: "Excel (.xlsx)" },
        ]}
      />

      {hasActiveFilter && (
        <Select
          label="Scope"
          value={scope}
          onValueChange={(value) => setScope(value as "all" | "filtered")}
          options={[
            { value: "all", label: "Entire collection" },
            { value: "filtered", label: "Only the current search/group filter" },
          ]}
        />
      )}

      <TextField
        label="Destination folder"
        value={destinationFolder}
        onChange={(e) => setDestinationFolder(e.target.value)}
      />
      <Button variant="secondary" onClick={handleChooseFolder} disabled={exporting}>
        Choose folder…
      </Button>

      {exporting && (
        <ProgressBar
          key={progressKey}
          eventName="export_collection:progress"
          label="Export progress"
        />
      )}

      {error && (
        <p className="hd-field__error" role="alert">
          {error}
        </p>
      )}

      {result && (
        <p>
          Exported {result.exportedFirearmCount} firearm(s) and {result.exportedPhotoCount} photo(s)
          to {result.spreadsheetPath} and {result.photosFolderPath}.
        </p>
      )}

      <div className="hd-dialog__actions">
        <Button variant="secondary" onClick={onClose} disabled={exporting}>
          Close
        </Button>
        <Button variant="primary" onClick={handleExport} disabled={exporting}>
          Export
        </Button>
      </div>
    </div>
  );
}
