import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
import {
  Button,
  Checkbox,
  ConfirmDialog,
  Dialog,
  PassphraseField,
  ProgressBar,
  TextField,
} from "../../components";
import type { PassphraseFieldHandle } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import * as databasesService from "./databasesService";
import { folderOf, joinPath } from "./paths";
import { RememberPassphraseConfirm } from "./RememberPassphraseConfirm";
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
  /** The passphrase was saved on this computer, or forgotten. */
  onPassphraseSavedChange: () => void;
}

/** The open database's settings (contracts/ui-databases.md §7): its
 * backups, where they go and how many are kept (FR-024, FR-026), what they
 * are (FR-029), restoring from one, and deleting them all, which apply with
 * Save, like every other form; and whether this computer remembers its
 * passphrase (FR-017), which changes at once. */
export function DatabaseSettingsDialog({
  open,
  onOpenChange,
  status,
  onSaved,
  onRestore,
  onPassphraseSavedChange,
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
        onPassphraseSavedChange={onPassphraseSavedChange}
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
  onPassphraseSavedChange,
}: {
  status: DatabaseStatus;
  onSaved: (settings: CollectionSettings) => void;
  onCancel: () => void;
  onRestore: () => void;
  onPassphraseSavedChange: () => void;
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
        <ThisComputer status={status} disabled={saving} onChange={onPassphraseSavedChange} />
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

/** Said wherever passphrases can't be saved (FR-019). */
const KEYRING_UNAVAILABLE = "Not available: this computer has no keyring service.";

/** Whether this computer remembers the passphrase, and changing that
 * (FR-017, FR-018, FR-019; contracts/ui-databases.md §7). Remembering asks
 * for the passphrase with the FR-017 confirmation; the backend checks it
 * opens the database before saving it. */
function ThisComputer({
  status,
  disabled,
  onChange,
}: {
  status: DatabaseStatus;
  disabled: boolean;
  onChange: () => void;
}) {
  const field = useRef<PassphraseFieldHandle>(null);
  const [asking, setAsking] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [forgetting, setForgetting] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);

  async function remember() {
    const passphrase = field.current?.read() ?? "";
    field.current?.reset();
    if (passphrase === "") {
      setError("Enter the passphrase to remember it.");
      // Keeps the confirmation open.
      throw new Error("no passphrase");
    }
    try {
      await databasesService.savePassphrase(passphrase);
    } catch (e) {
      if (e instanceof CommandFailure && e.code === "PASSPHRASE_INCORRECT")
        setError(e.fieldErrors?.passphrase ?? `That isn't the passphrase of ${status.name}.`);
      else if (e instanceof CommandFailure && e.code === "KEYRING_UNAVAILABLE")
        setError(KEYRING_UNAVAILABLE);
      else setError("The passphrase couldn't be saved.");
      throw e;
    }
    onChange();
  }

  async function forget() {
    setProblem(null);
    setForgetting(true);
    try {
      await databasesService.forgetSavedPassphrase();
      onChange();
    } catch (e) {
      setProblem(
        e instanceof CommandFailure && e.code === "KEYRING_UNAVAILABLE"
          ? "The saved passphrase couldn't be forgotten: this computer's keyring isn't available."
          : "The saved passphrase couldn't be forgotten.",
      );
    } finally {
      setForgetting(false);
    }
  }

  let body;
  if (status.passphraseSaved) {
    body = (
      <>
        <p className="hd-settings-state">
          The passphrase is remembered on this computer: {status.name} opens without asking for it.
        </p>
        <div className="hd-settings-actions__buttons">
          <Button size="sm" pending={forgetting} disabled={disabled} onClick={() => void forget()}>
            Forget saved passphrase
          </Button>
        </div>
      </>
    );
  } else if (!status.keyringAvailable) {
    body = <p className="hd-field__hint">{KEYRING_UNAVAILABLE}</p>;
  } else {
    body = (
      <>
        <p className="hd-settings-state">
          Not remembered: {status.name} asks for its passphrase each time it opens.
        </p>
        <div className="hd-settings-actions__buttons">
          <Button
            size="sm"
            disabled={disabled}
            onClick={() => {
              setError(undefined);
              setAsking(true);
            }}
          >
            Remember the passphrase on this computer…
          </Button>
        </div>
      </>
    );
  }

  return (
    <fieldset className="hd-form-section hd-form-fieldset">
      <legend className="hd-form-subgroup__title">This computer</legend>
      {body}
      {problem && (
        <p className="hd-field__error" role="alert">
          {problem}
        </p>
      )}
      <RememberPassphraseConfirm
        open={asking}
        name={status.name}
        onClose={() => {
          setAsking(false);
          setError(undefined);
        }}
        onConfirm={remember}
      >
        <PassphraseField
          ref={field}
          label={`Passphrase for ${status.name}`}
          autoComplete="current-password"
          autoFocus
          error={error}
          onInput={() => setError(undefined)}
        />
      </RememberPassphraseConfirm>
    </fieldset>
  );
}
