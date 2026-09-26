import { useEffect, useRef } from "react";
import type { FormEvent } from "react";
import { Button, Icon, Menu, MenuItem, PassphraseField } from "../../components";
import type { PassphraseFieldHandle } from "../../components";
import { formatDateTime } from "../../lib/dates";
import { folderOf, middleTruncate } from "./paths";

export interface ChooserRow {
  path: string;
  name: string;
  /** The file is at `path` (FR-012). */
  available: boolean;
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
  onSelect: () => void;
  onOpen: (passphrase: string) => void;
  onRemove: () => void;
  onLocate: () => void;
  onGoBack: () => void;
  onTakeOver: () => void;
}

/** One database in the chooser: its name and folder and, when selected, its
 * passphrase and **Open**, or what stopped it opening
 * (contracts/ui-databases.md §1). */
export function RecentDatabaseRow({
  entry,
  selected,
  disabled,
  opening,
  error,
  unavailableNote,
  elsewhere,
  onSelect,
  onOpen,
  onRemove,
  onLocate,
  onGoBack,
  onTakeOver,
}: RecentDatabaseRowProps) {
  const folder = folderOf(entry.path);
  const field = useRef<PassphraseFieldHandle>(null);

  // Back to the field for another try once a failed open returns.
  const wasOpening = useRef(opening);
  useEffect(() => {
    if (wasOpening.current && !opening) field.current?.focus();
    wasOpening.current = opening;
  }, [opening]);

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
            <Identity name={entry.name} folder={folder} />
          </button>
          {overflow}
        </div>
      </li>
    );
  }

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    const passphrase = field.current?.read() ?? "";
    field.current?.reset();
    onOpen(passphrase);
  }

  return (
    <li className="hd-db-row hd-db-row--selected" aria-current="true">
      <div className="hd-db-row__head">
        <div className="hd-db-row__summary">
          <Identity name={entry.name} folder={folder} />
        </div>
        {overflow}
      </div>
      {elsewhere && !opening ? (
        <div className="hd-db-row__detail" role="alert">
          <p className="hd-db-row__message">
            {entry.name} is marked as open on <strong>{elsewhere.machineName}</strong> since{" "}
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
        <form className="hd-db-row__unlock" onSubmit={handleSubmit} noValidate>
          <PassphraseField
            ref={field}
            label={`Passphrase for ${entry.name}`}
            autoComplete="current-password"
            autoFocus
            disabled={opening}
            error={error}
            fieldClassName="hd-db-row__passphrase"
          />
          <Button type="submit" variant="primary" pending={opening}>
            {opening ? "Opening…" : "Open"}
          </Button>
        </form>
      )}
    </li>
  );
}

function Identity({ name, folder }: { name: string; folder: string }) {
  return (
    <>
      <Icon name="archive" className="hd-db-row__icon" />
      <span className="hd-db-row__text">
        <span className="hd-db-row__name">{name}</span>
        <span className="hd-db-row__folder" title={folder}>
          {middleTruncate(folder)}
        </span>
      </span>
    </>
  );
}
