import { formatDate } from "../../lib/dates";
import { formatCents } from "../../lib/money";
import { dispositionLabel } from "./types";
import type { DispositionHistoryEntry } from "./types";

/** The dispositions a restored firearm kept as history (FR-033), newest
 * first. Read-only; nothing renders when there are none. */
export function DispositionHistoryList({ entries }: { entries: DispositionHistoryEntry[] }) {
  if (entries.length === 0) return null;
  return (
    <>
      <h3 className="hd-subhead">Earlier dispositions</h3>
      <ul className="hd-past-dispositions">
        {entries.map((entry) => (
          <li key={entry.id}>
            <strong>{dispositionLabel(entry.dispositionType)}</strong> to{" "}
            {entry.dispositionRecipient} on {formatDate(entry.dispositionDate)}
            {entry.dispositionPrice != null && <> for {formatCents(entry.dispositionPrice)}</>}
            <span className="hd-muted hd-past-dispositions__restored">
              Restored {formatDate(entry.reversedAt.slice(0, 10))}
            </span>
          </li>
        ))}
      </ul>
    </>
  );
}
