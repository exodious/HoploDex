import { memo, useId } from "react";
import type { ReactNode } from "react";

/** One column of a {@link GroupedTable}. The page chooses the columns, so it
 * drops the ones a grouping makes redundant; the table lays out whatever it
 * is given, the same for every group. */
export interface TableColumn<Row> {
  /** Names the column; the column's cells and heading carry `hd-col--{id}`. */
  id: string;
  header: ReactNode;
  /** The column's CSS width in every group. A column without one takes the
   * width the others leave over (`table-layout: fixed`). */
  width?: string;
  /** Right-aligned figures (`hd-table__num`). */
  numeric?: boolean;
  /** Left out below 900px, heading, cells and `<col>` together
   * (`hd-col--narrow-hidden`), whatever the grouping. */
  narrowHidden?: boolean;
  /** Extra class for the column's cells. */
  cellClassName?: string;
  cell: (row: Row) => ReactNode;
}

export interface TableGroup<Row> {
  key: string;
  /** What the group's title row says, before its count. */
  title: ReactNode;
  /** Every row the group has, which may be more than `rows`, the part
   * rendered so far. */
  total: number;
  rows: Row[];
}

export interface GroupedTableProps<Row> {
  /** Keep this array and `onRowClick` the same between renders unless they
   * really change: a row re-renders only when one of them does or its own row
   * object is a new one, so a batch of rows added to the end renders only the
   * new rows (issue #51). */
  columns: TableColumn<Row>[];
  groups: TableGroup<Row>[];
  /** Whether to give each group a title row. Ungrouped, there is one group
   * and no title. */
  grouped: boolean;
  rowKey: (row: Row) => string | number;
  /** Rows that are muted, as a disposed record is. */
  isMuted?: (row: Row) => boolean;
  onRowClick: (row: Row) => void;
  className?: string;
}

/** A column's class names, for its heading, cells and `<col>`. */
function cls<Row>(column: TableColumn<Row>, ...more: (string | false | undefined)[]): string {
  return [`hd-col--${column.id}`, column.narrowHidden && "hd-col--narrow-hidden", ...more]
    .filter(Boolean)
    .join(" ");
}

interface RowProps<Row> {
  row: Row;
  muted: boolean;
  columns: TableColumn<Row>[];
  onRowClick: (row: Row) => void;
}

/** One row. Memoised, so a batch of rows appended to the list renders only
 * the new rows, not the thousands already mounted (issue #51): a row renders
 * again only when its row object, `muted`, `columns` or `onRowClick` is a new
 * one. The list pages keep each row's object across batches and their columns
 * and callbacks across renders for this. */
const GroupedTableRow = memo(function GroupedTableRow<R>({
  row,
  muted,
  columns,
  onRowClick,
}: RowProps<R>) {
  return (
    <tr className={muted ? "hd-row hd-row--disposed" : "hd-row"} onClick={() => onRowClick(row)}>
      {columns.map((column) => (
        <td
          key={column.id}
          className={cls(column, column.numeric && "hd-table__num hd-num", column.cellClassName)}
        >
          {column.cell(row)}
        </td>
      ))}
    </tr>
  );
}) as <R>(props: RowProps<R>) => ReactNode;

/** The list layout of the collection and the Accessories page (issue #51): one
 * table for the whole page, so every group's columns line up. Its heading row
 * sticks below the top bar, and the current group's title row sticks under it,
 * until the next group's title takes its place (`collection.css`, "List
 * view"). Each group is its own labelled `<tbody>` (a row group) that opens
 * with its title row. */
export function GroupedTable<Row>({
  columns,
  groups,
  grouped,
  rowKey,
  isMuted,
  onRowClick,
  className,
}: GroupedTableProps<Row>) {
  const idPrefix = useId();
  return (
    <div className="hd-browse">
      <table className={className ? `hd-table ${className}` : "hd-table"}>
        <colgroup>
          {columns.map((column) => (
            <col
              key={column.id}
              className={cls(column)}
              style={column.width ? { width: column.width } : undefined}
            />
          ))}
        </colgroup>
        <thead>
          <tr>
            {columns.map((column) => (
              <th
                key={column.id}
                scope="col"
                className={cls(column, column.numeric && "hd-table__num")}
              >
                {column.header}
              </th>
            ))}
          </tr>
        </thead>
        {groups.map((group, index) => {
          const titleId = `${idPrefix}-group-${index}`;
          return (
            <tbody key={group.key} aria-labelledby={grouped ? titleId : undefined}>
              {grouped && (
                <tr className="hd-group-row">
                  <td colSpan={columns.length}>
                    <h2 className="hd-group__title">
                      <span id={titleId}>{group.title}</span>
                      <span className="hd-group__count hd-num">{group.total}</span>
                    </h2>
                  </td>
                </tr>
              )}
              {group.rows.map((row) => (
                <GroupedTableRow
                  key={rowKey(row)}
                  row={row}
                  muted={isMuted?.(row) === true}
                  columns={columns}
                  onRowClick={onRowClick}
                />
              ))}
            </tbody>
          );
        })}
      </table>
    </div>
  );
}
