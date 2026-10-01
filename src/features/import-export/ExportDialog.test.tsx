import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { ExportDialog } from "./ExportDialog";
import * as importExportService from "./importExportService";
import type { ExportScope } from "./types";

vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("./importExportService");

const browse = { query: "", groupBy: undefined, includeDisposed: false, view: "list" } as const;

/** contracts/tauri-commands.md `get_export_scope`'s output. */
function scope(overrides: Partial<ExportScope> = {}): ExportScope {
  return {
    firearmCount: 3,
    accessoryCount: 2,
    includesRegistration: false,
    includesAccessories: true,
    ...overrides,
  };
}

/** The dialog reads its counts and disclosure from `get_export_scope`
 * (specs/006-accessory-links contracts/ui-accessories.md §11), not from the
 * collection's list or `list_firearms`. */
function mockScope(forAll: ExportScope, forFiltered: ExportScope = forAll) {
  vi.mocked(importExportService.getExportScope).mockImplementation(async (input) =>
    input.scope === "filtered" ? forFiltered : forAll,
  );
}

function renderDialog(browseState: Parameters<typeof ExportDialog>[0]["browse"] = browse) {
  render(
    <CollectionContext.Provider value={{ firearms: [] } as unknown as CollectionState}>
      <ExportDialog open onOpenChange={vi.fn()} browse={browseState} />
    </CollectionContext.Provider>,
  );
}

beforeEach(() => {
  vi.mocked(importExportService.exportCollection).mockReset();
  vi.mocked(importExportService.getExportScope).mockReset();
  mockScope(scope({ includesAccessories: false }));
});

describe("ExportDialog disclosure (Constitution V: what leaves the device, and where)", () => {
  it("describes the export as not encrypted and points to encrypted backups (FR-031)", () => {
    renderDialog();

    const dialog = screen.getByRole("dialog", { name: "Export collection" });
    const description = document.getElementById(dialog.getAttribute("aria-describedby") ?? "");
    expect(description).toHaveTextContent(
      "Exports the collection to a spreadsheet. The file is not encrypted: anyone who can open it can read it. For encrypted backups of the whole database, see Database settings.",
    );
    expect(screen.getByText("not encrypted", { selector: "strong" })).toBeInTheDocument();
  });

  it("says, before anything is chosen, that the export is unencrypted and leaves HoploDex", () => {
    renderDialog();

    const note = screen.getByRole("note");
    expect(note).toHaveTextContent(/unencrypted/i);
    expect(note).toHaveTextContent(/spreadsheet/i);
    expect(note).toHaveTextContent(/photos/i);
    expect(note).toHaveTextContent(/outside HoploDex/i);
    expect(note).toHaveTextContent(/folder you choose/i);
  });

  it("names the chosen destination folder in the notice before the user confirms", async () => {
    const user = userEvent.setup();
    renderDialog();

    await user.type(screen.getByLabelText(/Save to folder/), "/home/sam/Documents/backups");

    const note = screen.getByRole("note");
    expect(note).toHaveTextContent("/home/sam/Documents/backups");
    expect(note).toHaveTextContent(/unencrypted/i);
    expect(note).toHaveTextContent(/outside HoploDex/i);
    // Nothing has been written yet: this is the notice, not the outcome.
    expect(importExportService.exportCollection).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Export" })).toBeInTheDocument();
  });

  it("follows the folder as it changes", async () => {
    const user = userEvent.setup();
    renderDialog();
    const folder = screen.getByLabelText(/Save to folder/);

    await user.type(folder, "/tmp/a");
    expect(screen.getByRole("note")).toHaveTextContent("/tmp/a");

    await user.clear(folder);
    await user.type(folder, "/tmp/b");
    expect(screen.getByRole("note")).toHaveTextContent("/tmp/b");
    expect(screen.getByRole("note")).not.toHaveTextContent("/tmp/a");
  });
});

describe("ExportDialog registration disclosure (FR-020, US4-2)", () => {
  const PLAIN = "including serial numbers and values.";
  const WITH_REGISTRATION = "including serial numbers, values and registration details.";
  const filtered = { ...browse, query: "glock" };
  const none = scope({ includesAccessories: false, includesRegistration: false });
  const registered = scope({ includesAccessories: false, includesRegistration: true });

  it("ends with serial numbers and values when no firearm is registered", async () => {
    mockScope(none);
    renderDialog();
    await waitFor(() => expect(screen.getByRole("note")).toHaveTextContent(PLAIN));
  });

  it("adds registration details when the export includes a registered firearm", async () => {
    mockScope(registered);
    renderDialog();
    await waitFor(() => expect(screen.getByRole("note")).toHaveTextContent(WITH_REGISTRATION));
  });

  it("follows the scope: the current results decide it once they are chosen", async () => {
    const user = userEvent.setup();
    mockScope(registered, none);
    renderDialog(filtered);
    await waitFor(() => expect(screen.getByRole("note")).toHaveTextContent(WITH_REGISTRATION));

    await user.click(await screen.findByRole("radio", { name: /Current results/ }));
    await waitFor(() => expect(screen.getByRole("note")).toHaveTextContent(PLAIN));

    await user.click(screen.getByRole("radio", { name: /Entire collection/ }));
    await waitFor(() => expect(screen.getByRole("note")).toHaveTextContent(WITH_REGISTRATION));
  });

  it("adds registration details when the current results hold a registered firearm", async () => {
    const user = userEvent.setup();
    mockScope(none, registered);
    renderDialog(filtered);
    await waitFor(() => expect(screen.getByRole("note")).toHaveTextContent(PLAIN));

    await user.click(await screen.findByRole("radio", { name: /Current results/ }));
    await waitFor(() => expect(screen.getByRole("note")).toHaveTextContent(WITH_REGISTRATION));
  });
});

// specs/006-accessory-links contracts/ui-accessories.md §11 (FR-020, FR-021).
describe("ExportDialog with accessories (US5)", () => {
  const filtered = { ...browse, query: "glock" };

  /** The sentence of the privacy note that holds `phrase`. */
  function sentenceWith(phrase: string): string | undefined {
    const text = screen.getByRole("note").textContent ?? "";
    return text.split(/(?<=\.)\s+/).find((sentence) => sentence.includes(phrase));
  }

  it("reads the whole collection's counts from get_export_scope", async () => {
    mockScope(scope({ firearmCount: 3, accessoryCount: 2 }));
    renderDialog();

    expect(await screen.findByText(/3 firearms and 2 accessories/)).toBeInTheDocument();
    expect(importExportService.getExportScope).toHaveBeenCalledWith(
      expect.objectContaining({ scope: "all" }),
    );
  });

  it("shows each scope's own counts, asking for the filtered one with the current filter", async () => {
    mockScope(
      scope({ firearmCount: 3, accessoryCount: 2 }),
      scope({ firearmCount: 5, accessoryCount: 4 }),
    );
    renderDialog(filtered);

    expect(
      await screen.findByRole("radio", { name: /Entire collection.*3 firearms and 2 accessories/ }),
    ).toBeInTheDocument();
    expect(
      await screen.findByRole("radio", { name: /Current results.*5 firearms and 4 accessories/ }),
    ).toBeInTheDocument();
    expect(importExportService.getExportScope).toHaveBeenCalledWith({
      scope: "filtered",
      filter: expect.objectContaining({ query: "glock" }),
    });
  });

  it("hints that the current results include what is mounted on them, only for that scope", async () => {
    const user = userEvent.setup();
    renderDialog(filtered);
    const hint = "Includes everything mounted on these firearms.";
    expect(screen.queryByText(hint)).not.toBeInTheDocument();

    await user.click(await screen.findByRole("radio", { name: /Current results/ }));
    expect(screen.getByText(hint)).toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: /Entire collection/ }));
    expect(screen.queryByText(hint)).not.toBeInTheDocument();
  });

  it("names accessories, with their serial numbers, values and photos, when the export includes one", async () => {
    mockScope(scope({ includesAccessories: true, includesRegistration: false }));
    renderDialog();

    await waitFor(() =>
      expect(screen.getByRole("note")).toHaveTextContent(
        "accessories, with their serial numbers, values and photos",
      ),
    );
  });

  it("words the accessories and the registration details in one sentence", async () => {
    mockScope(scope({ includesAccessories: true, includesRegistration: true }));
    renderDialog();

    await waitFor(() =>
      expect(screen.getByRole("note")).toHaveTextContent(
        "accessories, with their serial numbers, values and photos",
      ),
    );
    const sentence = sentenceWith("accessories, with their serial numbers, values and photos");
    expect(sentence).toContain("registration details");
  });

  it("says nothing of accessories when the scope has none", async () => {
    mockScope(scope({ accessoryCount: 0, includesAccessories: false }));
    renderDialog();

    await waitFor(() => expect(screen.getByRole("note")).toHaveTextContent(/serial numbers/));
    expect(screen.getByRole("note")).not.toHaveTextContent(/accessories/i);
  });

  it("follows the scope for the accessories disclosure", async () => {
    const user = userEvent.setup();
    mockScope(
      scope({ includesAccessories: true }),
      scope({ accessoryCount: 0, includesAccessories: false }),
    );
    renderDialog(filtered);
    await waitFor(() => expect(screen.getByRole("note")).toHaveTextContent(/accessories, with/));

    await user.click(await screen.findByRole("radio", { name: /Current results/ }));
    await waitFor(() => expect(screen.getByRole("note")).not.toHaveTextContent(/accessories/i));
  });

  it("lists both file names when a CSV export wrote an accessory file", async () => {
    const user = userEvent.setup();
    vi.mocked(importExportService.exportCollection).mockResolvedValue({
      spreadsheetPath: "/out/hoplodex-export-20261001-120000.csv",
      accessorySpreadsheetPath: "/out/hoplodex-export-20261001-120000-accessories.csv",
      photosFolderPath: "/out/hoplodex-export-20261001-120000_photos",
      exportedFirearmCount: 3,
      exportedAccessoryCount: 2,
      exportedPhotoCount: 1,
    });
    renderDialog();

    await user.type(screen.getByLabelText(/Save to folder/), "/out");
    await user.click(screen.getByRole("button", { name: "Export" }));

    expect(await screen.findByText("/out/hoplodex-export-20261001-120000.csv")).toBeInTheDocument();
    expect(
      screen.getByText("/out/hoplodex-export-20261001-120000-accessories.csv"),
    ).toBeInTheDocument();
  });

  it("lists one spreadsheet when there is no accessory file (a workbook, or no accessory)", async () => {
    const user = userEvent.setup();
    vi.mocked(importExportService.exportCollection).mockResolvedValue({
      spreadsheetPath: "/out/hoplodex-export-20261001-120000.xlsx",
      accessorySpreadsheetPath: null,
      photosFolderPath: "/out/hoplodex-export-20261001-120000_photos",
      exportedFirearmCount: 3,
      exportedAccessoryCount: 2,
      exportedPhotoCount: 1,
    });
    renderDialog();

    await user.type(screen.getByLabelText(/Save to folder/), "/out");
    await user.click(screen.getByRole("button", { name: "Export" }));

    expect(
      await screen.findByText("/out/hoplodex-export-20261001-120000.xlsx"),
    ).toBeInTheDocument();
    expect(screen.queryByText(/-accessories\.csv/)).not.toBeInTheDocument();
  });
});
