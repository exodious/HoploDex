import { useId } from "react";
import { Button, Icon, Menu, MenuItem } from "../../components";
import { MountedList } from "./MountedList";
import type { MountedEntry, RecordLabel } from "./types";
import "./mounts.css";

// 006 contracts/ui-accessories.md §5 (006 FR-012, FR-013), as 014 FR-028
// amends it: the Mounted section of an active record's page lists what is
// mounted on it (006 research.md §14), read-only. What is mounted is changed
// in the record's edit form, held until its Save (014 FR-024); the Mount menu
// keeps only New accessory…, which creates a record. The wording is "mount"
// and "Mounted", never "item" or "host".

export interface MountedSectionProps {
  /** The record whose page this is. */
  record: RecordLabel;
  /** `MountDetail.mounted`: everything below it, depth-first. */
  mounted: MountedEntry[];
  /** The page opens the accessory form with Mounted on preset to `record`. */
  onNewAccessory: () => void;
  /** The page opens the record's edit form at its Mounted list. */
  onChangeMounted: () => void;
}

export function MountedSection({ mounted, onNewAccessory, onChangeMounted }: MountedSectionProps) {
  const titleId = useId();

  return (
    <section className="hd-panel hd-mounted-section" aria-labelledby={titleId}>
      <header className="hd-panel__head">
        <h2 className="hd-panel__title" id={titleId}>
          Mounted
        </h2>
        <div className="hd-panel__actions">
          <Button size="sm" variant="ghost" icon="pencil" onClick={onChangeMounted}>
            Change what&apos;s mounted…
          </Button>
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
          </Menu>
        </div>
      </header>

      {mounted.length === 0 ? (
        <p className="hd-panel__empty">Nothing mounted.</p>
      ) : (
        <MountedList entries={mounted} />
      )}
    </section>
  );
}
