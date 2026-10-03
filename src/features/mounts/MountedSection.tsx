import { useId, useState } from "react";
import { Button, ConfirmDialog, Dialog, Icon, Menu, MenuItem, useToast } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import { useCollection } from "../app/collectionStore";
import { MountedList } from "./MountedList";
import { MountChooser } from "./MountChooser";
import { mountRecord } from "./mountsService";
import { recordKey } from "./recordKey";
import { recordNameText } from "./recordNames";
import type { MountedEntry, RecordLabel } from "./types";
import "./mounts.css";

// specs/006-accessory-links contracts/ui-accessories.md §5 (FR-012, FR-013):
// the Mounted section of an active record's page. The list (research.md §14),
// the Mount menu, the "Mount on {name}" dialog, the move question and the
// unmount question. The wording is "mount" and "Mounted", never "item" or
// "host".

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
  // The directly mounted record whose Unmount the user pressed, and whether
  // anything is mounted on it in turn.
  const [unmounting, setUnmounting] = useState<{ item: RecordLabel; carries: boolean } | null>(
    null,
  );
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

  async function choose(item: RecordLabel, mountedOn: RecordLabel | null = null) {
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
                onClick={() =>
                  setUnmounting({
                    item: entry.label,
                    carries: mounted.some(
                      (other) => recordKey(other.host) === recordKey(entry.label.record),
                    ),
                  })
                }
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
        <div className="hd-mount-picker">
          <MountChooser
            role="item"
            record={record.record}
            value={null}
            label="Accessory or firearm"
            placeholder="Search by make, model, nickname or serial number"
            error={error}
            onChange={(item, mountedOn) => item && void choose(item, mountedOn)}
          />
        </div>
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

      <ConfirmDialog
        open={unmounting !== null}
        onOpenChange={(open) => !open && setUnmounting(null)}
        title={unmounting ? `Unmount ${recordNameText(unmounting.item)}?` : "Unmount?"}
        description={
          unmounting
            ? `It will no longer be mounted on ${hostName}.${
                unmounting.carries ? " Everything mounted on it stays mounted on it." : ""
              }`
            : ""
        }
        confirmLabel="Unmount"
        destructive={false}
        onConfirm={async () => {
          if (unmounting) await place(unmounting.item, false);
        }}
      />
    </section>
  );
}
