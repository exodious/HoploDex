import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
import { withIdlePaused } from "../session/useIdleActivity";
import {
  Button,
  Checkbox,
  ConfirmDialog,
  Dialog,
  NumberField,
  PassphraseField,
  ProgressBar,
  SegmentedControl,
  Select,
  useToast,
} from "../../components";
import type { PassphraseFieldHandle } from "../../components";
import { firstRefusedField, focusFormField } from "../../lib/formFocus";
import { CommandFailure } from "../../services/tauriClient";
import { useSessionScope } from "../session/sessionScope";
import { chooseDocumentOpening, useDocumentOpening } from "../media/documentOpening";
import type { DocumentOpening } from "../media/types";
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

/** The idle durations offered, in minutes (003 FR-034). */
const IDLE_MINUTES = [1, 2, 5, 10, 15, 30, 60, 120, 240];

/** How many backups may be kept: 1 to the backend's `MAX_KEEP_COUNT`
 * (src-tauri/src/models/database.rs), which stays the authority. */
const MIN_KEEP_COUNT = 1;
const MAX_KEEP_COUNT = 100;

/** What backups are and aren't (003 FR-029), said wherever they are set up. */
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

/** The open database's settings (003 contracts/ui-databases.md §7): its
 * backups, where they go and how many are kept (003 FR-024, FR-026), what they
 * are (003 FR-029), restoring from one, and deleting them all, and when it
 * locks (003 FR-034, FR-038), and whether this computer remembers its
 * passphrase (003 FR-017), which all apply with Save, like every other form,
 * and which Cancel discards. Save waits until something differs from what
 * is saved (#34). */
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

/** The order the fields are read in, for the first one refused to take the focus. */
const FIELD_ORDER = ["keepCount", "location"] as const satisfies readonly (keyof Errors)[];

/** Why a move left backups at the old location (003 FR-026). */
type LeftBehind = NonNullable<Extract<ExistingBackupsOutcome, { action: "move" }>["leftBehind"]>;

/** The warning shown when a move left backups behind
 * (003 contracts/ui-databases.md §7 "Afterwards"). */
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

/** What this computer is to do with the passphrase on Save (#34): remember it (asking for it
 * then), forget the saved one, or nothing. */
type PassphraseChoice = "remember" | "forget" | null;

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
  const scope = useSessionScope();
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
  // This computer's keyring is staged like the rest: nothing touches it until Save. The saved
  // state is kept here, so it reads right as soon as Save has changed it.
  const [passphraseSaved, setPassphraseSaved] = useState(status.passphraseSaved);
  const [passphraseChoice, setPassphraseChoice] = useState<PassphraseChoice>(null);
  const [asking, setAsking] = useState(false);
  const [askError, setAskError] = useState<string | undefined>();
  const passphraseField = useRef<PassphraseFieldHandle>(null);
  const [note, setNote] = useState<string | null>(null);
  const [errors, setErrors] = useState<Errors>({});
  const [serverError, setServerError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const formRef = useRef<HTMLFormElement>(null);
  // The field that was refused, to focus once Save is through and the form is
  // enabled again (a disabled field can't take focus).
  const [refused, setRefused] = useState<keyof Errors | null>(null);

  useEffect(() => {
    if (saving || !refused) return;
    focusFormField(formRef.current, refused);
    setRefused(null);
  }, [saving, refused]);
  // A changed location with backups at the old one (003 FR-026, US3-4a), or an
  // old one that can't be reached (003 US3-4b): nothing is saved until answered.
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
    if (e instanceof CommandFailure && e.fieldErrors) {
      setErrors(e.fieldErrors);
      // As every form does (the constitution's UI consistency): the first
      // field the backend refused takes the focus.
      setRefused(firstRefusedField(FIELD_ORDER, e.fieldErrors) ?? null);
    } else
      setServerError(e instanceof CommandFailure ? e.message : "The settings couldn't be saved.");
  }

  /** The backup settings are through: the lock settings follow, then what
   * happened to the old location's backups is said (003 contracts/ui-databases.md
   * §7 "Afterwards"). */
  async function finish(backupsSaved: BackupSettingsSaved, oldFolder?: string) {
    const settings = await databasesService.updateLockSettings(scope, lock);
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

  /** Saves the backup and lock settings, backup settings first: the lock settings wait until those
   * are through, which may take a question and a move first. Nothing is sent when they are what
   * is saved, as when only the passphrase was staged. */
  async function save(existingBackups?: ExistingBackupsChoice) {
    setServerError(null);
    setLeftBehind(null);
    setSaving(true);
    try {
      if (!settingsDirty) {
        onSaved(status.settings);
        return;
      }
      await finish(
        await databasesService.updateBackupSettings(scope, backupInput(existingBackups)),
      );
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

  /** The passphrase half of Save is through (#34). Whatever the settings do next, this is done,
   * so it is said and shown as it now is. */
  function passphraseDone(remembered: boolean) {
    setPassphraseSaved(remembered);
    setPassphraseChoice(null);
    setNote(
      remembered
        ? "The passphrase is now remembered on this computer."
        : "The saved passphrase was forgotten.",
    );
    onPassphraseSavedChange();
  }

  /** Forgets the saved passphrase, then saves the rest. When the keyring refuses, nothing else
   * is saved either, and the choice stays staged. */
  async function forgetThenSave() {
    setServerError(null);
    setNote(null);
    setSaving(true);
    try {
      await databasesService.forgetSavedPassphrase(scope);
    } catch (e) {
      setServerError(
        e instanceof CommandFailure && e.code === "KEYRING_UNAVAILABLE"
          ? "The saved passphrase couldn't be forgotten: this computer's keyring isn't available. Nothing was saved."
          : "The saved passphrase couldn't be forgotten. Nothing was saved.",
      );
      setSaving(false);
      return;
    }
    passphraseDone(false);
    await save();
  }

  /** The passphrase prompt's confirmation (#34): the passphrase is read from the field and
   * emptied at once, checked and saved by the backend, and goes out of reach here with this call.
   * A refusal keeps the prompt open with the error; success closes it and saves the rest. */
  async function rememberThenSave() {
    const passphrase = passphraseField.current?.read() ?? "";
    passphraseField.current?.reset();
    if (passphrase === "") {
      setAskError("Enter the passphrase to remember it.");
      // Keeps the prompt open.
      throw new Error("no passphrase");
    }
    try {
      await databasesService.savePassphrase(scope, passphrase);
    } catch (e) {
      if (e instanceof CommandFailure && e.code === "PASSPHRASE_INCORRECT")
        setAskError(e.fieldErrors?.passphrase ?? "That isn't this database's passphrase.");
      else if (e instanceof CommandFailure && e.code === "KEYRING_UNAVAILABLE")
        setAskError(KEYRING_UNAVAILABLE);
      else setAskError("The passphrase couldn't be saved.");
      throw e;
    }
    setServerError(null);
    passphraseDone(true);
    void save();
  }

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    if (!dirty) return;
    const count = Number(keepCount.trim());
    if (!Number.isInteger(count) || keepCount.trim() === "") {
      setErrors({ keepCount: "Enter a whole number of backups." });
      setRefused("keepCount");
      return;
    }
    setErrors({});
    // Remembering asks for the passphrase now, not while it was being ticked, so it is held no
    // longer than the save needs. Cancelling that prompt saves nothing.
    if (passphraseChoice === "remember") {
      setAskError(undefined);
      setAsking(true);
    } else if (passphraseChoice === "forget") void forgetThenSave();
    else void save();
  }

  /** Nothing is saved: the location goes back to the saved one, and the
   * other fields keep their input. */
  function keepSavedLocation() {
    setQuestion(null);
    setLocation(savedLocation());
  }

  const isDefault = location.input.kind === "default";
  const savedInput = savedLocation().input;
  const savedLock = status.settings.lock;
  // Whether anything differs from what is saved (#34): Save waits for it.
  const settingsDirty =
    enabled !== saved.enabled ||
    keepCount.trim() !== String(saved.keepCount) ||
    location.input.kind !== savedInput.kind ||
    (location.input.kind === "custom" &&
      savedInput.kind === "custom" &&
      location.input.path !== savedInput.path) ||
    lock.idleEnabled !== savedLock.idleEnabled ||
    lock.idleMinutes !== savedLock.idleMinutes ||
    lock.onScreenLock !== savedLock.onScreenLock;
  const dirty = settingsDirty || passphraseChoice !== null;
  const backupsOff = !enabled;

  return (
    <>
      <form ref={formRef} className="hd-dialog__form" onSubmit={handleSubmit} noValidate>
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
            <div data-field="keepCount">
              <NumberField
                label="Keep the latest"
                value={keepCount}
                min={MIN_KEEP_COUNT}
                max={MAX_KEEP_COUNT}
                disabled={saving || backupsOff}
                onValueChange={(text) => {
                  setKeepCount(text);
                  setErrors((current) => ({ ...current, keepCount: undefined }));
                }}
                error={errors.keepCount}
                fieldClassName="hd-field--third"
                suffix="backups"
              />
            </div>
            <div className="hd-field" data-field="location" data-disabled={backupsOff || undefined}>
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
                <Button
                  size="sm"
                  disabled={saving || backupsOff}
                  onClick={() => void chooseFolder()}
                >
                  Change…
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={saving || backupsOff || isDefault}
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
          <DocumentsSection />
          <ThisComputer
            status={status}
            passphraseSaved={passphraseSaved}
            choice={passphraseChoice}
            disabled={saving}
            onChoose={setPassphraseChoice}
          />
          {note && (
            <p className="hd-banner" role="status">
              {note}
            </p>
          )}
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
          <Button type="submit" variant="primary" pending={saving} disabled={!dirty}>
            Save
          </Button>
        </footer>
      </form>
      {/* Outside the form: React sends a portal's events up through its
        parents, and the question's own submit mustn't save this form. */}
      <RememberPassphraseConfirm
        open={asking}
        onClose={() => {
          setAsking(false);
          setAskError(undefined);
        }}
        onConfirm={rememberThenSave}
      >
        <PassphraseField
          ref={passphraseField}
          label="Passphrase"
          autoComplete="current-password"
          autoFocus
          error={askError}
          onInput={() => setAskError(undefined)}
        />
      </RememberPassphraseConfirm>
      <ExistingBackupsDialog
        question={question}
        newFolder={location.path}
        name={status.name}
        send={(choice) => databasesService.updateBackupSettings(scope, backupInput(choice))}
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

/** What a document's name does (007 FR-012; 007 contracts/ui-document-preview.md §5). It is this
 * computer's setting, not the database's: it is saved as soon as it is chosen, through the store
 * the document lists read, and the dialog's Save and Cancel leave it alone. Choosing "Open in
 * another app" asks natively first, so the control stays at the old value, busy, until that is
 * answered, and stays there if it was declined. */
function DocumentsSection() {
  const notify = useToast();
  const opening = useDocumentOpening();
  const [pending, setPending] = useState(false);

  async function choose(value: DocumentOpening) {
    if (value === opening) return;
    setPending(true);
    try {
      await chooseDocumentOpening(value);
    } catch (e) {
      notify(e instanceof CommandFailure ? e.message : "The setting couldn't be saved.", "error");
    } finally {
      setPending(false);
    }
  }

  return (
    <fieldset className="hd-form-section hd-form-fieldset">
      <legend className="hd-form-subgroup__title">Documents</legend>
      <SegmentedControl<DocumentOpening>
        label="Open documents"
        value={opening}
        busy={pending}
        onChange={(value) => void choose(value)}
        options={[
          { value: "preview", label: "Preview in HoploDex" },
          { value: "external", label: "Open in another app" },
        ]}
      />
      <p className="hd-field__hint">
        Applies to every database on this computer. Saved for this computer as soon as you choose.
      </p>
    </fieldset>
  );
}

/** When the database locks (003 FR-034, FR-036, FR-038;
 * 003 contracts/ui-databases.md §7 "Locking"). */
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

/** Restoring from a backup, and deleting them all (003 FR-028, FR-029). */
function BackupActions({
  name,
  disabled,
  onRestore,
}: {
  name: string;
  disabled: boolean;
  onRestore: () => void;
}) {
  const scope = useSessionScope();
  const [count, setCount] = useState<number | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [progress, setProgress] = useState<CountProgress | null>(null);
  const [result, setResult] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    databasesService.listBackups(scope).then(
      (listed) => current && setCount(listed.backups.length),
      () => current && setCount(0),
    );
    return () => {
      current = false;
    };
  }, [scope]);

  useEffect(
    () => (confirming ? databasesService.onBackupsDeleteProgress(setProgress) : undefined),
    [confirming],
  );

  async function deleteAll() {
    try {
      const deleted = await databasesService.deleteAllBackups(scope, true);
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

/** Said wherever passphrases can't be saved (003 FR-019). */
const KEYRING_UNAVAILABLE = "Not available: this computer has no keyring service.";

/** Whether this computer remembers the passphrase (003 FR-017, FR-018, FR-019;
 * 003 contracts/ui-databases.md §7). Like the rest of the form it only stages the
 * choice: ticking Remember asks for nothing until Save, and Save then asks
 * for the passphrase with the 003 FR-017 confirmation; unticking forgets the
 * saved one on Save. Cancel throws the choice away (#34). */
function ThisComputer({
  status,
  passphraseSaved,
  choice,
  disabled,
  onChoose,
}: {
  status: DatabaseStatus;
  /** Whether the passphrase is saved now, which Save may change. */
  passphraseSaved: boolean;
  choice: PassphraseChoice;
  disabled: boolean;
  onChoose: (choice: PassphraseChoice) => void;
}) {
  const remembered = choice ? choice === "remember" : passphraseSaved;
  let state: string;
  if (choice === "remember")
    state = "When you click Save, you'll be asked for the passphrase to remember it.";
  else if (choice === "forget")
    state =
      "The saved passphrase will be forgotten when you click Save. The database will then ask for its passphrase each time it opens.";
  else if (passphraseSaved)
    state =
      "The passphrase is remembered on this computer: the database opens without asking for it.";
  else state = "Not remembered: the database asks for its passphrase each time it opens.";

  return (
    <fieldset className="hd-form-section hd-form-fieldset">
      <legend className="hd-form-subgroup__title">This computer</legend>
      {passphraseSaved || status.keyringAvailable ? (
        <Checkbox
          label="Remember the passphrase on this computer"
          checked={remembered}
          disabled={disabled}
          onCheckedChange={(next) =>
            onChoose(next === passphraseSaved ? null : next ? "remember" : "forget")
          }
          hint={state}
        />
      ) : (
        <p className="hd-field__hint">{KEYRING_UNAVAILABLE}</p>
      )}
    </fieldset>
  );
}
