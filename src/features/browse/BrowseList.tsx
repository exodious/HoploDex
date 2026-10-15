import { useCallback, useMemo } from "react";
import { formatDollars } from "../../lib/money";
import { FirearmName } from "../app/FirearmName";
import { CoverageCell } from "./CoverageCell";
import { FirearmThumbnail } from "./FirearmThumbnail";
import { GroupedTable } from "./GroupedTable";
import type { TableColumn } from "./GroupedTable";
import { MountLines } from "./MountLines";
import { caliberText } from "./types";
import type { FirearmSummary, GroupBy, VisibleGroup } from "./types";

export interface BrowseListProps {
  groups: VisibleGroup[];
  groupBy: GroupBy | undefined;
  onSelect: (id: number) => void;
}

/** Row/list layout for browsing the collection (US2 Acceptance Scenario
 * 1): one table for every group, with the serial number under each name so
 * firearms sharing a make and model stay distinguishable. */
export function BrowseList({ groups, groupBy, onSelect }: BrowseListProps) {
  // The columns and the click handler stay the same while the grouping and
  // `onSelect` do, so rows already mounted are not rendered again when more
  // rows are added (GroupedTable, issue #51).
  const columns = useMemo(() => firearmColumns(groupBy, onSelect), [groupBy, onSelect]);
  const onRowClick = useCallback((firearm: FirearmSummary) => onSelect(firearm.id), [onSelect]);
  return (
    <GroupedTable
      columns={columns}
      groups={groups.map((group) => ({
        key: group.key,
        title: group.key,
        total: group.total,
        rows: group.firearms,
      }))}
      grouped={groupBy !== undefined}
      rowKey={firearmKey}
      isMuted={isDisposed}
      onRowClick={onRowClick}
    />
  );
}

const firearmKey = (firearm: FirearmSummary) => firearm.id;
const isDisposed = (firearm: FirearmSummary) => firearm.status === "disposed";

/** The columns, in order, less the ones the grouping repeats in its titles.
 * Widths are shares of the table, so they are the same in every group. The
 * Caliber cell carries the cartridge too, so it goes for either grouping
 * (FR-027). Type is what a narrow window drops. */
function firearmColumns(
  groupBy: GroupBy | undefined,
  onSelect: (id: number) => void,
): TableColumn<FirearmSummary>[] {
  const columns: (TableColumn<FirearmSummary> | false)[] = [
    {
      id: "firearm",
      header: "Firearm",
      cell: (firearm) => (
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
            <MountLines mountedOn={firearm.mountedOn} mountedCounts={firearm.mountedCounts} />
          </div>
        </div>
      ),
    },
    groupBy !== "caliber" &&
      groupBy !== "cartridge" && {
        id: "caliber",
        header: "Caliber",
        width: "15%",
        cellClassName: "hd-table__caliber",
        cell: (firearm) => (
          // Truncated to fit; the full text stays in the row's name.
          <span className="hd-cell-truncate" title={caliberText(firearm)}>
            {caliberText(firearm)}
          </span>
        ),
      },
    groupBy !== "type" && {
      id: "type",
      header: "Type",
      width: "11%",
      narrowHidden: true,
      cell: (firearm) => firearm.firearmTypeName,
    },
    groupBy !== "action_type" && {
      id: "action",
      header: "Action",
      width: "13%",
      cell: (firearm) => firearm.actionTypeName,
    },
    {
      id: "value",
      header: "Est. value",
      width: "14%",
      numeric: true,
      cell: (firearm) => formatDollars(firearm.estimatedValue),
    },
    {
      id: "coverage",
      header: "Coverage",
      width: "14%",
      cell: (firearm) => <CoverageCell firearm={firearm} />,
    },
  ];
  return columns.filter((column) => column !== false);
}

export function GroupHeading({ group }: { group: VisibleGroup }) {
  return (
    <h2 className="hd-group__title">
      {group.key}
      <span className="hd-group__count hd-num">{group.total}</span>
    </h2>
  );
}
