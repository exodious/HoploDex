import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
import { Button, Checkbox, ConfirmDialog, Dialog, ProgressBar, TextField } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import * as databasesService from "./databasesService";
import { folderOf, joinPath } from "./paths";
import type {
  BackupLocationInput,
  CollectionSettings,
  CountProgress,
  DatabaseStatus,
} from "./types";
import "../firearms/forms.css";
import "./databases.css";

/** What backups are and aren't (FR-029), said wherever they are set up. */
const BACKUP_STATEMENTS = [
  "Each backup is a complete copy of the collection.",
  "A backup opens only with the passphrase you had when it was made.",
  "Firearms you delete stay in earlier backups until those backups are removed.",
  "Backups on the same disk as the database don't protect against losing that disk.",
];

export interface DatabaseSettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  status: DatabaseStatus;
  /** The settings were saved. */
  onSaved: (settings: CollectionSettings) => void;
  /** "Restore from a backup…" was chosen. */
  onRestore: () => void;
}

/** The open database's settings (contracts/ui-databases.md §7): its
 * backups, where they go and how many are kept (FR-024, FR-026), what they
 * are (FR-029), restoring from one, and deleting them all. They apply with
 * Save, like every other form. */
export function DatabaseSettingsDialog({
  open,
  onOpenChange,
  status,
  onSaved,
  onRestore,
}: DatabaseSettingsDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange} title={`${status.name} settings`} bare>
      {/* Mounted only while open, so every opening starts from what is saved. */}
      <SettingsForm
        status={status}
        onSaved={(settings) => {
          onSaved(settings);
          onOpenChange(false);
        }}
        onCancel={() => onOpenChange(false)}
        onRestore={onRestore}
      />
    </Dialog>
  );
}

type Errors = Partial<Record<"keepCount" | "location", string>>;

/** Where the location shown came from: what is saved, or a change not yet
 * saved. */
interface ShownLocation {
  input: BackupLocationInput;
  path: string;
  available: boolean;
}

function SettingsForm({
  status,
  onSaved,
  onCancel,
  onRestore,
}: {
  status: DatabaseStatus;
  onSaved: (settings: CollectionSettings) => void;
  onCancel: () => void;
  onRestore: () => void;
}) {
  const saved = status.settings.backups;
  const defaultFolder = joinPath(folderOf(status.path), "HoploDex backups");
  const [enabled, setEnabled] = useState(saved.enabled);
  const [keepCount, setKeepCount] = useState(String(saved.keepCount));
  const [location, setLocation] = useState<ShownLocation>({
    input:
      saved.location.kind === "default"
        ? { kind: "default" }
        : { kind: "custom", path: saved.location.path },
    path: saved.location.path,
    available: saved.location.available,
  });
  const [errors, setErrors] = useState<Errors>({});
  const [serverError, setServerError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  async function chooseFolder() {
    const chosen = await openFolderDialog({
      directory: true,
      multiple: false,
      title: `Choose where to keep backups of ${status.name}`,
      defaultPath: location.available ? location.path : undefined,
    });
    if (typeof chosen !== "string") return;
    setLocation({ input: { kind: "custom", path: chosen }, path: chosen, available: true });
    setErrors((current) => ({ ...current, location: undefined }));
  }

  function useDefault() {
    setLocation({ input: { kind: "default" }, path: defaultFolder, available: true });
    setErrors((current) => ({ ...current, location: undefined }));
  }

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    const count = Number(keepCount.trim());
    if (!Number.isInteger(count) || keepCount.trim() === "") {
      setErrors({ keepCount: "Enter a whole number of backups." });
      return;
    }
    setErrors({});
    setServerError(null);
    setSaving(true);
    try {
      onSaved(
        await databasesService.updateBackupSettings({
          enabled,
          keepCount: count,
          location: location.input,
        }),
      );
    } catch (e) {
      if (e instanceof CommandFailure && e.fieldErrors) setErrors(e.fieldErrors);
      else
        setServerError(e instanceof CommandFailure ? e.message : "The settings couldn't be saved.");
    } finally {
      setSaving(false);
    }
  }

  const isDefault = location.input.kind === "default";

  return (
    <form className="hd-dialog__form" onSubmit={handleSubmit} noValidate>
      <div className="hd-dialog__body hd-form-section">
        {serverError && (
          <p className="hd-banner hd-banner--error" role="alert">
            {serverError}
          </p>
        )}
        <fieldset className="hd-form-section hd-form-fieldset">
          <legend className="hd-form-subgroup__title">Backups</legend>
          <Checkbox
            label="Make automatic backups"
            checked={enabled}
            disabled={saving}
            onCheckedChange={setEnabled}
            hint="When you close a changed database, at most once a day."
          />
          <TextField
            label="Keep the latest"
            inputMode="numeric"
            value={keepCount}
            disabled={saving}
            onChange={(e) => {
              setKeepCount(e.target.value);
              setErrors((current) => ({ ...current, keepCount: undefined }));
            }}
            error={errors.keepCount}
            fieldClassName="hd-field--third"
            trailing={<span className="hd-input__suffix">backups</span>}
          />
          <div className="hd-field">
            <span className="hd-field__label" id="backup-location-label">
              Location
            </span>
            <p className="hd-settings-location" aria-labelledby="backup-location-label">
              <span className="hd-privacy-note__path">{location.path}</span>{" "}
              {isDefault && (
                <span className="hd-settings-location__note">(next to the database)</span>
              )}
              {!location.available && (
                <span className="hd-settings-location__warning">
                  Not available on this computer
                </span>
              )}
            </p>
            <div className="hd-settings-location__actions">
              <Button size="sm" disabled={saving} onClick={() => void chooseFolder()}>
                Change…
              </Button>
              <Button size="sm" variant="ghost" disabled={saving || isDefault} onClick={useDefault}>
                Use the default
              </Button>
            </div>
            {errors.location && (
              <p className="hd-field__error" role="alert">
                {errors.location}
              </p>
            )}
          </div>
          <ul className="hd-settings-statements">
            {BACKUP_STATEMENTS.map((statement) => (
              <li key={statement}>{statement}</li>
            ))}
          </ul>
          <BackupActions name={status.name} disabled={saving} onRestore={onRestore} />
        </fieldset>
      </div>
      <footer className="hd-dialog__footer">
        <Button variant="secondary" onClick={onCancel} disabled={saving}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" pending={saving}>
          Save
        </Button>
      </footer>
    </form>
  );
}

/** Restoring from a backup, and deleting them all (FR-028, FR-029). */
function BackupActions({
  name,
  disabled,
  onRestore,
}: {
  name: string;
  disabled: boolean;
  onRestore: () => void;
}) {
  const [count, setCount] = useState<number | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [progress, setProgress] = useState<CountProgress | null>(null);
  const [result, setResult] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    databasesService.listBackups().then(
      (listed) => current && setCount(listed.backups.length),
      () => current && setCount(0),
    );
    return () => {
      current = false;
    };
  }, []);

  useEffect(
    () => (confirming ? databasesService.onBackupsDeleteProgress(setProgress) : undefined),
    [confirming],
  );

  async function deleteAll() {
    try {
      const deleted = await databasesService.deleteAllBackups(true);
      setCount(deleted.failedPaths.length);
      setResult(
        deleted.failedPaths.length === 0
          ? `${deleted.deletedCount} ${deleted.deletedCount === 1 ? "backup was" : "backups were"} deleted.`
          : `${deleted.deletedCount} deleted. These couldn't be deleted: ${deleted.failedPaths.join(", ")}`,
      );
    } catch (e) {
      setResult(e instanceof CommandFailure ? e.message : "The backups couldn't be deleted.");
    } finally {
      setProgress(null);
    }
  }

  return (
    <div className="hd-settings-actions">
      <div className="hd-settings-actions__buttons">
        <Button size="sm" disabled={disabled} onClick={onRestore}>
          Restore from a backup…
        </Button>
        <Button
          size="sm"
          variant="danger"
          disabled={disabled || !count}
          onClick={() => {
            setResult(null);
            setConfirming(true);
          }}
        >
          Delete all backups…
        </Button>
      </div>
      {count === 0 && !result && <p className="hd-field__hint">There are no backups yet.</p>}
      {result && (
        <p className="hd-field__hint" role="status">
          {result}
        </p>
      )}
      <ConfirmDialog
        open={confirming}
        onOpenChange={setConfirming}
        title={`Delete all ${count ?? 0} backups of ${name}?`}
        description="They are deleted securely where this computer supports it. This can't be undone."
        confirmLabel="Delete all backups"
        onConfirm={deleteAll}
      >
        {progress && <ProgressBar value={progress} label="Deleting backups" unit="backups" />}
      </ConfirmDialog>
    </div>
  );
}
