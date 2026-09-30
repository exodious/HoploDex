import { formatDollars } from "../../lib/money";
import { FirearmName } from "../app/FirearmName";
import { CoverageCell } from "./CoverageCell";
import { FirearmThumbnail } from "./FirearmThumbnail";
import { caliberText } from "./types";
import type { GroupBy, VisibleGroup } from "./types";

export interface BrowseListProps {
  groups: VisibleGroup[];
  groupBy: GroupBy | undefined;
  onSelect: (id: number) => void;
}

/** Row/list layout for browsing the collection (US2 Acceptance Scenario
 * 1): one table per group, with the serial number under each name so
 * firearms sharing a make and model stay distinguishable. */
export function BrowseList({ groups, groupBy, onSelect }: BrowseListProps) {
  const grouped = groupBy !== undefined;
  // A column repeating the group heading adds nothing. The Caliber cell
  // carries the cartridge too, so it goes for either (FR-027).
  const showCaliber = groupBy !== "caliber" && groupBy !== "cartridge";
  const showType = groupBy !== "type";
  const showAction = groupBy !== "action_type";
  return (
    <div className="hd-browse">
      {groups.map((group, index) => (
        <section key={group.key} className="hd-group" aria-label={grouped ? group.key : undefined}>
          {grouped && <GroupHeading group={group} />}
          <table className="hd-table">
            <thead className={index > 0 ? "hd-sr-only" : undefined}>
              <tr>
                <th scope="col">Firearm</th>
                {showCaliber && <th scope="col">Caliber</th>}
                {showType && <th scope="col">Type</th>}
                {showAction && <th scope="col">Action</th>}
                <th scope="col" className="hd-table__num">
                  Est. value
                </th>
                <th scope="col">Coverage</th>
              </tr>
            </thead>
            <tbody>
              {group.firearms.map((firearm) => (
                <tr
                  key={firearm.id}
                  className={firearm.status === "disposed" ? "hd-row hd-row--disposed" : "hd-row"}
                  onClick={() => onSelect(firearm.id)}
                >
                  <td>
                    <div className="hd-row__identity">
                      <FirearmThumbnail
                        className="hd-row__thumb"
                        thumbnailPhotoId={firearm.thumbnailPhotoId}
                        genericThumbnailKey={firearm.genericThumbnailKey}
                      />
                      <div className="hd-row__names">
                        <button
                          type="button"
                          className="hd-row__name"
                          onClick={(e) => {
                            e.stopPropagation();
                            onSelect(firearm.id);
                          }}
                        >
                          <FirearmName firearm={firearm} />
                        </button>
                        <span className="hd-row__serial">
                          {firearm.serialNumber ? (
                            <span className="hd-serial">{firearm.serialNumber}</span>
                          ) : (
                            "No serial number"
                          )}
                        </span>
                      </div>
                    </div>
                  </td>
                  {showCaliber && (
                    <td className="hd-table__caliber">
                      {/* Truncated to fit; the full text stays in the row's name. */}
                      <span className="hd-cell-truncate" title={caliberText(firearm)}>
                        {caliberText(firearm)}
                      </span>
                    </td>
                  )}
                  {showType && <td>{firearm.firearmTypeName}</td>}
                  {showAction && <td>{firearm.actionTypeName}</td>}
                  <td className="hd-table__num hd-num">{formatDollars(firearm.estimatedValue)}</td>
                  <td>
                    <CoverageCell firearm={firearm} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      ))}
    </div>
  );
}

export function GroupHeading({ group }: { group: VisibleGroup }) {
  return (
    <h2 className="hd-group__title">
      {group.key}
      <span className="hd-group__count hd-num">{group.total}</span>
    </h2>
  );
}
