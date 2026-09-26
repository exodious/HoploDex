import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Button, ChoiceCards, Dialog, Icon, ProgressBar, TextField } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import { useCollection } from "../app/collectionStore";
import * as browseService from "../browse/browseService";
import type { BrowseState } from "../browse/types";
import * as importExportService from "./importExportService";
import type { ExportResult, SpreadsheetFormat } from "./types";
import "./importExport.css";

export interface ExportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** The collection page's current search and filters, offered as an
   * export scope (Edge Case: "export while a filter is active"). */
  browse: BrowseState;
}

/** Export to a spreadsheet plus a sibling folder of original photos in a
 * single action (US5, FR-018, SC-005). */
export function ExportDialog({ open, onOpenChange, browse }: ExportDialogProps) {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Export collection"
      description={
        <>
          Exports the collection to a spreadsheet. The file is <strong>not encrypted</strong>:
          anyone who can open it can read it. For encrypted backups of the whole database, see
          Database settings.
        </>
      }
      bare
    >
      <ExportForm browse={browse} onClose={() => onOpenChange(false)} />
    </Dialog>
  );
}

function ExportForm({ browse, onClose }: { browse: BrowseState; onClose: () => void }) {
  const { firearms } = useCollection();
  const [format, setFormat] = useState<SpreadsheetFormat>("csv");
  const [scope, setScope] = useState<"all" | "filtered">("all");
  const [folder, setFolder] = useState("");
  const [filteredCount, setFilteredCount] = useState<number | null>(null);
  const [exporting, setExporting] = useState(false);
  const [result, setResult] = useState<ExportResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [folderError, setFolderError] = useState<string | undefined>();

  const query = browse.query.trim();
  const filter = { query: query || undefined, includeDisposed: browse.includeDisposed };
  const disposedCount = firearms.filter((f) => f.status === "disposed").length;
  const filterActive = query !== "" || (!browse.includeDisposed && disposedCount > 0);

  useEffect(() => {
    if (!filterActive) return;
    browseService
      .listFirearms(filter)
      .then((r) => setFilteredCount(r.groups.reduce((n, g) => n + g.firearms.length, 0)))
      .catch(() => setFilteredCount(null));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filterActive, query, browse.includeDisposed]);

  async function chooseFolder() {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: "Export to folder",
    });
    if (typeof selected === "string") {
      setFolder(selected);
      setFolderError(undefined);
    }
  }

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    if (!folder.trim()) {
      setFolderError("Choose the folder to save the export in.");
      return;
    }
    setError(null);
    setExporting(true);
    try {
      setResult(
        await importExportService.exportCollection({
          format,
          destinationFolder: folder.trim(),
          scope,
          filter: scope === "filtered" ? filter : undefined,
        }),
      );
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "The export didn't finish.");
    } finally {
      setExporting(false);
    }
  }

  if (result) {
    return (
      <>
        <div className="hd-dialog__body">
          <div className="hd-outcome" role="status">
            <Icon name="check" size={22} />
            <p className="hd-outcome__headline">
              Exported {result.exportedFirearmCount}{" "}
              {result.exportedFirearmCount === 1 ? "firearm" : "firearms"} and{" "}
              {result.exportedPhotoCount} {result.exportedPhotoCount === 1 ? "photo" : "photos"}.
            </p>
          </div>
          <dl className="hd-paths">
            <div>
              <dt>Spreadsheet</dt>
              <dd>{result.spreadsheetPath}</dd>
            </div>
            <div>
              <dt>Photos</dt>
              <dd>{result.photosFolderPath}</dd>
            </div>
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
              value: "csv",
              label: "CSV (.csv)",
              description: "Plain text; opens in any spreadsheet app.",
            },
            {
              value: "xlsx",
              label: "Excel (.xlsx)",
              description: "For Excel, Numbers, or LibreOffice Calc.",
            },
          ]}
        />

        {filterActive && (
          <ChoiceCards<"all" | "filtered">
            label="Firearms to include"
            value={scope}
            onChange={setScope}
            minCardWidth={180}
            options={[
              {
                value: "all",
                label: `Entire collection (${firearms.length})`,
                description: "Every record, including disposed firearms.",
              },
              {
                value: "filtered",
                label: `Current results${filteredCount != null ? ` (${filteredCount})` : ""}`,
                description: query
                  ? `Firearms matching “${query}”${browse.includeDisposed ? "" : ", not counting disposed ones"}.`
                  : "Active firearms only.",
              },
            ]}
          />
        )}

        <TextField
          label="Save to folder"
          required
          value={folder}
          onChange={(e) => {
            setFolder(e.target.value);
            setFolderError(undefined);
          }}
          error={folderError}
          spellCheck={false}
          trailing={
            <button type="button" className="hd-input__text-action" onClick={chooseFolder}>
              Choose…
            </button>
          }
        />

        <p className="hd-privacy-note" role="note">
          <Icon name="alert" size={16} />
          <span>
            The spreadsheet and the photos will be written to{" "}
            {folder.trim() ? (
              <strong className="hd-privacy-note__path">{folder.trim()}</strong>
            ) : (
              "the folder you choose"
            )}
            , unencrypted and outside HoploDex’s encrypted database. Anyone who can open that folder
            can read these records, including serial numbers and values.
          </span>
        </p>

        {exporting && (
          <ProgressBar
            eventName="export_collection:progress"
            label="Export progress"
            unit="firearms"
          />
        )}
        {error && (
          <p className="hd-banner hd-banner--error" role="alert">
            {error}
          </p>
        )}
      </div>
      <footer className="hd-dialog__footer">
        <Button variant="secondary" onClick={onClose} disabled={exporting}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" icon="upload" pending={exporting}>
          {exporting ? "Exporting…" : "Export"}
        </Button>
      </footer>
    </form>
  );
}
