import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { ImportDialog } from "./ImportDialog";
import * as importExportService from "./importExportService";
import type { ImportResult } from "./types";

vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("./importExportService");

const collection = { refresh: vi.fn().mockResolvedValue(undefined) } as unknown as CollectionState;

const result: ImportResult = {
  sessionId: "import-1",
  importedCount: 1,
  updatedCount: 0,
  skippedCount: 0,
  rowErrors: [],
  warnings: [],
  conflicts: [
    {
      conflictId: "c1",
      row: 2,
      existingFirearmId: 10,
      duplicateAllowed: false,
      make: "Glock",
      model: "19",
      serialNumber: "ABC123",
    },
    {
      conflictId: "c2",
      row: 3,
      existingFirearmId: 11,
      duplicateAllowed: true,
      make: "Glock",
      model: "26",
      serialNumber: "XYZ789",
    },
  ],
};

async function importTheFile(user: ReturnType<typeof userEvent.setup>) {
  render(
    <CollectionContext.Provider value={collection}>
      <ImportDialog open onOpenChange={vi.fn()} />
    </CollectionContext.Provider>,
  );
  await user.type(screen.getByLabelText(/Spreadsheet file/), "/tmp/import.csv");
  await user.click(screen.getByRole("button", { name: "Import" }));
  await screen.findByText(/matches a firearm|match firearms/);
}

describe("ImportDialog (FR-026, FR-032)", () => {
  beforeEach(() => {
    vi.mocked(importExportService.importCollection).mockResolvedValue(result);
  });

  it("offers 'Add as new' only where FR-032 would allow the new record", async () => {
    const user = userEvent.setup();
    await importTheFile(user);

    const blocked = screen.getByRole("radiogroup", { name: "Row 2: Glock 19" });
    expect(within(blocked).queryByRole("radio", { name: "Add as new" })).not.toBeInTheDocument();
    expect(within(blocked).getByRole("radio", { name: "Keep existing" })).toBeInTheDocument();
    expect(within(blocked).getByRole("radio", { name: "Replace existing" })).toBeInTheDocument();

    const allowed = screen.getByRole("radiogroup", { name: "Row 3: Glock 26" });
    expect(within(allowed).getByRole("radio", { name: "Add as new" })).toBeInTheDocument();
  });

  it("applying 'Add as new' to every row leaves the blocked row undecided", async () => {
    const user = userEvent.setup();
    await importTheFile(user);

    await user.click(screen.getByRole("button", { name: "Add as new" }));

    expect(
      within(screen.getByRole("radiogroup", { name: "Row 3: Glock 26" })).getByRole("radio", {
        name: "Add as new",
      }),
    ).toBeChecked();
    expect(screen.getByText("1 row still needs a decision.")).toBeInTheDocument();
  });

  it("keeps a conflict open, with the reason, when its decision couldn't be applied", async () => {
    const user = userEvent.setup();
    vi.mocked(importExportService.resolveImportConflicts).mockResolvedValue({
      resolvedCount: 1,
      unresolved: [{ row: 2, message: "nickname: That nickname is already used by Sig P226." }],
      warnings: [],
    });
    await importTheFile(user);

    await user.click(screen.getByRole("button", { name: "Keep existing" }));
    await user.click(screen.getByRole("button", { name: "Apply decisions" }));

    expect(await screen.findByText(/That nickname is already used/)).toBeInTheDocument();
    expect(screen.getByRole("radiogroup", { name: "Row 2: Glock 19" })).toBeInTheDocument();
    expect(screen.queryByRole("radiogroup", { name: "Row 3: Glock 26" })).not.toBeInTheDocument();
  });
});

describe("ImportDialog warnings (US4-6, FR-009)", () => {
  it("shows a Warnings section, separate from row errors and conflicts, with the count in the tally", async () => {
    const user = userEvent.setup();
    vi.mocked(importExportService.importCollection).mockResolvedValue({
      ...result,
      conflicts: [],
      warnings: [
        {
          row: 4,
          message:
            "Ridgeline Arms Hi-Power (serial RA-1) already has these original maker's marks.",
        },
      ],
    });
    render(
      <CollectionContext.Provider value={collection}>
        <ImportDialog open onOpenChange={vi.fn()} />
      </CollectionContext.Provider>,
    );
    await user.type(screen.getByLabelText(/Spreadsheet file/), "/tmp/import.csv");
    await user.click(screen.getByRole("button", { name: "Import" }));

    expect(await screen.findByRole("heading", { name: "Warnings" })).toBeInTheDocument();
    expect(screen.getByText("Row 4")).toBeInTheDocument();
    expect(screen.getByText(/already has these original maker's marks/)).toBeInTheDocument();
    const warningsTally = screen.getByText("warnings").closest(".hd-tally__item");
    expect(within(warningsTally as HTMLElement).getByText("1")).toBeInTheDocument();
  });

  it("shows nothing when there are no warnings", async () => {
    const user = userEvent.setup();
    vi.mocked(importExportService.importCollection).mockResolvedValue({
      ...result,
      conflicts: [],
      warnings: [],
    });
    render(
      <CollectionContext.Provider value={collection}>
        <ImportDialog open onOpenChange={vi.fn()} />
      </CollectionContext.Provider>,
    );
    await user.type(screen.getByLabelText(/Spreadsheet file/), "/tmp/import.csv");
    await user.click(screen.getByRole("button", { name: "Import" }));
    await screen.findByRole("button", { name: "Done" });

    expect(screen.queryByRole("heading", { name: "Warnings" })).not.toBeInTheDocument();
    expect(screen.queryByText("warnings")).not.toBeInTheDocument();
  });
});
