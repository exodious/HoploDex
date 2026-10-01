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
import { MountedChoices } from "../mounts/MountedChoices";
import { mountedStatements } from "../mounts/mountedStatements";
import { recordKey } from "../mounts/recordKey";
import { accessoryNameText } from "../mounts/recordNames";
import * as importExportService from "./importExportService";
import type {
  ConflictAction,
  ImportConflict,
  ImportResult,
  ImportTable,
  RowError,
  SnappedValue,
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
  items: { key: string; place: string; text: string }[];
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
              <span className="hd-row-errors__row hd-num">{item.place}</span>
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

const TABLE_NAMES: Record<ImportTable, string> = {
  firearms: "Firearms",
  accessories: "Accessories",
};

/** "Accessories, row 4": every report entry names its table (FR-022). */
function place(entry: { table: ImportTable; row: number }): string {
  return `${TABLE_NAMES[entry.table]}, row ${entry.row}`;
}

/** A conflict's record as the report names it: an accessory by its name
 * (FR-005), a firearm by make and model. */
function conflictName(conflict: ImportConflict): string {
  if (conflict.table === "accessories") {
    return accessoryNameText(conflict.make, conflict.model, conflict.kindName ?? "Accessory");
  }
  return `${conflict.make ?? ""} ${conflict.model ?? ""}`.trim();
}

function conflictKey(entry: { table: ImportTable; row: number }): string {
  return `${entry.table}-${entry.row}`;
}

/** The conflicts heading's nouns: "row matches a firearm", and so on. */
function conflictNouns(conflicts: ImportConflict[]): [string, string] {
  const accessories = conflicts.filter((c) => c.table === "accessories").length;
  if (accessories === 0) return ["row matches a firearm", "rows match firearms"];
  if (accessories === conflicts.length)
    return ["row matches an accessory", "rows match accessories"];
  return ["row matches a record", "rows match records"];
}

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
/** Fields the report names by label rather than by column (contracts/ui-registration.md §9). */
const SNAPPED_FIELD_LABELS: Partial<Record<SnappedValue["field"], string>> = {
  registrationForm: "Form",
  registeredTo: "Registered to",
};

export function ImportDialog({ open, onOpenChange }: ImportDialogProps) {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Import from a spreadsheet"
      description="Adds firearm and accessory records from CSV or Excel files with the same columns as an export. Photos aren't imported; add them to each record afterwards."
      size="lg"
      bare
    >
      <ImportFlow onClose={() => onOpenChange(false)} />
    </Dialog>
  );
}

/** One firearm table and one accessory table (FR-022): the backend stops on
 * a third file, so the picker never offers one. */
const MAX_IMPORT_FILES = 2;
const TOO_MANY_FILES =
  "An import takes at most two files, one of firearms and one of accessories. Nothing was added.";

function ImportFlow({ onClose }: { onClose: () => void }) {
  const { refresh } = useCollection();
  const notify = useToast();
  // The files picked (one, or the firearm and accessory files together), and
  // a path typed by hand, which counts as one more.
  const [files, setFiles] = useState<string[]>([]);
  const [typedPath, setTypedPath] = useState("");
  const [fileError, setFileError] = useState<string | undefined>();
  const [importing, setImporting] = useState(false);
  const [result, setResult] = useState<ImportResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [choices, setChoices] = useState<Record<string, ConflictAction>>({});
  const [resolving, setResolving] = useState(false);
  const [confirmingReplace, setConfirmingReplace] = useState(false);
  // Issue #56: for each conflict whose row disposes of a record with records
  // mounted on it, the ones to dispose of with it, by `recordKey`. The rest
  // are kept (FR-014).
  const [disposeWith, setDisposeWith] = useState<Record<string, Record<string, string>>>({});
  const [resolvedCount, setResolvedCount] = useState<number | null>(null);
  const [unresolved, setUnresolved] = useState<RowError[]>([]);

  const chosenFiles = [...files, ...(typedPath.trim() ? [typedPath.trim()] : [])];
  const full = chosenFiles.length >= MAX_IMPORT_FILES;

  async function chooseFile() {
    const selected = await withIdlePaused(() =>
      openDialog({
        multiple: true,
        title: "Import spreadsheet",
        filters: [{ name: "Spreadsheet", extensions: ["csv", "xlsx"] }],
      }),
    );
    const picked = (Array.isArray(selected) ? selected : selected ? [selected] : []).filter(
      (path): path is string => typeof path === "string",
    );
    if (picked.length === 0) return;
    // A second pick adds to the first. A pick that would take the import
    // past its two files adds nothing and says so (ui-accessories.md §11,
    // issue #56).
    const added = picked.filter((path) => !files.includes(path));
    if (chosenFiles.length + added.length > MAX_IMPORT_FILES) {
      setFileError(TOO_MANY_FILES);
      return;
    }
    setFiles([...files, ...added]);
    setFileError(undefined);
  }

  async function handleImport(event: FormEvent) {
    event.preventDefault();
    if (chosenFiles.length === 0) {
      setFileError("Choose the spreadsheet to import.");
      return;
    }
    setError(null);
    setImporting(true);
    try {
      const imported = await importExportService.importCollection({
        files: chosenFiles.map((filePath) => ({ filePath, format: formatForPath(filePath) })),
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
          ...(choices[c.conflictId] === "overwrite" &&
            c.mounted.length > 0 && {
              withMounted: c.mounted
                .map((entry) => entry.label.record)
                .filter((record) => recordKey(record) in (disposeWith[c.conflictId] ?? {})),
            }),
        })),
      });
      setResolvedCount((so_far) => (so_far ?? 0) + resolved.resolvedCount);
      // A decision the backend couldn't apply leaves its row open to be
      // decided again; everything else is done.
      const stillOpen = new Set(resolved.unresolved.map(conflictKey));
      setResult({
        ...result,
        conflicts: result.conflicts.filter((c) => stillOpen.has(conflictKey(c))),
        // specs/002-firearm-identification FR-009: a resolved overwrite or
        // duplicate can add its own original-marks warning, and
        // specs/006-accessory-links FR-023 a mount it couldn't make.
        warnings: [...result.warnings, ...resolved.warnings],
      });
      setUnresolved(resolved.unresolved);
      setChoices({});
      setDisposeWith({});
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
            value={typedPath}
            onChange={(e) => {
              setTypedPath(e.target.value);
              setFileError(undefined);
            }}
            error={fileError}
            spellCheck={false}
            // Two chosen files leave no room for a typed one.
            disabled={files.length >= MAX_IMPORT_FILES}
            hint={
              full
                ? "Two files chosen, the most one import takes. Remove one to choose another."
                : "Choose one file, or the firearm and accessory files together. Rows that match a record you already have are held for you to decide on."
            }
            trailing={
              <button
                type="button"
                className="hd-input__text-action"
                onClick={chooseFile}
                disabled={full}
              >
                Choose…
              </button>
            }
          />
          {files.length > 0 && (
            <ul className="hd-io-files" aria-label="Files to import">
              {files.map((path) => (
                <li key={path}>
                  <span className="hd-io-files__path">{path}</span>
                  <button
                    type="button"
                    className="hd-input__text-action"
                    aria-label={`Remove ${path}`}
                    onClick={() => setFiles((current) => current.filter((p) => p !== path))}
                  >
                    Remove
                  </button>
                </li>
              ))}
            </ul>
          )}
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
  // Rows that dispose of a record with records mounted on it ask, on
  // replacing, which go with it (issue #56).
  const disposing = conflicts.filter(
    (c) => choices[c.conflictId] === "overwrite" && c.mounted.length > 0,
  );

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
            key: place(derived),
            place: place(derived),
            text: `${derived.cartridge} → ${derived.caliber} (${
              derived.source === "catalog" ? "built-in" : "guessed"
            })`,
          }))}
        />
        <ReportDisclosure
          title="Spellings matched to existing values"
          summary="These differed only in letter case, spacing or separators, so they now match a spelling already in use."
          items={result.snappedValues.map((snapped) => ({
            key: `${place(snapped)}-${snapped.field}`,
            place: place(snapped),
            text: `${SNAPPED_FIELD_LABELS[snapped.field] ?? snapped.field}: “${snapped.sheetValue}” → “${snapped.recordedValue}”`,
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
                <li key={`${place(rowError)}-${rowError.message}`}>
                  <span className="hd-row-errors__row hd-num">{place(rowError)}</span>
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
              These rows imported, with a note: a firearm shares original maker's marks with one in
              your collection, or a mount couldn’t be made and the record was left unmounted.
            </p>
            <ul className="hd-row-errors hd-row-errors--info">
              {result.warnings.map((warning) => (
                <li key={`${place(warning)}-${warning.message}`}>
                  <span className="hd-row-errors__row hd-num">{place(warning)}</span>
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
                <li key={`${place(item)}-${item.message}`}>
                  <span className="hd-row-errors__row hd-num">{place(item)}</span>
                  <span>{item.message}</span>
                </li>
              ))}
            </ul>
          </section>
        )}

        {conflicts.length > 0 && (
          <section className="hd-io-section" aria-labelledby="conflicts-title">
            <h3 className="hd-io-section__title" id="conflicts-title">
              {count(conflicts.length, ...conflictNouns(conflicts))} already in your collection
            </h3>
            <p className="hd-form-note">
              Same record ID, or for a firearm the same make, model, and serial number. Choose what
              to do with each one. A firearm can’t be added as new while an active one has the same
              make, model, and serial number.
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
                  <th scope="col">Record</th>
                  <th scope="col">Decision</th>
                </tr>
              </thead>
              <tbody>
                {conflicts.map((conflict) => (
                  <tr key={conflict.conflictId}>
                    <td className="hd-num">{place(conflict)}</td>
                    <td>
                      <span className="hd-conflicts__name">{conflictName(conflict)}</span>
                      {conflict.serialNumber && (
                        <span className="hd-serial hd-conflicts__serial">
                          {conflict.serialNumber}
                        </span>
                      )}
                    </td>
                    <td>
                      <SegmentedControl<ConflictAction>
                        label={`${place(conflict)}: ${conflictName(conflict)}`}
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
      >
        {disposing.length > 0 && (
          <div className="hd-io-disposing">
            <p className="hd-form-note">
              {disposing.length === 1
                ? "This row marks its record disposed. Records disposed with it take the row's type, recipient and date, with no price."
                : "These rows mark their records disposed. Records disposed with one take its row's type, recipient and date, with no price."}
            </p>
            {disposing.map((conflict) => {
              const name = conflictName(conflict);
              const chosen = disposeWith[conflict.conflictId] ?? {};
              return (
                <MountedChoices
                  key={conflict.conflictId}
                  title={`Mounted on ${name} (${place(conflict)})`}
                  mounted={conflict.mounted}
                  disposeWith={chosen}
                  onChange={(next) =>
                    setDisposeWith((current) => ({ ...current, [conflict.conflictId]: next }))
                  }
                  statements={mountedStatements(name, conflict.mounted, chosen)}
                />
              );
            })}
          </div>
        )}
      </ConfirmDialog>
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
