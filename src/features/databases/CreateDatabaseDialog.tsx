import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import { withIdlePaused } from "../session/useIdleActivity";
import { Button, Dialog, Icon, TextField } from "../../components";
import { firstRefusedField, focusFormField } from "../../lib/formFocus";
import { CommandFailure } from "../../services/tauriClient";
import { ChosenPathField } from "../import-export/ChosenPathField";
import * as databasesService from "./databasesService";
import { joinPath } from "./paths";
import { DEFAULT_SETTINGS } from "./settings";
import type { ChosenPath } from "../import-export/types";
import type { CreateDatabaseInput, SuggestedLocation } from "./types";
import { FlatpakReachNote } from "./FlatpakReachNote";
import "../firearms/forms.css";
import "./databases.css";

export interface CreateDatabaseDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  suggested: SuggestedLocation;
  /** This copy is a Flatpak, so the folder field says what it reaches (#260). */
  flatpak?: boolean;
  /** Creates and opens the database once its passphrase is chosen in the
   * native dialog; rejects with the command's failure. */
  onCreate: (input: CreateDatabaseInput) => Promise<void>;
}

/** Creates a database protected by a passphrase the user chooses (003 FR-003,
 * FR-004, FR-009), saying before it exists where it and its backups will be
 * (003 FR-024). The passphrase and the acknowledgement are asked for in the
 * native dialog **Create database…** opens, never in the page (amended
 * 2026-11-04 to 003 contracts/ui-databases.md §2). */
export function CreateDatabaseDialog({
  open,
  onOpenChange,
  suggested,
  flatpak = false,
  onCreate,
}: CreateDatabaseDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange} title="Create a database" size="lg" bare>
      {/* Mounted only while open, so every opening starts from the suggestion. */}
      <CreateDatabaseForm
        suggested={suggested}
        flatpak={flatpak}
        onCreate={onCreate}
        onCancel={() => onOpenChange(false)}
      />
    </Dialog>
  );
}

type Errors = Partial<Record<"name" | "folder", string>>;

/** The order the fields are read in, for the first one refused to take the focus. */
const FIELD_ORDER = ["name", "folder"] as const satisfies readonly (keyof Errors)[];

function CreateDatabaseForm({
  suggested,
  flatpak,
  onCreate,
  onCancel,
}: {
  suggested: SuggestedLocation;
  flatpak: boolean;
  onCreate: (input: CreateDatabaseInput) => Promise<void>;
  onCancel: () => void;
}) {
  const [name, setName] = useState(suggested.name);
  // The suggested folder until one is picked in the chooser the backend opens,
  // whose answer it keeps: the page sends only its token.
  const [folder, setFolder] = useState<ChosenPath | null>(null);
  const [errors, setErrors] = useState<Errors>({});
  const [serverError, setServerError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const formRef = useRef<HTMLFormElement>(null);
  // The field the backend refused, to focus once the form is enabled again
  // (a disabled field can't take focus).
  const [refused, setRefused] = useState<keyof Errors | null>(null);

  useEffect(() => {
    if (submitting || !refused) return;
    focusFormField(formRef.current, refused);
    setRefused(null);
  }, [submitting, refused]);

  function clearError(field: keyof Errors) {
    setErrors((current) => ({ ...current, [field]: undefined }));
  }
  const trimmedName = name.trim();
  const folderPath = folder?.path ?? suggested.folder;
  const shownName = trimmedName || suggested.name;
  const target = joinPath(folderPath, `${shownName}.hoplodex`);
  const backups = joinPath(folderPath, "HoploDex backups");

  async function chooseFolder() {
    try {
      const chosen = await withIdlePaused(() => databasesService.chooseDatabaseFolder());
      if (chosen) {
        setFolder(chosen);
        clearError("folder");
      }
    } catch (e) {
      setErrors((current) => ({
        ...current,
        folder: e instanceof CommandFailure ? e.message : "The folder couldn't be chosen.",
      }));
    }
  }

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    const found: Errors = {};
    if (!trimmedName) found.name = "Enter a name.";
    setErrors(found);
    setServerError(null);
    if (Object.keys(found).length > 0) {
      setRefused(firstRefusedField(FIELD_ORDER, found) ?? null);
      return;
    }

    setSubmitting(true);
    try {
      // The backend checks the name and folder, then asks for the
      // passphrase in the native dialog.
      await onCreate({ ...(folder ? { folderToken: folder.token } : {}), name: trimmedName });
    } catch (e) {
      if (e instanceof CommandFailure && e.code === "PASSPHRASE_CANCELLED") {
        // Cancelled in the dialog: nothing was made, and the form stays.
      } else if (e instanceof CommandFailure && e.fieldErrors) {
        setErrors(e.fieldErrors);
        // As every form does (the constitution's UI consistency): the first
        // field the backend refused takes the focus.
        setRefused(firstRefusedField(FIELD_ORDER, e.fieldErrors) ?? null);
      } else if (e instanceof CommandFailure && e.code === "DATABASE_EXISTS") {
        setErrors({ name: e.message });
        setRefused("name");
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
    <form ref={formRef} className="hd-dialog__form" onSubmit={handleSubmit} noValidate>
      <div className="hd-dialog__body hd-form-section">
        {serverError && (
          <p className="hd-banner hd-banner--error" role="alert">
            {serverError}
          </p>
        )}
        <div data-field="name">
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
        </div>
        <div data-field="folder">
          <ChosenPathField
            label="Folder"
            value={folderPath}
            chooseLabel="Choose a folder…"
            onChoose={() => void chooseFolder()}
            hint={`Saved as ${target}`}
            error={errors.folder}
            disabled={submitting}
          />
          <FlatpakReachNote flatpak={flatpak} />
        </div>
        <section className="hd-privacy-note" role="note" aria-labelledby="create-db-backups">
          <Icon name="archive" size={16} />
          <div>
            <h3 id="create-db-backups" className="hd-privacy-note__title">
              Backups
            </h3>
            <p>
              Backups are on. When “{shownName}” locks or HoploDex quits after a change, HoploDex
              saves a copy in <strong className="hd-privacy-note__path">{backups}</strong>, at most
              once a day, keeping the latest {DEFAULT_SETTINGS.keepCount}. Each backup holds the
              whole collection and opens with the passphrase you had when it was made. You can
              change this in the database settings.
            </p>
          </div>
        </section>
      </div>
      <footer className="hd-dialog__footer">
        <Button variant="secondary" onClick={onCancel} disabled={submitting}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" pending={submitting}>
          {submitting ? "Creating…" : "Create database…"}
        </Button>
      </footer>
    </form>
  );
}
