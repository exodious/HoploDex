import { useId } from "react";
import type { ReactNode } from "react";
import { recordKey } from "./recordKey";
import { recordNameWithType } from "./recordNames";
import { RecordName } from "./RecordName";
import type { MountedEntry } from "./types";
import "./mounts.css";

// specs/006-accessory-links research.md §14, contracts/ui-accessories.md §5:
// everything mounted below a record as one flat, depth-first outline with a
// single fixed indent, so the depth never widens the layout. Shared by the
// Mounted section (an Unmount button on each direct entry) and the dispose
// dialog (§7: a keep or dispose choice and a price field on each row).

export interface MountedListProps {
  /** `MountDetail.mounted`: depth-first, below the record asked about. */
  entries: MountedEntry[];
  /** Names the list for a screen reader when its surroundings do not. */
  label?: string;
  /** Each name is a link to its record, through `navigation.open`. A dialog
   * that must not be left turns this off. */
  links?: boolean;
  /** Controls at the end of an entry's first line. */
  actions?: (entry: MountedEntry) => ReactNode;
  /** Content under an entry, indented with it. */
  detail?: (entry: MountedEntry) => ReactNode;
}

export function MountedList({ entries, label, links = true, actions, detail }: MountedListProps) {
  const idPrefix = useId();
  const byRecord = new Map(entries.map((entry) => [recordKey(entry.label.record), entry.label]));

  return (
    <ul className="hd-mounted" aria-label={label}>
      {entries.map((entry) => {
        const key = recordKey(entry.label.record);
        const direct = entry.depth <= 1;
        const host = direct ? undefined : byRecord.get(recordKey(entry.host));
        const onId = `${idPrefix}-${key}-on`;
        return (
          <li
            key={key}
            className={`hd-mounted__entry hd-mounted__entry--${direct ? "direct" : "nested"}`}
          >
            <div className="hd-mounted__line">
              <span className="hd-mounted__name">
                <RecordName
                  label={entry.label}
                  link={links}
                  withType
                  describedBy={host ? onId : undefined}
                />
              </span>
              {actions && <span className="hd-mounted__actions">{actions(entry)}</span>}
            </div>
            {host && (
              <p className="hd-mounted__on" id={onId}>
                on {recordNameWithType(host)}
              </p>
            )}
            {detail?.(entry)}
          </li>
        );
      })}
    </ul>
  );
}
