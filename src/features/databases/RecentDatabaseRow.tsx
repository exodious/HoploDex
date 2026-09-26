import { useEffect, useRef } from "react";
import type { FormEvent } from "react";
import { Button, Icon, PassphraseField } from "../../components";
import type { PassphraseFieldHandle } from "../../components";
import { folderOf, middleTruncate } from "./paths";

export interface ChooserRow {
  path: string;
  name: string;
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
  onSelect: () => void;
  onOpen: (passphrase: string) => void;
}

/** One database in the chooser: its name and folder, and, when selected,
 * its passphrase and **Open** (contracts/ui-databases.md §1). */
export function RecentDatabaseRow({
  entry,
  selected,
  disabled,
  opening,
  error,
  onSelect,
  onOpen,
}: RecentDatabaseRowProps) {
  const folder = folderOf(entry.path);
  const field = useRef<PassphraseFieldHandle>(null);

  // Back to the field for another try once a failed open returns.
  const wasOpening = useRef(opening);
  useEffect(() => {
    if (wasOpening.current && !opening) field.current?.focus();
    wasOpening.current = opening;
  }, [opening]);

  if (!selected) {
    return (
      <li className="hd-db-row">
        <button
          type="button"
          className="hd-db-row__select"
          aria-label={`${entry.name}, ${folder}`}
          disabled={disabled}
          onClick={onSelect}
        >
          <Identity name={entry.name} folder={folder} />
        </button>
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
      <div className="hd-db-row__summary">
        <Identity name={entry.name} folder={folder} />
      </div>
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
