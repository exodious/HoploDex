import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { CommandFailure } from "../../services/tauriClient";
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
  importedAccessoryCount: 0,
  updatedCount: 0,
  skippedCount: 0,
  rowErrors: [],
  warnings: [],
  derivedCalibers: [],
  snappedValues: [],
  conflicts: [
    {
      conflictId: "c1",
      table: "firearms",
      row: 2,
      existingRecord: { kind: "firearm", id: 10 },
      kindName: null,
      duplicateAllowed: false,
      make: "Glock",
      model: "19",
      serialNumber: "ABC123",
    },
    {
      conflictId: "c2",
      table: "firearms",
      row: 3,
      existingRecord: { kind: "firearm", id: 11 },
      kindName: null,
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

    const blocked = screen.getByRole("radiogroup", { name: "Firearms, row 2: Glock 19" });
    expect(within(blocked).queryByRole("radio", { name: "Add as new" })).not.toBeInTheDocument();
    expect(within(blocked).getByRole("radio", { name: "Keep existing" })).toBeInTheDocument();
    expect(within(blocked).getByRole("radio", { name: "Replace existing" })).toBeInTheDocument();

    const allowed = screen.getByRole("radiogroup", { name: "Firearms, row 3: Glock 26" });
    expect(within(allowed).getByRole("radio", { name: "Add as new" })).toBeInTheDocument();
  });

  it("applying 'Add as new' to every row leaves the blocked row undecided", async () => {
    const user = userEvent.setup();
    await importTheFile(user);

    await user.click(screen.getByRole("button", { name: "Add as new" }));

    expect(
      within(screen.getByRole("radiogroup", { name: "Firearms, row 3: Glock 26" })).getByRole("radio", {
        name: "Add as new",
      }),
    ).toBeChecked();
    expect(screen.getByText("1 row still needs a decision.")).toBeInTheDocument();
  });

  it("keeps a conflict open, with the reason, when its decision couldn't be applied", async () => {
    const user = userEvent.setup();
    vi.mocked(importExportService.resolveImportConflicts).mockResolvedValue({
      resolvedCount: 1,
      unresolved: [{ table: "firearms", row: 2, message: "nickname: That nickname is already used by Sig P226." }],
      warnings: [],
    });
    await importTheFile(user);

    await user.click(screen.getByRole("button", { name: "Keep existing" }));
    await user.click(screen.getByRole("button", { name: "Apply decisions" }));

    expect(await screen.findByText(/That nickname is already used/)).toBeInTheDocument();
    expect(screen.getByRole("radiogroup", { name: "Firearms, row 2: Glock 19" })).toBeInTheDocument();
    expect(screen.queryByRole("radiogroup", { name: "Firearms, row 3: Glock 26" })).not.toBeInTheDocument();
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
          table: "firearms",
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
    expect(screen.getByText("Firearms, row 4")).toBeInTheDocument();
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

describe("ImportDialog report of derived and matched values (US4, FR-025, FR-026, SC-007)", () => {
  async function importWith(overrides: Partial<ImportResult>) {
    const user = userEvent.setup();
    vi.mocked(importExportService.importCollection).mockResolvedValue({
      ...result,
      conflicts: [],
      ...overrides,
    });
    render(
      <CollectionContext.Provider value={collection}>
        <ImportDialog open onOpenChange={vi.fn()} />
      </CollectionContext.Provider>,
    );
    await user.type(screen.getByLabelText(/Spreadsheet file/), "/tmp/import.csv");
    await user.click(screen.getByRole("button", { name: "Import" }));
    await screen.findByRole("button", { name: "Done" });
    return user;
  }

  it("lists the calibers filled in from the cartridge in row order, saying which were guessed", async () => {
    const user = await importWith({
      derivedCalibers: [
        { table: "firearms", row: 4, cartridge: "9x19mm Parabellum", caliber: "9mm", source: "catalog" },
        { table: "firearms", row: 7, cartridge: ".30 Custom Improved", caliber: ".30", source: "guess" },
      ],
    });

    await user.click(
      screen.getByRole("button", { name: /Calibers filled in from the cartridge \(2\)/ }),
    );

    const rows = screen
      .getAllByRole("listitem")
      .map((item) => [...item.querySelectorAll("span")].map((part) => part.textContent));
    expect(rows).toEqual([
      ["Firearms, row 4", "9x19mm Parabellum → 9mm (built-in)"],
      ["Firearms, row 7", ".30 Custom Improved → .30 (guessed)"],
    ]);
  });

  it('labels a snapped registration form and "Registered to" by name (US4)', async () => {
    const user = await importWith({
      snappedValues: [
        { table: "firearms", row: 2, field: "registrationForm", sheetValue: "FORM 4", recordedValue: "Form 4" },
        { table: "firearms", row: 3, field: "registeredTo", sheetValue: "jane doe", recordedValue: "Jane Doe" },
      ],
    });

    await user.click(
      screen.getByRole("button", { name: /Spellings matched to existing values \(2\)/ }),
    );

    const rows = screen
      .getAllByRole("listitem")
      .map((item) => [...item.querySelectorAll("span")].map((part) => part.textContent));
    expect(rows).toEqual([
      ["Firearms, row 2", "Form: “FORM 4” → “Form 4”"],
      ["Firearms, row 3", "Registered to: “jane doe” → “Jane Doe”"],
    ]);
  });

  it("lists the spellings matched to existing values", async () => {
    const user = await importWith({
      snappedValues: [
        {
          table: "firearms",
          row: 2,
          field: "make",
          sheetValue: "springfield armory",
          recordedValue: "Springfield Armory",
        },
        {
          table: "firearms",
          row: 5,
          field: "cartridge",
          sheetValue: "9X19mm Parabellum",
          recordedValue: "9x19mm Parabellum",
        },
      ],
    });

    await user.click(
      screen.getByRole("button", { name: /Spellings matched to existing values \(2\)/ }),
    );

    const rows = screen
      .getAllByRole("listitem")
      .map((item) => [...item.querySelectorAll("span")].map((part) => part.textContent));
    expect(rows).toEqual([
      ["Firearms, row 2", "make: “springfield armory” → “Springfield Armory”"],
      ["Firearms, row 5", "cartridge: “9X19mm Parabellum” → “9x19mm Parabellum”"],
    ]);
  });

  it("shows each section only when it has rows", async () => {
    await importWith({
      derivedCalibers: [
        { table: "firearms", row: 4, cartridge: "9x19mm Parabellum", caliber: "9mm", source: "catalog" },
      ],
    });

    expect(
      screen.getByRole("button", { name: /Calibers filled in from the cartridge \(1\)/ }),
    ).toBeInTheDocument();
    expect(screen.queryByText(/Spellings matched to existing values/)).not.toBeInTheDocument();
  });

  it("shows neither section when nothing was derived or matched", async () => {
    await importWith({});

    expect(screen.queryByText(/Calibers filled in from the cartridge/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Spellings matched to existing values/)).not.toBeInTheDocument();
  });
});
