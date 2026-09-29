import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import {
  Button,
  Checkbox,
  Icon,
  Menu,
  MenuItem,
  PassphraseField,
  placeFocus,
} from "../../components";
import type { PassphraseFieldHandle } from "../../components";
import { formatDateTime, formatRecentDay, formatRecentMoment } from "../../lib/dates";
import type { BackupSummary } from "./types";
import { folderOf, middleTruncate } from "./paths";
import { RememberPassphraseConfirm } from "./RememberPassphraseConfirm";

export interface ChooserRow {
  path: string;
  name: string;
  /** The file is at `path` (FR-012). */
  available: boolean;
  /** Its passphrase is saved on this computer (FR-017). */
  passphraseSaved: boolean;
  /** What this computer knows about it, for a database in the recent list
   * (FR-040); a file just picked has none. */
  details?: RowDetails;
}

export interface RowDetails {
  /** ISO-8601 UTC. */
  lastOpenedAt: string;
  backups: BackupSummary | null;
  changedSinceLeftAt: string | null;
}

/** Another computer's open marker, from `DATABASE_OPEN_ELSEWHERE`. */
export interface OpenElsewhere {
  machineName: string;
  /** ISO-8601 UTC. */
  since: string;
}

export interface RecentDatabaseRowProps {
  entry: ChooserRow;
  selected: boolean;
  /** Another row is opening. */
  disabled: boolean;
  /** This row is opening. */
  opening: boolean;
  /** Why the last open failed, shown in the field's error slot. */
  error?: string;
  /** Why an unavailable row can't be opened. */
  unavailableNote?: string;
  /** The last open found the database open on another computer. */
  elsewhere?: OpenElsewhere;
  /** The saved passphrase no longer opens it, so it is asked for (US5-5). */
  savedFailed?: boolean;
  /** Passphrases can be saved on this computer (FR-019). */
  keyringAvailable: boolean;
  /** Restoring from a backup is offered below the error: the database is
   * damaged, or the passphrase didn't open it, and it has backups (US3-6). */
  onRestore?: () => void;
  onSelect: () => void;
  /** Opens with the typed passphrase, remembered on this computer when
   * `remember` is set, or with the saved one when `passphrase` is `null`. */
  onOpen: (passphrase: string | null, remember: boolean) => void;
  onRemove: () => void;
  onLocate: () => void;
  onGoBack: () => void;
  onTakeOver: () => void;
}

/** One database in the chooser: its name and folder and, when selected, what
 * this computer knows about it, its passphrase and **Open** (only **Open**
 * when the passphrase is saved), or what stopped it opening
 * (contracts/ui-databases.md §1). */
export function RecentDatabaseRow({
  entry,
  selected,
  disabled,
  opening,
  error,
  unavailableNote,
  elsewhere,
  savedFailed = false,
  keyringAvailable,
  onSelect,
  onOpen,
  onRemove,
  onLocate,
  onGoBack,
  onTakeOver,
  onRestore,
}: RecentDatabaseRowProps) {
  const folder = folderOf(entry.path);
  const field = useRef<PassphraseFieldHandle>(null);
  const openButton = useRef<HTMLButtonElement>(null);
  /** Opens with the saved passphrase, with no prompt. */
  const usesSaved = entry.passphraseSaved && !savedFailed;
  const [remember, setRemember] = useState(false);
  const [confirmingRemember, setConfirmingRemember] = useState(false);

  // Back to the field, or to Open, for another try once a failed open
  // returns.
  const wasOpening = useRef(opening);
  useEffect(() => {
    if (wasOpening.current && !opening) {
      if (field.current) field.current.focus();
      else placeFocus(openButton.current);
    }
    wasOpening.current = opening;
  }, [opening]);

  // Remembering is asked for each time a row is selected.
  useEffect(() => {
    if (!selected) setRemember(false);
  }, [selected]);

  const overflow = (
    <Menu
      align="end"
      trigger={
        <Button
          variant="ghost"
          size="sm"
          icon="more"
          className="hd-db-row__more"
          aria-label={`More actions for ${entry.name}`}
          disabled={disabled || opening}
        />
      }
    >
      <MenuItem onSelect={onRemove} note="The database file is not deleted.">
        Remove from list
      </MenuItem>
    </Menu>
  );

  if (!entry.available) {
    return (
      <li className="hd-db-row hd-db-row--unavailable">
        <div className="hd-db-row__summary">
          <Identity name={entry.name} folder={folder} />
        </div>
        <div className="hd-db-row__detail">
          <p className="hd-db-row__note">{unavailableNote ?? "Not found at this location"}</p>
          <div className="hd-db-row__actions">
            <Button size="sm" icon="folder" disabled={disabled} onClick={onLocate}>
              Locate…
            </Button>
            <Button size="sm" variant="ghost" disabled={disabled} onClick={onRemove}>
              Remove from list
            </Button>
          </div>
        </div>
      </li>
    );
  }

  if (!selected) {
    return (
      <li className="hd-db-row">
        <div className="hd-db-row__head">
          <button
            type="button"
            className="hd-db-row__select"
            aria-label={`${entry.name}, ${folder}`}
            disabled={disabled}
            onClick={onSelect}
          >
            <Identity name={entry.name} folder={folder} saved={entry.passphraseSaved} />
          </button>
          {overflow}
        </div>
      </li>
    );
  }

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    if (usesSaved) {
      onOpen(null, false);
      return;
    }
    const passphrase = field.current?.read() ?? "";
    field.current?.reset();
    onOpen(passphrase, remember);
  }

  return (
    <li className="hd-db-row hd-db-row--selected" aria-current="true">
      <div className="hd-db-row__head">
        <div className="hd-db-row__summary">
          <Identity name={entry.name} folder={folder} saved={usesSaved} />
        </div>
        {overflow}
      </div>
      {elsewhere && !opening ? (
        <div className="hd-db-row__detail" role="alert">
          <p className="hd-db-row__message">
            “{entry.name}” is marked as open on <strong>{elsewhere.machineName}</strong> since{" "}
            {formatDateTime(elsewhere.since)}. It may still be open there, may not have been closed
            properly, or its latest changes may not have synced to this computer yet.
          </p>
          <div className="hd-db-row__actions">
            <Button autoFocus onClick={onGoBack}>
              Go back
            </Button>
            <Button variant="danger" onClick={onTakeOver}>
              Take over…
            </Button>
          </div>
        </div>
      ) : (
        <>
          {entry.details && <Details details={entry.details} />}
          <form
            className={
              usesSaved ? "hd-db-row__unlock hd-db-row__unlock--saved" : "hd-db-row__unlock"
            }
            onSubmit={handleSubmit}
            noValidate
          >
            {!usesSaved && (
              <PassphraseField
                ref={field}
                label={`Passphrase for “${entry.name}”`}
                autoComplete="current-password"
                autoFocus
                disabled={opening}
                error={error}
                fieldClassName="hd-db-row__passphrase"
              />
            )}
            <Button
              ref={openButton}
              type="submit"
              variant="primary"
              pending={opening}
              autoFocus={usesSaved}
            >
              {opening ? "Opening…" : "Open"}
            </Button>
          </form>
          {usesSaved && error && (
            <p className="hd-field__error hd-db-row__saved-error" role="alert">
              {error}
            </p>
          )}
          {/* A saved passphrase that failed is replaced by the next one that
              works, so there is nothing to ask. */}
          {!entry.passphraseSaved && (
            <div className="hd-db-row__remember">
              <Checkbox
                label="Remember on this computer"
                checked={remember}
                disabled={opening || !keyringAvailable}
                hint={
                  keyringAvailable
                    ? undefined
                    : "Not available: this computer has no keyring service."
                }
                onCheckedChange={(checked) =>
                  checked ? setConfirmingRemember(true) : setRemember(false)
                }
              />
              <RememberPassphraseConfirm
                open={confirmingRemember}
                name={entry.name}
                onClose={() => setConfirmingRemember(false)}
                onConfirm={() => setRemember(true)}
              />
            </div>
          )}
          {onRestore && !opening && (
            <div className="hd-db-row__actions hd-db-row__restore">
              <Button size="sm" icon="archive" onClick={onRestore}>
                Restore from a backup…
              </Button>
            </div>
          )}
        </>
      )}
    </li>
  );
}

function Identity({
  name,
  folder,
  saved = false,
}: {
  name: string;
  folder: string;
  saved?: boolean;
}) {
  return (
    <>
      <Icon name="archive" className="hd-db-row__icon" />
      <span className="hd-db-row__text">
        <span className="hd-db-row__name">{name}</span>
        <span className="hd-db-row__folder" title={folder}>
          {middleTruncate(folder)}
        </span>
        {saved && (
          <span className="hd-db-row__saved">Opens without a passphrase on this computer</span>
        )}
      </span>
    </>
  );
}

/** "on September 27", or "today" and "yesterday" alone. */
function onDay(moment: string): string {
  const day = formatRecentDay(moment);
  return day === "today" || day === "yesterday" ? day : `on ${day}`;
}

/** What this computer knows about the selected database before it is
 * unlocked (FR-040): its last open here, whether the file changed after it
 * was last closed here, and its backups. Nothing from inside it. */
function Details({ details }: { details: RowDetails }) {
  const { lastOpenedAt, backups, changedSinceLeftAt } = details;
  return (
    <dl className="hd-db-row__details">
      <div className="hd-db-row__fact">
        <dt>Last opened here</dt>
        <dd>
          {formatRecentMoment(lastOpenedAt)}
          {changedSinceLeftAt && (
            <span className="hd-db-row__changed">
              Changed {onDay(changedSinceLeftAt)}, after it was last closed here.
            </span>
          )}
        </dd>
      </div>
      {backups && (
        <div className="hd-db-row__fact">
          <dt>Last backup</dt>
          <dd>
            {backups.latestMadeAt ? formatRecentMoment(backups.latestMadeAt) : "None"}
            {backups.count === 1 && <span className="hd-db-row__fact-note">1 kept</span>}
            {backups.count > 1 && backups.oldestMadeAt && (
              <span className="hd-db-row__fact-note">
                {backups.count} kept, the oldest from {formatRecentDay(backups.oldestMadeAt)}
              </span>
            )}
          </dd>
        </div>
      )}
    </dl>
  );
}
