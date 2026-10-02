import { useRef, useState } from "react";
import { Combobox, Icon } from "../../components";
import type { ComboboxOption } from "../../components";
import { listMountCandidates } from "./mountsService";
import type { MountCandidate } from "./mountsService";
import { recordNameText } from "./recordNames";
import type { RecordLabel, RecordRef } from "./types";

// specs/006-accessory-links contracts/ui-accessories.md §4 (FR-010, FR-012):
// the "Mounted on" field of both forms, and the search of the Mounted
// section's "Mount on {name}" dialog. A `Combobox` over `list_mount_candidates`.

export interface MountChooserProps {
  /** What is chosen now; `null` = not mounted. */
  value: RecordLabel | null;
  /** `mountedOn` is where a chosen candidate is mounted now (`null` when
   * nowhere), for the question a move needs (§5); it is not passed when the
   * field is cleared. */
  onChange: (label: RecordLabel | null, mountedOn?: RecordLabel | null) => void;
  /** The record being placed (role "host": left out of the list with
   * everything mounted on it; `null` for a new record), or the record
   * receiving the choice (role "item"). */
  record: RecordRef | null;
  /** "host" (default): choosing what `record` is mounted on. "item": choosing
   * what to mount on `record`. */
  role?: "host" | "item";
  /** The field's label; "Mounted on" unless the dialog says otherwise. */
  label?: string;
  /** What the empty field says; "Not mounted" unless the dialog says otherwise. */
  placeholder?: string;
  hint?: string;
  error?: string;
  id?: string;
}

const NOT_MOUNTED_KEY = "none";

function candidateKey(label: RecordLabel): string {
  return `${label.record.kind}:${label.record.id}`;
}

/** The muted text after a name: a firearm's type, then the serial number. An
 * accessory's kind is already in its name (FR-005). */
function candidateMarker(label: RecordLabel): string | undefined {
  const parts: string[] = [];
  if (label.record.kind === "firearm") parts.push(label.typeName);
  if (label.serialNumber) parts.push(label.serialNumber);
  return parts.length > 0 ? parts.join(" · ") : undefined;
}

export function MountChooser({
  value,
  onChange,
  record,
  role = "host",
  label = "Mounted on",
  placeholder = "Not mounted",
  hint,
  error,
  id,
}: MountChooserProps) {
  // The text typed since the field was last settled; `null` shows `value`.
  const [typed, setTyped] = useState<string | null>(null);
  const typedNow = useRef<string | null>(null);
  const candidates = useRef(new Map<string, MountCandidate>());
  const text = typed ?? (value ? recordNameText(value) : "");

  function setTypedText(next: string | null) {
    typedNow.current = next;
    setTyped(next);
  }

  async function loadOptions(searched: string): Promise<ComboboxOption[]> {
    // The field showing the chosen name is not a search for it.
    const query = typedNow.current === null ? "" : searched.trim();
    const { candidates: found } = await listMountCandidates({ role, record, query });
    const rows: ComboboxOption[] = found.map((entry: MountCandidate) => {
      const { label: candidate, mountedOn } = entry;
      const key = candidateKey(candidate);
      candidates.current.set(key, entry);
      return {
        key,
        value: recordNameText(candidate),
        marker: candidateMarker(candidate),
        detail: mountedOn ? `Mounted on ${recordNameText(mountedOn)}` : undefined,
      };
    });
    return query === "" && role === "host"
      ? [{ key: NOT_MOUNTED_KEY, value: "Not mounted" }, ...rows]
      : rows;
  }

  function pick(key: string) {
    setTypedText(null);
    const picked = key === NOT_MOUNTED_KEY ? undefined : candidates.current.get(key);
    if (picked) onChange(picked.label, picked.mountedOn);
    else onChange(null);
  }

  return (
    <Combobox
      id={id}
      label={label}
      value={text}
      placeholder={placeholder}
      hint={hint}
      error={error}
      loadOptions={loadOptions}
      onInputChange={setTypedText}
      onPick={pick}
      onBlurSettle={() => setTypedText(null)}
      trailing={
        value && (
          <button
            type="button"
            className="hd-input__action"
            aria-label={`Clear ${label}`}
            onClick={() => {
              setTypedText(null);
              onChange(null);
            }}
          >
            <Icon name="close" />
          </button>
        )
      }
    />
  );
}
