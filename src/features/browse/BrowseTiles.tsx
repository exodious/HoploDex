import { InsuranceWarningBadge } from "../../components";
import { FirearmThumbnail } from "./FirearmThumbnail";
import type { FirearmGroup } from "./types";

export interface BrowseTilesProps {
  groups: FirearmGroup[];
  onSelect: (id: number) => void;
}

/** Tile/thumbnail layout for browsing the collection (US2 Acceptance
 * Scenario 1). Shows a firearm's own photo thumbnail, or its type's
 * bundled generic thumbnail when it has none (FR-009, US4 Scenario 3). */
export function BrowseTiles({ groups, onSelect }: BrowseTilesProps) {
  const isGrouped = groups.length > 1 || (groups[0]?.key ?? "All") !== "All";

  return (
    <div>
      {groups.map((group) => (
        <section key={group.key}>
          {isGrouped && <h3>{group.key}</h3>}
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fill, minmax(160px, 1fr))",
              gap: 12,
            }}
          >
            {group.firearms.map((firearm) => (
              <button
                key={firearm.id}
                type="button"
                onClick={() => onSelect(firearm.id)}
                style={{
                  display: "flex",
                  flexDirection: "column",
                  alignItems: "center",
                  gap: 6,
                  padding: 12,
                }}
              >
                <FirearmThumbnail
                  thumbnailPhotoId={firearm.thumbnailPhotoId}
                  genericThumbnailKey={firearm.genericThumbnailKey}
                  alt=""
                  size={96}
                />
                <span>
                  {firearm.make} {firearm.model}
                </span>
                {firearm.insuranceWarning !== "none" && (
                  <InsuranceWarningBadge kind={firearm.insuranceWarning} />
                )}
              </button>
            ))}
          </div>
        </section>
      ))}
      {groups.every((g) => g.firearms.length === 0) && (
        <p>No firearms match. Add your first one, or clear the search/group filters.</p>
      )}
    </div>
  );
}
