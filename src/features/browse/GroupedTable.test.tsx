import { readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";
import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { render } from "../../test/sessionScope";
import { GroupedTable } from "./GroupedTable";
import type { TableColumn, TableGroup } from "./GroupedTable";

interface Item {
  id: number;
  name: string;
  kind: string;
  amount: number;
  gone?: boolean;
}

const COLUMNS: TableColumn<Item>[] = [
  { id: "name", header: "Name", cell: (item) => item.name },
  { id: "kind", header: "Kind", width: "20%", narrowHidden: true, cell: (item) => item.kind },
  { id: "amount", header: "Amount", width: "15%", numeric: true, cell: (item) => item.amount },
];

const GROUPS: TableGroup<Item>[] = [
  {
    key: "a",
    title: "Alpha",
    total: 3,
    rows: [
      { id: 1, name: "One", kind: "x", amount: 10 },
      { id: 2, name: "Two", kind: "x", amount: 20, gone: true },
    ],
  },
  { key: "b", title: "Beta", total: 1, rows: [{ id: 3, name: "Three", kind: "y", amount: 30 }] },
  { key: "c", title: "Gamma", total: 1, rows: [{ id: 4, name: "Four", kind: "z", amount: 40 }] },
];

function renderTable(
  props: Partial<Parameters<typeof GroupedTable<Item>>[0]> = {},
  groups = GROUPS,
) {
  return render(
    <GroupedTable<Item>
      columns={COLUMNS}
      groups={groups}
      grouped
      rowKey={(item) => item.id}
      isMuted={(item) => item.gone === true}
      onRowClick={() => {}}
      {...props}
    />,
  );
}

// Issue #51: one table for the whole page, so columns line up across groups.
describe("GroupedTable", () => {
  it("is one table with one heading row, however many groups there are", () => {
    renderTable();

    expect(screen.getAllByRole("table")).toHaveLength(1);
    expect(screen.getAllByRole("columnheader").map((h) => h.textContent)).toEqual([
      "Name",
      "Kind",
      "Amount",
    ]);
    // The heading row is visible: in a head, not hidden from view.
    const table = screen.getByRole("table");
    expect(table.querySelectorAll("thead")).toHaveLength(1);
    expect(table.querySelector("thead")).not.toHaveClass("hd-sr-only");
  });

  it("gives each group a row group named by its title, opening with the title row and count", () => {
    renderTable();

    const groups = screen.getAllByRole("rowgroup").filter((g) => g.tagName === "TBODY");
    expect(groups).toHaveLength(3);
    const alpha = screen.getByRole("rowgroup", { name: "Alpha" });
    const [titleRow, ...rows] = within(alpha).getAllByRole("row");
    expect(titleRow).toHaveTextContent("Alpha3");
    expect(within(titleRow).getByRole("heading", { level: 2 })).toHaveTextContent("Alpha3");
    // The title row spans every column.
    expect(within(titleRow).getByRole("cell")).toHaveAttribute("colspan", "3");
    // The count is the group's whole size; the rows are what is rendered so far.
    expect(rows).toHaveLength(2);
    expect(screen.getByRole("rowgroup", { name: "Beta" })).toBeInTheDocument();
    expect(screen.getByRole("rowgroup", { name: "Gamma" })).toBeInTheDocument();
  });

  it("has no title row, and no named row group, when not grouped", () => {
    renderTable({ grouped: false }, [
      { key: "All", title: "All", total: 4, rows: [GROUPS[0].rows[0]] },
    ]);

    expect(screen.getAllByRole("table")).toHaveLength(1);
    expect(screen.queryByRole("heading")).not.toBeInTheDocument();
    expect(screen.getAllByRole("row")).toHaveLength(2); // heading row, one item
    expect(screen.queryByRole("rowgroup", { name: /./ })).not.toBeInTheDocument();
    expect(screen.getAllByRole("columnheader")).toHaveLength(3);
  });

  it("sets the same column widths whatever the number of groups", () => {
    const widths = (groups: TableGroup<Item>[]) => {
      const { container, unmount } = renderTable({}, groups);
      const result = [...container.querySelectorAll<HTMLElement>("col")].map((col) => [
        col.className,
        col.style.width,
      ]);
      unmount();
      return result;
    };

    const one = widths([GROUPS[0]]);
    expect(one).toEqual([
      ["hd-col--name", ""],
      ["hd-col--kind hd-col--narrow-hidden", "20%"],
      ["hd-col--amount", "15%"],
    ]);
    expect(widths(GROUPS)).toEqual(one);
  });

  it("marks a narrow-hidden column in its heading, cells and <col>, and only that one", () => {
    const { container } = renderTable();

    const hidden = [...container.querySelectorAll(".hd-col--narrow-hidden")];
    expect(hidden.map((el) => el.tagName).sort()).toEqual(["COL", "TD", "TD", "TD", "TD", "TH"]);
    expect(hidden.every((el) => el.classList.contains("hd-col--kind"))).toBe(true);
    // The title row's one cell spans the columns, so it is never one of them.
    expect(container.querySelector(".hd-group-row td")).not.toHaveClass("hd-col--narrow-hidden");
  });

  it("hides the narrow column in CSS by its class, not by its position", () => {
    const css = readFileSync("src/features/browse/collection.css", "utf8");

    // The old rule hid the third column, which was a different column for
    // each grouping.
    expect(css).not.toMatch(/nth-child/);
    expect(css).toMatch(
      /@media \(max-width: 900px\)[^@]*\.hd-table \.hd-col--narrow-hidden\s*\{\s*display: none;/,
    );
  });

  it("right-aligns a numeric column and mutes a muted row", () => {
    renderTable();

    expect(screen.getByRole("columnheader", { name: "Amount" })).toHaveClass("hd-table__num");
    expect(screen.getByText("10")).toHaveClass("hd-table__num", "hd-num");
    expect(screen.getByText("Two").closest("tr")).toHaveClass("hd-row", "hd-row--disposed");
    expect(screen.getByText("One").closest("tr")).not.toHaveClass("hd-row--disposed");
  });

  it("reports a clicked row", async () => {
    const onRowClick = vi.fn();
    renderTable({ onRowClick });

    await userEvent.setup().click(screen.getByText("Three"));

    expect(onRowClick).toHaveBeenCalledWith(GROUPS[1].rows[0]);
  });

  // Issue #51: every 150-row batch used to render every row already mounted
  // (about 400 ms of React commit at 5,000 to 10,000 rows).
  describe("rendering more rows", () => {
    const items = (from: number, to: number): Item[] =>
      Array.from({ length: to - from }, (_, i) => ({
        id: from + i,
        name: `Item ${from + i}`,
        kind: "x",
        amount: from + i,
      }));
    const group = (rows: Item[]): TableGroup<Item>[] => [
      { key: "All", title: "All", total: 400, rows },
    ];
    const rowKey = (item: Item) => item.id;
    const onRowClick = () => {};

    function countedColumns() {
      const cell = vi.fn((item: Item) => item.name);
      const columns: TableColumn<Item>[] = [
        { id: "name", header: "Name", cell },
        { id: "amount", header: "Amount", numeric: true, cell: (item) => item.amount },
      ];
      return { cell, columns };
    }

    it("renders only the rows added, not the ones already mounted", () => {
      const { cell, columns } = countedColumns();
      const first = items(0, 150);
      const { rerender } = render(
        <GroupedTable<Item>
          columns={columns}
          groups={group(first)}
          grouped
          rowKey={rowKey}
          onRowClick={onRowClick}
        />,
      );
      expect(cell).toHaveBeenCalledTimes(150);
      cell.mockClear();

      // What a page does for the next batch: new group and array objects
      // around the same row objects, the same columns and callback.
      rerender(
        <GroupedTable<Item>
          columns={columns}
          groups={group([...first, ...items(150, 300)])}
          grouped
          rowKey={rowKey}
          onRowClick={onRowClick}
        />,
      );

      expect(cell).toHaveBeenCalledTimes(150);
      expect(cell.mock.calls.map(([item]) => item.id)).toEqual(
        Array.from({ length: 150 }, (_, i) => 150 + i),
      );
      expect(screen.getAllByRole("row")).toHaveLength(1 + 1 + 300); // heading, title, rows
    });

    it("renders a row again when its own row object, its muting or the columns change", () => {
      const { cell, columns } = countedColumns();
      const rows = items(0, 3);
      const table = (props: {
        columns?: TableColumn<Item>[];
        rows?: Item[];
        isMuted?: (item: Item) => boolean;
      }) => (
        <GroupedTable<Item>
          columns={props.columns ?? columns}
          groups={group(props.rows ?? rows)}
          grouped
          rowKey={rowKey}
          isMuted={props.isMuted}
          onRowClick={onRowClick}
        />
      );
      const { rerender } = render(table({}));
      cell.mockClear();

      rerender(table({ rows: [rows[0], { ...rows[1], name: "Renamed" }, rows[2]] }));
      expect(cell.mock.calls.map(([item]) => item.id)).toEqual([1]);
      expect(screen.getByText("Renamed")).toBeInTheDocument();
      cell.mockClear();

      rerender(table({ rows, isMuted: (item) => item.id === 2 }));
      expect(screen.getByText("Item 2").closest("tr")).toHaveClass("hd-row--disposed");
      cell.mockClear();

      // A regrouping hands over new columns: every row is drawn against them.
      rerender(table({ rows, columns: [...columns], isMuted: (item) => item.id === 2 }));
      expect(cell).toHaveBeenCalledTimes(3);
    });
  });
});
