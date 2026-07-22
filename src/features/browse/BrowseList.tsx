import { InsuranceWarningBadge } from "../../components";
import { formatCents } from "../../lib/money";
import { FirearmThumbnail } from "./FirearmThumbnail";
import type { FirearmGroup } from "./types";

export interface BrowseListProps {
  groups: FirearmGroup[];
  onSelect: (id: number) => void;
}

/** Row/list layout for browsing the collection (US2 Acceptance Scenario 1). */
export function BrowseList({ groups, onSelect }: BrowseListProps) {
  const isGrouped = groups.length > 1 || (groups[0]?.key ?? "All") !== "All";

  return (
    <div>
      {groups.map((group) => (
        <section key={group.key}>
          {isGrouped && <h3>{group.key}</h3>}
          <ul>
            {group.firearms.map((firearm) => (
              <li key={firearm.id} style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <FirearmThumbnail
                  thumbnailPhotoId={firearm.thumbnailPhotoId}
                  genericThumbnailKey={firearm.genericThumbnailKey}
                  alt=""
                  size={40}
                />
                <button type="button" onClick={() => onSelect(firearm.id)}>
                  {firearm.make} {firearm.model}
                </button>
                <span>
                  {" "}
                  {firearm.caliber} · {formatCents(firearm.estimatedValue)}
                </span>
                {firearm.insuranceWarning !== "none" && (
                  <InsuranceWarningBadge kind={firearm.insuranceWarning} />
                )}
              </li>
            ))}
          </ul>
        </section>
      ))}
      {groups.every((g) => g.firearms.length === 0) && (
        <p>No firearms match. Add your first one, or clear the search/group filters.</p>
      )}
    </div>
  );
}
