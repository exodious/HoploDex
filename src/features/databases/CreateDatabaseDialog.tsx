import { useRef, useState } from "react";
import type { FormEvent } from "react";
import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
import { withIdlePaused } from "../session/useIdleActivity";
import {
  Button,
  Checkbox,
  Dialog,
  Icon,
  MIN_PASSPHRASE_CHARS,
  PASSPHRASE_TOO_SHORT,
  PASSPHRASES_DIFFER,
  PassphraseField,
  TextField,
  usePassphraseChecks,
} from "../../components";
import type { PassphraseFieldHandle } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import { joinPath } from "./paths";
import { DEFAULT_SETTINGS } from "./settings";
import type { CreateDatabaseInput, SuggestedLocation } from "./types";
import "../firearms/forms.css";
import "./databases.css";

const ACKNOWLEDGEMENT =
  "I have stored this passphrase somewhere safe. If it is forgotten, nobody, including HoploDex, can open this database or recover the collection.";

export interface CreateDatabaseDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  suggested: SuggestedLocation;
  /** Creates and opens the database; rejects with the command's failure. */
  onCreate: (input: CreateDatabaseInput) => Promise<void>;
}

/** Creates a database protected by a passphrase the user chooses (FR-003,
 * FR-004, FR-009), saying before it exists where it and its backups will be
 * (FR-024). */
export function CreateDatabaseDialog({
  open,
  onOpenChange,
  suggested,
  onCreate,
}: CreateDatabaseDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange} title="Create a database" size="lg" bare>
      {/* Mounted only while open, so every opening starts from the suggestion. */}
      <CreateDatabaseForm
        suggested={suggested}
        onCreate={onCreate}
        onCancel={() => onOpenChange(false)}
      />
    </Dialog>
  );
}

type Errors = Partial<
  Record<"name" | "folder" | "passphrase" | "confirmation" | "acknowledgedUnrecoverable", string>
>;

function CreateDatabaseForm({
  suggested,
  onCreate,
  onCancel,
}: {
  suggested: SuggestedLocation;
  onCreate: (input: CreateDatabaseInput) => Promise<void>;
  onCancel: () => void;
}) {
  const [name, setName] = useState(suggested.name);
  const [folder, setFolder] = useState(suggested.folder);
  const [acknowledged, setAcknowledged] = useState(false);
  const [errors, setErrors] = useState<Errors>({});
  const [serverError, setServerError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const passphrase = useRef<PassphraseFieldHandle>(null);

  function clearError(field: keyof Errors) {
    setErrors((current) => ({ ...current, [field]: undefined }));
  }
  const confirmation = useRef<PassphraseFieldHandle>(null);
  // Checked as they are typed; Create database waits until all is well.
  const checks = usePassphraseChecks({ passphrase, confirmation });

  function edited(field: "passphrase" | "confirmation") {
    clearError(field);
    checks.check();
  }

  const trimmedName = name.trim();
  const trimmedFolder = folder.trim();
  const shownName = trimmedName || suggested.name;
  const target = trimmedFolder ? joinPath(trimmedFolder, `${shownName}.hoplodex`) : null;
  const backups = trimmedFolder ? joinPath(trimmedFolder, "HoploDex backups") : null;

  async function chooseFolder() {
    const selected = await withIdlePaused(() =>
      openFolderDialog({
        directory: true,
        multiple: false,
        title: "Choose where to keep the database",
        defaultPath: trimmedFolder || undefined,
      }),
    );
    if (typeof selected === "string") {
      setFolder(selected);
      clearError("folder");
    }
  }

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    // Each passphrase is read once and the field emptied at once, whatever
    // happens next (FR-007).
    const typed = passphrase.current?.read() ?? "";
    const confirmed = confirmation.current?.read() ?? "";
    passphrase.current?.reset();
    confirmation.current?.reset();
    checks.clear();

    const found: Errors = {};
    if (!trimmedName) found.name = "Enter a name.";
    if (!trimmedFolder) found.folder = "Enter a folder.";
    if ([...typed.normalize("NFC")].length < MIN_PASSPHRASE_CHARS) {
      found.passphrase = PASSPHRASE_TOO_SHORT;
    } else if (typed !== confirmed) {
      found.confirmation = PASSPHRASES_DIFFER;
    }
    setErrors(found);
    setServerError(null);
    if (Object.keys(found).length > 0) {
      // The emptied field that needs retyping gets the focus.
      if (found.passphrase) passphrase.current?.focus();
      else if (found.confirmation) confirmation.current?.focus();
      return;
    }

    setSubmitting(true);
    try {
      await onCreate({
        folder: trimmedFolder,
        name: trimmedName,
        passphrase: typed,
        acknowledgedUnrecoverable: acknowledged,
      });
    } catch (e) {
      if (e instanceof CommandFailure && e.fieldErrors) {
        setErrors(e.fieldErrors);
      } else if (e instanceof CommandFailure && e.code === "DATABASE_EXISTS") {
        setErrors({ name: e.message });
      } else {
        setServerError(
          e instanceof CommandFailure ? e.message : "The database couldn't be created.",
        );
      }
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form className="hd-dialog__form" onSubmit={handleSubmit} noValidate>
      <div className="hd-dialog__body hd-form-section">
        {serverError && (
          <p className="hd-banner hd-banner--error" role="alert">
            {serverError}
          </p>
        )}
        <TextField
          label="Name"
          required
          value={name}
          maxLength={120}
          disabled={submitting}
          onChange={(e) => {
            setName(e.target.value);
            clearError("name");
          }}
          hint="This is also the file name."
          error={errors.name}
          fieldClassName="hd-field--half"
        />
        <TextField
          label="Folder"
          required
          value={folder}
          disabled={submitting}
          spellCheck={false}
          onChange={(e) => {
            setFolder(e.target.value);
            clearError("folder");
          }}
          hint={target ? `Saved as ${target}` : undefined}
          error={errors.folder}
          trailing={
            <button
              type="button"
              className="hd-input__text-action"
              disabled={submitting}
              onClick={() => void chooseFolder()}
            >
              Choose…
            </button>
          }
        />
        <div className="hd-form-grid hd-form-grid--2">
          <PassphraseField
            ref={passphrase}
            label="Passphrase"
            autoComplete="new-password"
            required
            strength
            disabled={submitting}
            error={errors.passphrase ?? checks.errors.passphrase}
            onInput={() => edited("passphrase")}
          />
          <PassphraseField
            ref={confirmation}
            label="Confirm passphrase"
            autoComplete="new-password"
            required
            disabled={submitting}
            error={errors.confirmation ?? checks.errors.confirmation}
            onInput={() => edited("confirmation")}
          />
        </div>
        <section className="hd-privacy-note" role="note" aria-labelledby="create-db-backups">
          <Icon name="archive" size={16} />
          <div>
            <h3 id="create-db-backups" className="hd-privacy-note__title">
              Backups
            </h3>
            <p>
              Backups are on. When you close {shownName} after changing it, HoploDex saves a copy in{" "}
              <strong className="hd-privacy-note__path">
                {backups ?? "the database's folder"}
              </strong>
              , at most once a day, keeping the latest {DEFAULT_SETTINGS.keepCount}. Each backup
              holds the whole collection and opens with the passphrase you had when it was made. You
              can change this in the database settings.
            </p>
          </div>
        </section>
        <div>
          <Checkbox
            label={ACKNOWLEDGEMENT}
            checked={acknowledged}
            disabled={submitting}
            onCheckedChange={(checked) => {
              setAcknowledged(checked);
              clearError("acknowledgedUnrecoverable");
            }}
          />
          {errors.acknowledgedUnrecoverable && (
            <p className="hd-field__error" role="alert">
              {errors.acknowledgedUnrecoverable}
            </p>
          )}
        </div>
      </div>
      <footer className="hd-dialog__footer">
        <Button variant="secondary" onClick={onCancel} disabled={submitting}>
          Cancel
        </Button>
        <Button
          type="submit"
          variant="primary"
          pending={submitting}
          disabled={!acknowledged || !checks.ready}
        >
          {submitting ? "Creating…" : "Create database"}
        </Button>
      </footer>
    </form>
  );
}
