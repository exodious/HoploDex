import { formatDollars } from "../../lib/money";
import { FirearmName } from "../app/FirearmName";
import { GroupHeading } from "./BrowseList";
import { CoverageCell } from "./CoverageCell";
import { FirearmThumbnail } from "./FirearmThumbnail";
import { hasAny } from "../mounts/recordCounts";
import { MountLines } from "./MountLines";
import { caliberText } from "./types";
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
                {/* The card holds the tile's button and, beside it, the mount lines:
                    the host is a link, and a link cannot sit inside a button. */}
                <div className="hd-tile-card">
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
                        <span>{caliberText(firearm)}</span>
                        <span aria-hidden> · </span>
                        {firearm.serialNumber ? (
                          <span className="hd-serial">{firearm.serialNumber}</span>
                        ) : (
                          "No serial"
                        )}
                      </span>
                      <span className="hd-tile__foot">
                        <span className="hd-tile__value hd-num">
                          {formatDollars(firearm.estimatedValue)}
                        </span>
                        <CoverageCell firearm={firearm} />
                      </span>
                    </span>
                  </button>
                  {(firearm.mountedOn || hasAny(firearm.mountedCounts)) && (
                    <div className="hd-tile__mounts">
                      <MountLines
                        mountedOn={firearm.mountedOn}
                        mountedCounts={firearm.mountedCounts}
                      />
                    </div>
                  )}
                </div>
              </li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}
