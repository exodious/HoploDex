import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import { render, TEST_SESSION_ID } from "../../test/sessionScope";
import userEvent from "@testing-library/user-event";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { CommandFailure } from "../../services/tauriClient";
import { ExportDialog } from "./ExportDialog";
import * as importExportService from "./importExportService";
import type { ExportScope } from "./types";

/** The scope a mocked service receives: the test render's. */
const SCOPE = expect.objectContaining({ id: TEST_SESSION_ID });
vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("./importExportService");

const browse = { query: "", groupBy: undefined, includeDisposed: false, view: "list" } as const;

/** 006 contracts/tauri-commands.md `get_export_scope`'s output. */
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
 * (006 contracts/ui-accessories.md §11), not from the collection's
 * list or `list_firearms`. */
function mockScope(forAll: ExportScope, forFiltered: ExportScope = forAll) {
  vi.mocked(importExportService.getExportScope).mockImplementation(async (_scope, input) =>
    input.scope === "filtered" ? forFiltered : forAll,
  );
}

function dialogElement(browseState: Parameters<typeof ExportDialog>[0]["browse"], open = true) {
  return (
    <CollectionContext.Provider value={{ firearms: [] } as unknown as CollectionState}>
      <ExportDialog open={open} onOpenChange={vi.fn()} browse={browseState} />
    </CollectionContext.Provider>
  );
}

/** Renders the dialog open and waits for it to show, which it does once the
 * counts are read (#116). */
async function renderDialog(browseState: Parameters<typeof ExportDialog>[0]["browse"] = browse) {
  render(dialogElement(browseState));
  await screen.findByRole("dialog", { name: "Export collection" });
}

beforeEach(() => {
  vi.mocked(importExportService.exportCollection).mockReset();
  vi.mocked(importExportService.getExportScope).mockReset();
  mockScope(scope({ includesAccessories: false }));
});

describe("ExportDialog disclosure (Constitution V: what leaves the device, and where)", () => {
  it("describes the export as not encrypted and points to encrypted backups (003 FR-031)", async () => {
    await renderDialog();

    const dialog = screen.getByRole("dialog", { name: "Export collection" });
    const description = document.getElementById(dialog.getAttribute("aria-describedby") ?? "");
    expect(description).toHaveTextContent(
      "Exports the collection to a spreadsheet. The file is not encrypted: anyone who can open it can read it. For encrypted backups of the whole database, see Database settings.",
    );
    expect(screen.getByText("not encrypted", { selector: "strong" })).toBeInTheDocument();
  });

  it("says, before anything is chosen, that the export is unencrypted and leaves HoploDex", async () => {
    await renderDialog();

    const note = screen.getByRole("note");
    expect(note).toHaveTextContent(/unencrypted/i);
    expect(note).toHaveTextContent(/spreadsheet/i);
    expect(note).toHaveTextContent(/photos/i);
    expect(note).toHaveTextContent(/outside HoploDex/i);
    expect(note).toHaveTextContent(/folder you choose/i);
  });

  it("names the chosen destination folder in the notice before the user confirms", async () => {
    const user = userEvent.setup();
    await renderDialog();

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
    await renderDialog();
    const folder = screen.getByLabelText(/Save to folder/);

    await user.type(folder, "/tmp/a");
    expect(screen.getByRole("note")).toHaveTextContent("/tmp/a");

    await user.clear(folder);
    await user.type(folder, "/tmp/b");
    expect(screen.getByRole("note")).toHaveTextContent("/tmp/b");
    expect(screen.getByRole("note")).not.toHaveTextContent("/tmp/a");
  });
});

describe("ExportDialog registration disclosure (005 FR-020, US4-2)", () => {
  const PLAIN = "including serial numbers and values.";
  const WITH_REGISTRATION = "including serial numbers, values and registration details.";
  const filtered = { ...browse, query: "glock" };
  const none = scope({ includesAccessories: false, includesRegistration: false });
  const registered = scope({ includesAccessories: false, includesRegistration: true });

  it("ends with serial numbers and values when no firearm is registered", async () => {
    mockScope(none);
    await renderDialog();
    await waitFor(() => expect(screen.getByRole("note")).toHaveTextContent(PLAIN));
  });

  it("adds registration details when the export includes a registered firearm", async () => {
    mockScope(registered);
    await renderDialog();
    await waitFor(() => expect(screen.getByRole("note")).toHaveTextContent(WITH_REGISTRATION));
  });

  it("follows the scope: the current results decide it once they are chosen", async () => {
    const user = userEvent.setup();
    mockScope(registered, none);
    await renderDialog(filtered);
    await waitFor(() => expect(screen.getByRole("note")).toHaveTextContent(WITH_REGISTRATION));

    await user.click(await screen.findByRole("radio", { name: /Current results/ }));
    await waitFor(() => expect(screen.getByRole("note")).toHaveTextContent(PLAIN));

    await user.click(screen.getByRole("radio", { name: /Entire collection/ }));
    await waitFor(() => expect(screen.getByRole("note")).toHaveTextContent(WITH_REGISTRATION));
  });

  it("adds registration details when the current results hold a registered firearm", async () => {
    const user = userEvent.setup();
    mockScope(none, registered);
    await renderDialog(filtered);
    await waitFor(() => expect(screen.getByRole("note")).toHaveTextContent(PLAIN));

    await user.click(await screen.findByRole("radio", { name: /Current results/ }));
    await waitFor(() => expect(screen.getByRole("note")).toHaveTextContent(WITH_REGISTRATION));
  });
});

// 006 contracts/ui-accessories.md §11 (006 FR-020, FR-021).
describe("ExportDialog with accessories (006 US5)", () => {
  const filtered = { ...browse, query: "glock" };

  /** The sentence of the privacy note that holds `phrase`. */
  function sentenceWith(phrase: string): string | undefined {
    const text = screen.getByRole("note").textContent ?? "";
    return text.split(/(?<=\.)\s+/).find((sentence) => sentence.includes(phrase));
  }

  it("reads the whole collection's counts from get_export_scope", async () => {
    mockScope(scope({ firearmCount: 3, accessoryCount: 2 }));
    await renderDialog();

    expect(await screen.findByText(/3 firearms and 2 accessories/)).toBeInTheDocument();
    expect(importExportService.getExportScope).toHaveBeenCalledWith(
      SCOPE,
      expect.objectContaining({ scope: "all" }),
    );
  });

  // 001 US5, filtered export edge case
  it("shows each scope's own counts, asking for the filtered one with the current filter", async () => {
    mockScope(
      scope({ firearmCount: 3, accessoryCount: 2 }),
      scope({ firearmCount: 5, accessoryCount: 4 }),
    );
    await renderDialog(filtered);

    expect(
      await screen.findByRole("radio", { name: /Entire collection.*3 firearms and 2 accessories/ }),
    ).toBeInTheDocument();
    expect(
      await screen.findByRole("radio", { name: /Current results.*5 firearms and 4 accessories/ }),
    ).toBeInTheDocument();
    expect(importExportService.getExportScope).toHaveBeenCalledWith(SCOPE, {
      scope: "filtered",
      filter: expect.objectContaining({ query: "glock" }),
    });
  });

  it("hints that the current results include what is mounted on them, only for that scope", async () => {
    const user = userEvent.setup();
    await renderDialog(filtered);
    const hint = "Includes everything mounted on these firearms.";
    expect(screen.queryByText(hint)).not.toBeInTheDocument();

    await user.click(await screen.findByRole("radio", { name: /Current results/ }));
    expect(screen.getByText(hint)).toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: /Entire collection/ }));
    expect(screen.queryByText(hint)).not.toBeInTheDocument();
  });

  it("names accessories, with their serial numbers, values and photos, when the export includes one", async () => {
    mockScope(scope({ includesAccessories: true, includesRegistration: false }));
    await renderDialog();

    await waitFor(() =>
      expect(screen.getByRole("note")).toHaveTextContent(
        "accessories, with their serial numbers, values and photos",
      ),
    );
  });

  it("words the accessories and the registration details in one sentence", async () => {
    mockScope(scope({ includesAccessories: true, includesRegistration: true }));
    await renderDialog();

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
    await renderDialog();

    await waitFor(() => expect(screen.getByRole("note")).toHaveTextContent(/serial numbers/));
    expect(screen.getByRole("note")).not.toHaveTextContent(/accessories/i);
  });

  it("follows the scope for the accessories disclosure", async () => {
    const user = userEvent.setup();
    mockScope(
      scope({ includesAccessories: true }),
      scope({ accessoryCount: 0, includesAccessories: false }),
    );
    await renderDialog(filtered);
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
    await renderDialog();

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
    await renderDialog();

    await user.type(screen.getByLabelText(/Save to folder/), "/out");
    await user.click(screen.getByRole("button", { name: "Export" }));

    expect(
      await screen.findByText("/out/hoplodex-export-20261001-120000.xlsx"),
    ).toBeInTheDocument();
    expect(screen.queryByText(/-accessories\.csv/)).not.toBeInTheDocument();
  });
});

// 011 US1-8, 011 FR-004: inside a Flatpak a destination outside every grant is refused before
// anything is written (011 contracts/tauri-commands.md, "Commands that check reach").
describe("ExportDialog refused as out of reach (011 US1-8, 011 FR-004)", () => {
  it("shows the backend's message and stays open for another folder", async () => {
    const user = userEvent.setup();
    const message =
      "This copy of HoploDex is a Flatpak, and it can only reach the HoploDex folder in your Documents folder (/home/sam/Documents/HoploDex) and removable drives, so it can't use /srv/out. Move the file into /home/sam/Documents/HoploDex or onto a removable drive, or give HoploDex access to /srv/out by running: flatpak override --user --filesystem=/srv/out io.github.exodious.HoploDex and then restarting HoploDex.";
    vi.mocked(importExportService.exportCollection).mockRejectedValue(
      new CommandFailure({
        code: "LOCATION_OUT_OF_REACH",
        message,
        details: {
          path: "/srv/out",
          folder: "/srv/out",
          grants: ["/home/sam/Documents/HoploDex", "/media", "/run/media"],
        },
      }),
    );
    await renderDialog();

    await user.type(screen.getByLabelText(/Save to folder/), "/srv/out");
    await user.click(screen.getByRole("button", { name: "Export" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(message);
    expect(screen.getByRole("button", { name: "Export" })).toBeEnabled();
  });
});

// 001 spec.md US5, the filtered-export edge case: only the
// current search results are exported when the user asks for them.
describe("ExportDialog exporting the current results (001 US5, filtered export)", () => {
  const filtered = { ...browse, query: "glock" };
  const exported = {
    spreadsheetPath: "/out/hoplodex-export-20261001-120000.csv",
    accessorySpreadsheetPath: null,
    photosFolderPath: "/out/hoplodex-export-20261001-120000_photos",
    exportedFirearmCount: 1,
    exportedAccessoryCount: 0,
    exportedPhotoCount: 0,
  };

  it("sends scope filtered with the page's search when the current results are chosen", async () => {
    const user = userEvent.setup();
    vi.mocked(importExportService.exportCollection).mockResolvedValue(exported);
    await renderDialog(filtered);
    await user.type(screen.getByLabelText(/Save to folder/), "/out");
    await user.click(await screen.findByRole("radio", { name: /Current results/ }));

    await user.click(screen.getByRole("button", { name: "Export" }));

    await screen.findByText("/out/hoplodex-export-20261001-120000.csv");
    expect(importExportService.exportCollection).toHaveBeenCalledTimes(1);
    expect(importExportService.exportCollection).toHaveBeenCalledWith(SCOPE, {
      format: "csv",
      destinationFolder: "/out",
      scope: "filtered",
      filter: { query: "glock", includeDisposed: false },
    });
  });

  it("sends scope all, with no filter, when the entire collection stays chosen", async () => {
    const user = userEvent.setup();
    vi.mocked(importExportService.exportCollection).mockResolvedValue(exported);
    await renderDialog(filtered);
    await user.type(screen.getByLabelText(/Save to folder/), "/out");

    await user.click(screen.getByRole("button", { name: "Export" }));

    await screen.findByText("/out/hoplodex-export-20261001-120000.csv");
    expect(importExportService.exportCollection).toHaveBeenCalledWith(SCOPE, {
      format: "csv",
      destinationFolder: "/out",
      scope: "all",
      filter: undefined,
    });
  });
});

// #116: a centred dialog that opens and then grows moves, so the counts and the
// disclosure that lengthen it are read before it shows.
describe("ExportDialog opens at the size it keeps (#116)", () => {
  const filtered = { ...browse, query: "glock" };
  /** A `get_export_scope` that answers when the test says so. */
  function holdScopes() {
    const answers: Record<"all" | "filtered", (scope: ExportScope) => void> = {
      all: () => {},
      filtered: () => {},
    };
    vi.mocked(importExportService.getExportScope).mockImplementation(
      (_scope, input) =>
        new Promise((resolve) => {
          answers[input.scope] = resolve;
        }),
    );
    return answers;
  }

  it("shows nothing until the counts are read, then opens with them in its first render", async () => {
    const answers = holdScopes();
    render(dialogElement(browse));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    await act(async () => answers.all(scope({ firearmCount: 3, accessoryCount: 2 })));

    const dialog = screen.getByRole("dialog", { name: "Export collection" });
    expect(within(dialog).getByText("Exports 3 firearms and 2 accessories.")).toBeInTheDocument();
    expect(within(dialog).getByRole("note")).toHaveTextContent("and accessories, with");
  });

  it("waits for the current results' counts too when a filter is active", async () => {
    const answers = holdScopes();
    render(dialogElement(filtered));

    await act(async () => answers.all(scope({ firearmCount: 3, accessoryCount: 2 })));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    await act(async () => answers.filtered(scope({ firearmCount: 1, accessoryCount: 0 })));
    const dialog = screen.getByRole("dialog", { name: "Export collection" });
    expect(
      within(dialog).getByRole("radio", {
        name: /Entire collection.*3 firearms and 2 accessories/,
      }),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole("radio", { name: /Current results.*1 firearm and 0 accessories/ }),
    ).toBeInTheDocument();
  });

  it("opens with the note naming everything when reading the counts fails", async () => {
    vi.mocked(importExportService.getExportScope).mockRejectedValue(new Error("no"));
    render(dialogElement(browse));

    const dialog = await screen.findByRole("dialog", { name: "Export collection" });
    expect(within(dialog).getByRole("note")).toHaveTextContent("and accessories, with");
  });

  it("opens anyway when reading is slow, and fills the counts in when they come", async () => {
    const answers = holdScopes();
    render(dialogElement(browse));

    expect(await screen.findByRole("dialog", { name: "Export collection" })).toBeInTheDocument();
    expect(screen.queryByText(/^Exports \d+ firearm/)).not.toBeInTheDocument();
    await act(async () => answers.all(scope({ firearmCount: 3, accessoryCount: 2 })));
    expect(screen.getByText("Exports 3 firearms and 2 accessories.")).toBeInTheDocument();
  });

  it("reads again at the next opening, and shows nothing stale while it does", async () => {
    mockScope(scope({ firearmCount: 3, accessoryCount: 2 }));
    const view = render(dialogElement(browse));
    await screen.findByRole("dialog", { name: "Export collection" });

    view.rerender(dialogElement(browse, false));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    const answers = holdScopes();
    view.rerender(dialogElement(browse));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await act(async () => answers.all(scope({ firearmCount: 4, accessoryCount: 2 })));
    expect(screen.getByText("Exports 4 firearms and 2 accessories.")).toBeInTheDocument();
    expect(importExportService.getExportScope).toHaveBeenCalledTimes(2);
  });
});
