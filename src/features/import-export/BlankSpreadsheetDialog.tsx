import { useState } from "react";
import type { FormEvent } from "react";
import { useSessionScope } from "../session/sessionScope";
import { withIdlePaused } from "../session/useIdleActivity";
import { Button, ChoiceCards, Dialog, Icon } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import { ChosenPathField } from "./ChosenPathField";
import * as importExportService from "./importExportService";
import type { ChosenPath, ImportTemplateResult, SpreadsheetFormat } from "./types";
import "./importExport.css";

export interface BlankSpreadsheetDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** Saves a blank import spreadsheet, the columns import reads and no records,
 * in a folder the user picks (015 US1, contracts/ui.md §2). Opened from the
 * Import dialog, above it. Nothing of the collection is in the file, so it
 * carries no warning about unencrypted records (015 FR-008). */
export function BlankSpreadsheetDialog({ open, onOpenChange }: BlankSpreadsheetDialogProps) {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Get a blank spreadsheet"
      description="A spreadsheet with the columns import reads and no records, to fill in and import. It holds nothing from your collection."
      bare
    >
      <BlankSpreadsheetForm onClose={() => onOpenChange(false)} />
    </Dialog>
  );
}

function BlankSpreadsheetForm({ onClose }: { onClose: () => void }) {
  const sessionScope = useSessionScope();
  // 015 FR-002: Excel first, since only a workbook carries the notes and text cells.
  const [format, setFormat] = useState<SpreadsheetFormat>("xlsx");
  // The folder picked in the backend's chooser, by its token. A save that
  // finishes uses it up; one that fails leaves it chosen (015 FR-009).
  const [folder, setFolder] = useState<ChosenPath | null>(null);
  const [saving, setSaving] = useState(false);
  const [result, setResult] = useState<ImportTemplateResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [folderError, setFolderError] = useState<string | undefined>();

  async function chooseFolder() {
    try {
      const chosen = await withIdlePaused(() =>
        importExportService.chooseExportFolder(sessionScope),
      );
      if (chosen) {
        setFolder(chosen);
        setFolderError(undefined);
      }
    } catch (e) {
      setFolderError(e instanceof CommandFailure ? e.message : "The folder couldn't be chosen.");
    }
  }

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    if (!folder) {
      setFolderError("Choose the folder to save the spreadsheet in.");
      return;
    }
    setError(null);
    setSaving(true);
    try {
      setResult(
        await importExportService.saveImportTemplate(sessionScope, {
          format,
          folderToken: folder.token,
        }),
      );
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "The spreadsheet wasn't saved.");
    } finally {
      setSaving(false);
    }
  }

  if (result) {
    return (
      <>
        <div className="hd-dialog__body">
          <div className="hd-outcome" role="status">
            <Icon name="check" size={22} />
            <p className="hd-outcome__headline">Saved a blank spreadsheet.</p>
          </div>
          <dl className="hd-paths">
            <div>
              <dt>{result.accessorySpreadsheetPath ? "Firearms" : "Spreadsheet"}</dt>
              <dd>{result.spreadsheetPath}</dd>
            </div>
            {result.accessorySpreadsheetPath && (
              <div>
                <dt>Accessories</dt>
                <dd>{result.accessorySpreadsheetPath}</dd>
              </div>
            )}
          </dl>
        </div>
        <footer className="hd-dialog__footer">
          <Button variant="primary" onClick={onClose}>
            Done
          </Button>
        </footer>
      </>
    );
  }

  return (
    <form className="hd-dialog__form" onSubmit={handleSubmit} noValidate>
      <div className="hd-dialog__body hd-io-body">
        <ChoiceCards<SpreadsheetFormat>
          label="Format"
          value={format}
          onChange={setFormat}
          minCardWidth={180}
          options={[
            {
              value: "xlsx",
              label: "Excel (.xlsx)",
              description:
                "One file with a Firearms and an Accessories sheet, and a note on each column.",
            },
            {
              value: "csv",
              label: "CSV (.csv)",
              description: "Two plain-text files, firearms and accessories, with headers only.",
            },
          ]}
        />

        <ChosenPathField
          label="Save to folder"
          value={
            folder ? folder.path : <span className="hd-io-chosen__none">No folder chosen</span>
          }
          chooseLabel="Choose a folder…"
          onChoose={() => void chooseFolder()}
          error={folderError}
          disabled={saving}
        />

        {error && (
          <p className="hd-banner hd-banner--error" role="alert">
            {error}
          </p>
        )}
      </div>
      <footer className="hd-dialog__footer">
        <Button variant="secondary" onClick={onClose} disabled={saving}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" icon="file" pending={saving}>
          {saving ? "Saving…" : "Save"}
        </Button>
      </footer>
    </form>
  );
}
