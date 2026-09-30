import { useState } from "react";
import type { FormEvent } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { withIdlePaused } from "../session/useIdleActivity";
import {
  Button,
  ConfirmDialog,
  Dialog,
  Disclosure,
  Icon,
  ProgressBar,
  SegmentedControl,
  TextField,
  useToast,
} from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import { useCollection } from "../app/collectionStore";
import * as importExportService from "./importExportService";
import type {
  ConflictAction,
  ImportConflict,
  ImportResult,
  RowError,
  SpreadsheetFormat,
} from "./types";
import "./importExport.css";

/** One of the import report's lists of what the import changed (contracts/
 * ui-entry.md §8): a folded section, shown only when it has rows, in row
 * order. */
function ReportDisclosure({
  title,
  summary,
  items,
}: {
  title: string;
  summary: string;
  items: { key: string; row: number; text: string }[];
}) {
  const [open, setOpen] = useState(false);
  if (items.length === 0) return null;
  return (
    <section className="hd-io-section">
      <Disclosure
        title={`${title} (${items.length})`}
        summary={open ? undefined : summary}
        headingLevel={3}
        open={open}
        onOpenChange={setOpen}
      >
        <p className="hd-form-note">{summary}</p>
        <ul className="hd-row-errors hd-row-errors--info">
          {items.map((item) => (
            <li key={item.key}>
              <span className="hd-row-errors__row hd-num">Row {item.row}</span>
              <span>{item.text}</span>
            </li>
          ))}
        </ul>
      </Disclosure>
    </section>
  );
}

export interface ImportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

const CONFLICT_OPTIONS: { value: ConflictAction; label: string }[] = [
  { value: "skip", label: "Keep existing" },
  { value: "overwrite", label: "Replace existing" },
  { value: "duplicate", label: "Add as new" },
];

function formatForPath(filePath: string): SpreadsheetFormat {
  return filePath.toLowerCase().endsWith(".xlsx") ? "xlsx" : "csv";
}

function count(n: number, one: string, many: string) {
  return `${n} ${n === 1 ? one : many}`;
}

/** Import firearm records, without photos, from a spreadsheet (US5,
 * FR-019). Failed rows are reported individually without discarding the
 * rest (FR-020); rows matching an existing record are resolved one by one
 * or all at once (FR-026). */
export function ImportDialog({ open, onOpenChange }: ImportDialogProps) {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Import from a spreadsheet"
      description="Adds firearm records from a CSV or Excel file with the same columns as an export. Photos aren't imported; add them to each record afterwards."
      size="lg"
      bare
    >
      <ImportFlow onClose={() => onOpenChange(false)} />
    </Dialog>
  );
}

function ImportFlow({ onClose }: { onClose: () => void }) {
  const { refresh } = useCollection();
  const notify = useToast();
  const [filePath, setFilePath] = useState("");
  const [fileError, setFileError] = useState<string | undefined>();
  const [importing, setImporting] = useState(false);
  const [result, setResult] = useState<ImportResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [choices, setChoices] = useState<Record<string, ConflictAction>>({});
  const [resolving, setResolving] = useState(false);
  const [confirmingReplace, setConfirmingReplace] = useState(false);
  const [resolvedCount, setResolvedCount] = useState<number | null>(null);
  const [unresolved, setUnresolved] = useState<RowError[]>([]);

  async function chooseFile() {
    const selected = await withIdlePaused(() =>
      openDialog({
        multiple: false,
        title: "Import spreadsheet",
        filters: [{ name: "Spreadsheet", extensions: ["csv", "xlsx"] }],
      }),
    );
    if (typeof selected === "string") {
      setFilePath(selected);
      setFileError(undefined);
    }
  }

  async function handleImport(event: FormEvent) {
    event.preventDefault();
    if (!filePath.trim()) {
      setFileError("Choose the spreadsheet to import.");
      return;
    }
    setError(null);
    setImporting(true);
    try {
      const imported = await importExportService.importCollection({
        filePath: filePath.trim(),
        format: formatForPath(filePath.trim()),
      });
      setResult(imported);
      await refresh();
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "The import didn't finish.");
    } finally {
      setImporting(false);
    }
  }

  async function applyChoices(conflicts: ImportConflict[]) {
    if (!result) return;
    setResolving(true);
    try {
      const resolved = await importExportService.resolveImportConflicts({
        importSessionId: result.sessionId,
        resolutions: conflicts.map((c) => ({
          conflictId: c.conflictId,
          action: choices[c.conflictId],
        })),
      });
      setResolvedCount((so_far) => (so_far ?? 0) + resolved.resolvedCount);
      // A decision the backend couldn't apply leaves its row open to be
      // decided again; everything else is done.
      const stillOpen = new Set(resolved.unresolved.map((u) => u.row));
      setResult({
        ...result,
        conflicts: result.conflicts.filter((c) => stillOpen.has(c.row)),
        // specs/002-firearm-identification FR-009: a resolved overwrite or
        // duplicate can add its own original-marks warning.
        warnings: [...result.warnings, ...resolved.warnings],
      });
      setUnresolved(resolved.unresolved);
      setChoices({});
      await refresh();
      notify(`Resolved ${count(resolved.resolvedCount, "matching row", "matching rows")}.`);
    } catch (e) {
      notify(
        e instanceof CommandFailure ? e.message : "The matching rows couldn't be resolved.",
        "error",
      );
    } finally {
      setResolving(false);
    }
  }

  // ── Step 1: choose a file ──────────────────────────────────────────────
  if (!result) {
    return (
      <form className="hd-dialog__form" onSubmit={handleImport} noValidate>
        <div className="hd-dialog__body hd-io-body">
          <TextField
            label="Spreadsheet file"
            required
            value={filePath}
            onChange={(e) => {
              setFilePath(e.target.value);
              setFileError(undefined);
            }}
            error={fileError}
            spellCheck={false}
            hint="Rows that match a firearm you already have (same make, model, and serial number) are held for you to decide on."
            trailing={
              <button type="button" className="hd-input__text-action" onClick={chooseFile}>
                Choose…
              </button>
            }
          />
          {importing && (
            <ProgressBar
              eventName="import_collection:progress"
              label="Import progress"
              unit="rows"
            />
          )}
          {error && (
            <p className="hd-banner hd-banner--error" role="alert">
              {error}
            </p>
          )}
        </div>
        <footer className="hd-dialog__footer">
          <Button variant="secondary" onClick={onClose} disabled={importing}>
            Cancel
          </Button>
          <Button type="submit" variant="primary" icon="download" pending={importing}>
            {importing ? "Importing…" : "Import"}
          </Button>
        </footer>
      </form>
    );
  }

  // ── Step 2: results, and any rows needing a decision ───────────────────
  const conflicts = result.conflicts;
  const undecided = conflicts.filter((c) => !choices[c.conflictId]);
  const replacing = conflicts.filter((c) => choices[c.conflictId] === "overwrite").length;

  return (
    <>
      <div className="hd-dialog__body hd-io-body">
        <div className="hd-tally" role="status" tabIndex={-1} ref={focusOnMount}>
          <Tally value={result.importedCount} label="added" />
          <Tally value={result.updatedCount} label="updated" />
          <Tally value={result.skippedCount} label="skipped" />
          <Tally
            value={result.rowErrors.length}
            label="failed"
            warn={result.rowErrors.length > 0}
          />
          {conflicts.length > 0 && <Tally value={conflicts.length} label="need a decision" warn />}
          {result.warnings.length > 0 && <Tally value={result.warnings.length} label="warnings" />}
          {resolvedCount != null && <Tally value={resolvedCount} label="resolved" />}
        </div>
        <p className="hd-sr-only">
          Imported {count(result.importedCount, "new record", "new records")}.{" "}
          {count(result.rowErrors.length, "row", "rows")} failed.
        </p>

        <ReportDisclosure
          title="Calibers filled in from the cartridge"
          summary="The cartridge named a caliber, so the blank one was filled in. “Guessed” is a best reading; check it."
          items={result.derivedCalibers.map((derived) => ({
            key: `${derived.row}`,
            row: derived.row,
            text: `${derived.cartridge} → ${derived.caliber} (${
              derived.source === "catalog" ? "built-in" : "guessed"
            })`,
          }))}
        />
        <ReportDisclosure
          title="Spellings matched to existing values"
          summary="These differed only in letter case, spacing or separators, so they now match a spelling already in use."
          items={result.snappedValues.map((snapped) => ({
            key: `${snapped.row}-${snapped.field}`,
            row: snapped.row,
            text: `${snapped.field}: “${snapped.sheetValue}” → “${snapped.recordedValue}”`,
          }))}
        />

        {result.rowErrors.length > 0 && (
          <section className="hd-io-section" aria-labelledby="row-errors-title">
            <h3 className="hd-io-section__title" id="row-errors-title">
              Rows that couldn’t be imported
            </h3>
            <p className="hd-form-note">
              Everything else was imported. Fix these rows in the spreadsheet and import it again.
            </p>
            <ul className="hd-row-errors">
              {result.rowErrors.map((rowError) => (
                <li key={rowError.row}>
                  <span className="hd-row-errors__row hd-num">Row {rowError.row}</span>
                  <span>{rowError.message}</span>
                </li>
              ))}
            </ul>
          </section>
        )}

        {result.warnings.length > 0 && (
          <section className="hd-io-section" aria-labelledby="warnings-title">
            <h3 className="hd-io-section__title" id="warnings-title">
              Warnings
            </h3>
            <p className="hd-form-note">
              These rows imported, but share original maker's marks with a firearm already in your
              collection.
            </p>
            <ul className="hd-row-errors hd-row-errors--info">
              {result.warnings.map((warning) => (
                <li key={`${warning.row}-${warning.message}`}>
                  <span className="hd-row-errors__row hd-num">Row {warning.row}</span>
                  <span>{warning.message}</span>
                </li>
              ))}
            </ul>
          </section>
        )}

        {unresolved.length > 0 && (
          <section className="hd-io-section" aria-labelledby="unresolved-title">
            <h3 className="hd-io-section__title" id="unresolved-title">
              Decisions that couldn’t be applied
            </h3>
            <p className="hd-form-note">These rows are still waiting. Choose again below.</p>
            <ul className="hd-row-errors">
              {unresolved.map((item) => (
                <li key={`${item.row}-${item.message}`}>
                  <span className="hd-row-errors__row hd-num">Row {item.row}</span>
                  <span>{item.message}</span>
                </li>
              ))}
            </ul>
          </section>
        )}

        {conflicts.length > 0 && (
          <section className="hd-io-section" aria-labelledby="conflicts-title">
            <h3 className="hd-io-section__title" id="conflicts-title">
              {count(conflicts.length, "row matches a firearm", "rows match firearms")} already in
              your collection
            </h3>
            <p className="hd-form-note">
              Same make, model, and serial number. Choose what to do with each one. A firearm can’t
              be added as new while an active one has the same make, model, and serial number.
            </p>
            <div className="hd-bulk">
              <span className="hd-bulk__label">
                {undecided.length === conflicts.length
                  ? "Apply to every row:"
                  : `Apply to the ${count(undecided.length, "remaining row", "remaining rows")}:`}
              </span>
              {CONFLICT_OPTIONS.map((option) => (
                <Button
                  key={option.value}
                  size="sm"
                  disabled={undecided.length === 0}
                  onClick={() =>
                    setChoices((prev) => ({
                      ...prev,
                      // "Add as new" skips rows where FR-032 forbids it; those
                      // stay undecided for the user to choose keep or replace.
                      ...Object.fromEntries(
                        undecided
                          .filter((c) => option.value !== "duplicate" || c.duplicateAllowed)
                          .map((c) => [c.conflictId, option.value]),
                      ),
                    }))
                  }
                >
                  {option.label}
                </Button>
              ))}
            </div>
            <table className="hd-conflicts">
              <thead>
                <tr>
                  <th scope="col">Row</th>
                  <th scope="col">Firearm</th>
                  <th scope="col">Decision</th>
                </tr>
              </thead>
              <tbody>
                {conflicts.map((conflict) => (
                  <tr key={conflict.conflictId}>
                    <td className="hd-num">{conflict.row}</td>
                    <td>
                      <span className="hd-conflicts__name">
                        {conflict.make} {conflict.model}
                      </span>
                      {conflict.serialNumber && (
                        <span className="hd-serial hd-conflicts__serial">
                          {conflict.serialNumber}
                        </span>
                      )}
                    </td>
                    <td>
                      <SegmentedControl<ConflictAction>
                        label={`Row ${conflict.row}: ${conflict.make} ${conflict.model}`}
                        hideLabel
                        size="sm"
                        value={choices[conflict.conflictId] ?? ""}
                        onChange={(value) =>
                          setChoices((prev) => ({ ...prev, [conflict.conflictId]: value }))
                        }
                        options={
                          conflict.duplicateAllowed
                            ? CONFLICT_OPTIONS
                            : CONFLICT_OPTIONS.filter((option) => option.value !== "duplicate")
                        }
                      />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
        )}

        {resolvedCount != null && conflicts.length === 0 && (
          <div className="hd-outcome" role="status">
            <Icon name="check" size={22} />
            <p className="hd-outcome__headline">
              Resolved {count(resolvedCount, "matching row", "matching rows")}.
            </p>
          </div>
        )}
      </div>

      <footer className="hd-dialog__footer">
        {conflicts.length > 0 ? (
          <>
            <p className="hd-dialog__footer-note">
              {undecided.length > 0
                ? `${count(undecided.length, "row", "rows")} still ${undecided.length === 1 ? "needs" : "need"} a decision.`
                : "Every row has a decision."}
            </p>
            <Button
              variant="primary"
              disabled={undecided.length > 0}
              pending={resolving}
              onClick={() =>
                replacing > 0 ? setConfirmingReplace(true) : void applyChoices(conflicts)
              }
            >
              Apply decisions
            </Button>
          </>
        ) : (
          <Button variant="primary" onClick={onClose}>
            Done
          </Button>
        )}
      </footer>

      <ConfirmDialog
        open={confirmingReplace}
        onOpenChange={setConfirmingReplace}
        title={`Replace ${count(replacing, "existing record", "existing records")}?`}
        description="Their details will be overwritten with the spreadsheet's values. Their photos and documents are kept."
        confirmLabel="Replace records"
        onConfirm={() => applyChoices(conflicts)}
      />
    </>
  );
}

/** Moves focus (and the dialog's scroll position) to the results once the
 * form they replace is gone, so keyboard and screen-reader users land on
 * them. */
function focusOnMount(element: HTMLElement | null) {
  element?.focus({ preventScroll: false });
}

function Tally({ value, label, warn }: { value: number; label: string; warn?: boolean }) {
  return (
    <div className={warn ? "hd-tally__item hd-tally__item--warn" : "hd-tally__item"}>
      <span className="hd-tally__value hd-num">{value}</span>
      <span className="hd-tally__label">{label}</span>
    </div>
  );
}
