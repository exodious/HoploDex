import { useId, useRef, useState } from "react";
import {
  Button,
  Combobox,
  ConfirmDialog,
  Dialog,
  Icon,
  Menu,
  MenuItem,
  useToast,
} from "../../components";
import type { ComboboxOption } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import { useCollection } from "../app/collectionStore";
import { MountedList } from "./MountedList";
import { listMountCandidates, mountRecord } from "./mountsService";
import type { MountCandidate } from "./mountsService";
import { recordKey } from "./recordKey";
import { recordNameText } from "./recordNames";
import type { MountedEntry, RecordLabel } from "./types";
import "./mounts.css";

// specs/006-accessory-links contracts/ui-accessories.md §5 (FR-012, FR-013):
// the Mounted section of an active record's page. The list (research.md §14),
// the Mount menu, the "Mount on {name}" dialog and the move question. The
// wording is "mount" and "Mounted", never "item" or "host".

export interface MountedSectionProps {
  /** The record whose page this is. */
  record: RecordLabel;
  /** `MountDetail.mounted`: everything below it, depth-first. */
  mounted: MountedEntry[];
  /** The page opens the accessory form with Mounted on preset to `record`. */
  onNewAccessory: () => void;
}

function failureMessage(e: unknown, fallback: string): string {
  if (e instanceof CommandFailure) return e.fieldErrors?.host ?? e.message;
  return fallback;
}

export function MountedSection({ record, mounted, onNewAccessory }: MountedSectionProps) {
  const { refresh } = useCollection();
  const notify = useToast();
  const titleId = useId();
  const [picking, setPicking] = useState(false);
  // The record the user chose that is mounted elsewhere, and where.
  const [moving, setMoving] = useState<{ item: RecordLabel; from: RecordLabel } | null>(null);
  const [error, setError] = useState<string>();
  const hostName = recordNameText(record);

  /** Moves `item` onto this record, or off whatever it is on. */
  async function place(item: RecordLabel, onto: boolean): Promise<boolean> {
    const name = recordNameText(item);
    try {
      await mountRecord(item.record, onto ? record.record : null);
    } catch (e) {
      const message = failureMessage(
        e,
        onto ? `${name} couldn't be mounted.` : `${name} couldn't be unmounted.`,
      );
      if (onto) setError(message);
      else notify(message, "error");
      return false;
    }
    await refresh();
    notify(onto ? `${name} mounted on ${hostName}.` : `${name} unmounted.`);
    return true;
  }

  async function choose(item: RecordLabel, mountedOn: RecordLabel | null) {
    setError(undefined);
    if (mountedOn) {
      setMoving({ item, from: mountedOn });
      return;
    }
    if (await place(item, true)) setPicking(false);
  }

  return (
    <section className="hd-panel hd-mounted-section" aria-labelledby={titleId}>
      <header className="hd-panel__head">
        <h2 className="hd-panel__title" id={titleId}>
          Mounted
        </h2>
        <Menu
          align="end"
          trigger={
            <Button size="sm">
              Mount
              <Icon name="chevronDown" size={16} />
            </Button>
          }
        >
          <MenuItem icon="plus" onSelect={onNewAccessory}>
            New accessory…
          </MenuItem>
          <MenuItem
            icon="search"
            onSelect={() => {
              setError(undefined);
              setPicking(true);
            }}
          >
            Existing accessory or firearm…
          </MenuItem>
        </Menu>
      </header>

      {mounted.length === 0 ? (
        <p className="hd-panel__empty">Nothing mounted.</p>
      ) : (
        <MountedList
          entries={mounted}
          actions={(entry) =>
            entry.depth <= 1 ? (
              <Button
                size="sm"
                variant="ghost"
                aria-label={`Unmount ${recordNameText(entry.label)}`}
                onClick={() => void place(entry.label, false)}
              >
                Unmount
              </Button>
            ) : null
          }
        />
      )}

      <Dialog
        open={picking}
        onOpenChange={(open) => !open && setPicking(false)}
        title={`Mount on ${hostName}`}
        size="sm"
        footer={
          <Button variant="secondary" onClick={() => setPicking(false)}>
            Cancel
          </Button>
        }
      >
        <MountSearch record={record} error={error} onChoose={choose} />
      </Dialog>

      <ConfirmDialog
        open={moving !== null}
        onOpenChange={(open) => !open && setMoving(null)}
        title={moving ? `Move ${recordNameText(moving.item)}?` : "Move?"}
        description={
          moving
            ? `It is mounted on ${recordNameText(moving.from)}. Moving it takes everything mounted on it along.`
            : ""
        }
        confirmLabel="Move"
        destructive={false}
        onConfirm={async () => {
          if (moving && (await place(moving.item, true))) setPicking(false);
        }}
      />
    </section>
  );
}

/** The muted text after a candidate's name: a firearm's type, then the serial
 * number. An accessory's kind is already in its name (FR-005). */
function marker(label: RecordLabel): string | undefined {
  const parts: string[] = [];
  if (label.record.kind === "firearm") parts.push(label.typeName);
  if (label.serialNumber) parts.push(label.serialNumber);
  return parts.length > 0 ? parts.join(" · ") : undefined;
}

/** The dialog's search: `list_mount_candidates` with role "item", over the
 * shared `Combobox`. Each option names where the record is now (§4, §5). */
function MountSearch({
  record,
  error,
  onChoose,
}: {
  record: RecordLabel;
  error: string | undefined;
  onChoose: (item: RecordLabel, mountedOn: RecordLabel | null) => void;
}) {
  const [text, setText] = useState("");
  const found = useRef(new Map<string, MountCandidate>());

  async function loadOptions(searched: string): Promise<ComboboxOption[]> {
    const { candidates } = await listMountCandidates({
      role: "item",
      record: record.record,
      query: searched.trim(),
    });
    return candidates.map((candidate) => {
      const key = recordKey(candidate.label.record);
      found.current.set(key, candidate);
      return {
        key,
        value: recordNameText(candidate.label),
        marker: marker(candidate.label),
        detail: candidate.mountedOn
          ? `Mounted on ${recordNameText(candidate.mountedOn)}`
          : undefined,
      };
    });
  }

  return (
    <Combobox
      label="Accessory or firearm"
      placeholder="Search by make, model, nickname or serial number"
      value={text}
      error={error}
      loadOptions={loadOptions}
      onInputChange={setText}
      onPick={(key) => {
        const picked = found.current.get(key);
        setText("");
        if (picked) onChoose(picked.label, picked.mountedOn);
      }}
    />
  );
}
