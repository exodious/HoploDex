import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
import { withIdlePaused } from "../session/useIdleActivity";
import {
  Button,
  Checkbox,
  ConfirmDialog,
  Dialog,
  PassphraseField,
  ProgressBar,
  Select,
  TextField,
  useToast,
} from "../../components";
import type { PassphraseFieldHandle } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import * as databasesService from "./databasesService";
import { folderOf, joinPath } from "./paths";
import { DatabaseGuideLink } from "./DatabaseGuide";
import { ExistingBackupsDialog } from "./ExistingBackupsDialog";
import { RememberPassphraseConfirm } from "./RememberPassphraseConfirm";
import { minutesLabel } from "./settings";
import type {
  BackupLocationInput,
  BackupSettingsInput,
  BackupSettingsSaved,
  CollectionSettings,
  CountProgress,
  DatabaseStatus,
  ExistingBackupsChoice,
  ExistingBackupsOutcome,
  OldLocationBackups,
} from "./types";
import "../firearms/forms.css";
import "./databases.css";

/** The idle durations offered, in minutes (FR-034). */
const IDLE_MINUTES = [1, 2, 5, 10, 15, 30, 60, 120, 240];

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
 * are (FR-029), restoring from one, and deleting them all, and when it
 * locks (FR-034, FR-038), which apply with Save, like every other form;
 * and whether this computer remembers its passphrase (FR-017), which
 * changes at once. */
export function DatabaseSettingsDialog({
  open,
  onOpenChange,
  status,
  onSaved,
  onRestore,
  onPassphraseSavedChange,
}: DatabaseSettingsDialogProps) {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Database settings"
      description={
        <>
          The database file is <span className="hd-privacy-note__path">{status.path}</span>
        </>
      }
      bare
    >
      {/* Mounted only while open, so every opening starts from what is saved. */}
      <SettingsForm
        status={status}
        onSaved={(settings, keepOpen) => {
          onSaved(settings);
          if (!keepOpen) onOpenChange(false);
        }}
        onCancel={() => onOpenChange(false)}
        onRestore={onRestore}
        onPassphraseSavedChange={onPassphraseSavedChange}
      />
    </Dialog>
  );
}

type Errors = Partial<Record<"keepCount" | "location", string>>;

/** Why a move left backups at the old location (FR-026). */
type LeftBehind = NonNullable<Extract<ExistingBackupsOutcome, { action: "move" }>["leftBehind"]>;

/** The warning shown when a move left backups behind
 * (contracts/ui-databases.md §7 "Afterwards"). */
function leftBehindText(moved: number, left: LeftBehind, newFolder: string): string {
  const because = {
    nameTaken: `because a backup with the same name is already in ${newFolder}`,
    locationUnavailable: `because ${newFolder} became unavailable`,
    insufficientSpace: "because there wasn't enough space there",
    io: "because of an error",
  }[left.reason];
  const one = left.count === 1;
  return `${moved} ${moved === 1 ? "backup was" : "backups were"} moved. ${left.count} ${one ? "is" : "are"} still in ${left.folder} ${because}. HoploDex no longer manages ${one ? "it" : "them"} there.`;
}

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
  /** `keepOpen` when the result stays in the dialog. */
  onSaved: (settings: CollectionSettings, keepOpen?: boolean) => void;
  onCancel: () => void;
  onRestore: () => void;
  onPassphraseSavedChange: () => void;
}) {
  const saved = status.settings.backups;
  const defaultFolder = joinPath(folderOf(status.path), "HoploDex backups");
  const savedLocation = (): ShownLocation => ({
    input:
      saved.location.kind === "default"
        ? { kind: "default" }
        : { kind: "custom", path: saved.location.path },
    path: saved.location.path,
    available: saved.location.available,
  });
  const toast = useToast();
  const [enabled, setEnabled] = useState(saved.enabled);
  const [keepCount, setKeepCount] = useState(String(saved.keepCount));
  const [location, setLocation] = useState<ShownLocation>(savedLocation);
  const [lock, setLock] = useState(status.settings.lock);
  const [errors, setErrors] = useState<Errors>({});
  const [serverError, setServerError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  // A changed location with backups at the old one (FR-026, US3-4a), or an
  // old one that can't be reached (US3-4b): nothing is saved until answered.
  const [question, setQuestion] = useState<OldLocationBackups | null>(null);
  const [unreachable, setUnreachable] = useState<string | null>(null);
  const continuing = useRef(false);
  const [leftBehind, setLeftBehind] = useState<string | null>(null);

  async function chooseFolder() {
    const chosen = await withIdlePaused(() =>
      openFolderDialog({
        directory: true,
        multiple: false,
        title: "Choose where to keep this database's backups",
        defaultPath: location.available ? location.path : undefined,
      }),
    );
    if (typeof chosen !== "string") return;
    setLocation({ input: { kind: "custom", path: chosen }, path: chosen, available: true });
    setErrors((current) => ({ ...current, location: undefined }));
  }

  function useDefault() {
    setLocation({ input: { kind: "default" }, path: defaultFolder, available: true });
    setErrors((current) => ({ ...current, location: undefined }));
  }

  function backupInput(existingBackups?: ExistingBackupsChoice): BackupSettingsInput {
    return {
      enabled,
      keepCount: Number(keepCount.trim()),
      location: location.input,
      ...(existingBackups ? { existingBackups } : {}),
    };
  }

  function showSaveError(e: unknown) {
    if (e instanceof CommandFailure && e.fieldErrors) setErrors(e.fieldErrors);
    else
      setServerError(e instanceof CommandFailure ? e.message : "The settings couldn't be saved.");
  }

  /** The backup settings are through: the lock settings follow, then what
   * happened to the old location's backups is said (contracts/ui-databases.md
   * §7 "Afterwards"). */
  async function finish(backupsSaved: BackupSettingsSaved, oldFolder?: string) {
    const settings = await databasesService.updateLockSettings(lock);
    const outcome = backupsSaved.existingBackups;
    if (outcome?.action === "move" && outcome.leftBehind) {
      // The location is saved, so the result stays here.
      setLeftBehind(leftBehindText(outcome.movedCount, outcome.leftBehind, location.path));
      onSaved(settings, true);
      return;
    }
    if (outcome?.action === "move") toast(`The backups were moved to ${location.path}.`);
    else if (outcome?.action === "delete" && oldFolder)
      toast(`The backups at ${oldFolder} were deleted.`);
    onSaved(settings);
  }

  /** Saves, backup settings first: the lock settings wait until those are
   * through, which may take a question and a move first. */
  async function save(existingBackups?: ExistingBackupsChoice) {
    setServerError(null);
    setLeftBehind(null);
    setSaving(true);
    try {
      await finish(await databasesService.updateBackupSettings(backupInput(existingBackups)));
    } catch (e) {
      const details = e instanceof CommandFailure ? (e.details ?? {}) : {};
      if (e instanceof CommandFailure && e.code === "BACKUPS_AT_OLD_LOCATION")
        setQuestion(details as unknown as OldLocationBackups);
      else if (e instanceof CommandFailure && e.code === "OLD_BACKUP_LOCATION_UNAVAILABLE")
        setUnreachable(String(details.folder));
      else showSaveError(e);
    } finally {
      setSaving(false);
    }
  }

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    const count = Number(keepCount.trim());
    if (!Number.isInteger(count) || keepCount.trim() === "") {
      setErrors({ keepCount: "Enter a whole number of backups." });
      return;
    }
    setErrors({});
    void save();
  }

  /** Nothing is saved: the location goes back to the saved one, and the
   * other fields keep their input. */
  function keepSavedLocation() {
    setQuestion(null);
    setLocation(savedLocation());
  }

  const isDefault = location.input.kind === "default";

  return (
    <>
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
              hint="When a changed database locks or HoploDex quits, at most once a day."
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
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={saving || isDefault}
                  onClick={useDefault}
                >
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
              <li>
                Old backups are deleted securely, as far as this computer allows (see{" "}
                <DatabaseGuideLink />
                ).
              </li>
            </ul>
            <BackupActions name={status.name} disabled={saving} onRestore={onRestore} />
          </fieldset>
          <LockingSection status={status} lock={lock} disabled={saving} onChange={setLock} />
          <ThisComputer status={status} disabled={saving} onChange={onPassphraseSavedChange} />
          {leftBehind && (
            <p className="hd-banner" role="status">
              {leftBehind}
            </p>
          )}
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
      {/* Outside the form: React sends a portal's events up through its
        parents, and the question's own submit mustn't save this form. */}
      <ExistingBackupsDialog
        question={question}
        newFolder={location.path}
        name={status.name}
        send={(choice) => databasesService.updateBackupSettings(backupInput(choice))}
        onCancel={keepSavedLocation}
        onDone={(backupsSaved) => {
          const oldFolder = question?.folder;
          setQuestion(null);
          setSaving(true);
          finish(backupsSaved, oldFolder)
            .catch(showSaveError)
            .finally(() => setSaving(false));
        }}
      />
      <ConfirmDialog
        open={unreachable !== null}
        onOpenChange={(open) => {
          if (open) return;
          setUnreachable(null);
          if (!continuing.current) keepSavedLocation();
          continuing.current = false;
        }}
        title="The old backup location isn't available"
        description={`${unreachable ?? ""} can't be reached from this computer, so any backups there can't be moved or deleted from here. If you continue, they stay there, and HoploDex no longer manages them.`}
        confirmLabel="Continue"
        destructive={false}
        onConfirm={() => {
          continuing.current = true;
          return save("leave");
        }}
      />
    </>
  );
}

/** When the database locks (FR-034, FR-036, FR-038;
 * contracts/ui-databases.md §7 "Locking"). */
function LockingSection({
  status,
  lock,
  disabled,
  onChange,
}: {
  status: DatabaseStatus;
  lock: CollectionSettings["lock"];
  disabled: boolean;
  onChange: (lock: CollectionSettings["lock"]) => void;
}) {
  // A duration set some other way is still shown as it is.
  const offered = IDLE_MINUTES.includes(lock.idleMinutes)
    ? IDLE_MINUTES
    : [...IDLE_MINUTES, lock.idleMinutes].sort((a, b) => a - b);
  return (
    <fieldset className="hd-form-section hd-form-fieldset">
      <legend className="hd-form-subgroup__title">Locking</legend>
      <Checkbox
        label="Lock after a period without use"
        checked={lock.idleEnabled}
        disabled={disabled}
        onCheckedChange={(idleEnabled) => onChange({ ...lock, idleEnabled })}
        hint="Also locks when the computer goes to sleep. Turning this off stops both."
      />
      <Select
        label="After"
        value={String(lock.idleMinutes)}
        disabled={disabled || !lock.idleEnabled}
        onValueChange={(value) => onChange({ ...lock, idleMinutes: Number(value) })}
        options={offered.map((minutes) => ({
          value: String(minutes),
          label: minutesLabel(minutes),
        }))}
        fieldClassName="hd-field--third"
      />
      <Checkbox
        label="Lock when the computer's screen locks"
        checked={status.screenLockSupported && lock.onScreenLock}
        disabled={disabled || !status.screenLockSupported}
        onCheckedChange={(onScreenLock) => onChange({ ...lock, onScreenLock })}
        hint={
          status.screenLockSupported
            ? undefined
            : "Not available: this computer doesn't tell applications when the screen locks."
        }
      />
      <ul className="hd-settings-statements">
        <li>
          Locking closes the database: its data is cleared from memory, opened document copies are
          deleted, a backup is made if one is due, and it&apos;s released for other computers.
        </li>
        {status.passphraseSaved && (
          <li>
            Because the passphrase is saved on this computer, anyone using this computer account can
            reopen the database after it locks.
          </li>
        )}
      </ul>
    </fieldset>
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
        title={`Delete all ${count ?? 0} backups of “${name}”?`}
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
        setError(e.fieldErrors?.passphrase ?? "That isn't this database's passphrase.");
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
          The passphrase is remembered on this computer: the database opens without asking for it.
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
          Not remembered: the database asks for its passphrase each time it opens.
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
        onClose={() => {
          setAsking(false);
          setError(undefined);
        }}
        onConfirm={remember}
      >
        <PassphraseField
          ref={field}
          label="Passphrase"
          autoComplete="current-password"
          autoFocus
          error={error}
          onInput={() => setError(undefined)}
        />
      </RememberPassphraseConfirm>
    </fieldset>
  );
}
