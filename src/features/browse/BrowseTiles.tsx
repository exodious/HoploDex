import { formatCents } from "../../lib/money";
import { FirearmName } from "../app/FirearmName";
import { GroupHeading } from "./BrowseList";
import { CoverageCell } from "./CoverageCell";
import { FirearmThumbnail } from "./FirearmThumbnail";
import type { VisibleGroup } from "./types";

export interface BrowseTilesProps {
  groups: VisibleGroup[];
  grouped: boolean;
  onSelect: (id: number) => void;
}

/** Tile/thumbnail layout for browsing the collection (US2 Acceptance
 * Scenario 1). Shows a firearm's own photo thumbnail, or its type's
 * generic drawing when it has none (FR-009, US4 Scenario 3). */
export function BrowseTiles({ groups, grouped, onSelect }: BrowseTilesProps) {
  return (
    <div className="hd-browse">
      {groups.map((group) => (
        <section key={group.key} className="hd-group" aria-label={grouped ? group.key : undefined}>
          {grouped && <GroupHeading group={group} />}
          <ul className="hd-tiles">
            {group.firearms.map((firearm) => (
              <li key={firearm.id}>
                <button
                  type="button"
                  className={
                    firearm.status === "disposed" ? "hd-tile hd-tile--disposed" : "hd-tile"
                  }
                  onClick={() => onSelect(firearm.id)}
                >
                  <FirearmThumbnail
                    className="hd-tile__image"
                    thumbnailPhotoId={firearm.thumbnailPhotoId}
                    genericThumbnailKey={firearm.genericThumbnailKey}
                  />
                  <span className="hd-tile__body">
                    <span className="hd-tile__name">
                      <FirearmName firearm={firearm} />
                    </span>
                    <span className="hd-tile__meta">
                      {firearm.caliber}
                      <span aria-hidden> · </span>
                      {firearm.serialNumber ? (
                        <span className="hd-serial">{firearm.serialNumber}</span>
                      ) : (
                        "No serial"
                      )}
                    </span>
                    <span className="hd-tile__foot">
                      <span className="hd-tile__value hd-num">
                        {formatCents(firearm.estimatedValue, { whole: true })}
                      </span>
                      <CoverageCell firearm={firearm} />
                    </span>
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}
