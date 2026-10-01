import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CollectionContext } from "../app/collectionStore";
import * as browseService from "../browse/browseService";
import type { CollectionState } from "../app/collectionStore";
import { ExportDialog } from "./ExportDialog";
import * as importExportService from "./importExportService";

vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("./importExportService");
vi.mock("../browse/browseService");

const browse = { query: "", groupBy: undefined, includeDisposed: false, view: "list" } as const;

function renderDialog(
  firearms: { registeredAs: string | null }[] = [],
  browseState: Parameters<typeof ExportDialog>[0]["browse"] = browse,
) {
  render(
    <CollectionContext.Provider value={{ firearms } as unknown as CollectionState}>
      <ExportDialog open onOpenChange={vi.fn()} browse={browseState} />
    </CollectionContext.Provider>,
  );
}

describe("ExportDialog disclosure (Constitution V: what leaves the device, and where)", () => {
  beforeEach(() => {
    vi.mocked(importExportService.exportCollection).mockReset();
  });

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

  function listing(...registeredAs: (string | null)[]) {
    vi.mocked(browseService.listFirearms).mockResolvedValue({
      groups: [{ firearms: registeredAs.map((r) => ({ registeredAs: r })) }],
    } as unknown as Awaited<ReturnType<typeof browseService.listFirearms>>);
  }

  it("ends with serial numbers and values when no firearm is registered", () => {
    renderDialog([{ registeredAs: null }]);
    expect(screen.getByRole("note")).toHaveTextContent(PLAIN);
  });

  it("adds registration details when any firearm in the whole collection is registered", () => {
    renderDialog([{ registeredAs: null }, { registeredAs: "Suppressor" }]);
    expect(screen.getByRole("note")).toHaveTextContent(WITH_REGISTRATION);
  });

  it("follows the scope: the current results decide it once they are chosen", async () => {
    const user = userEvent.setup();
    listing(null);
    renderDialog([{ registeredAs: "Suppressor" }], filtered);
    expect(screen.getByRole("note")).toHaveTextContent(WITH_REGISTRATION);

    await user.click(await screen.findByRole("radio", { name: /Current results/ }));
    expect(screen.getByRole("note")).toHaveTextContent(PLAIN);

    await user.click(screen.getByRole("radio", { name: /Entire collection/ }));
    expect(screen.getByRole("note")).toHaveTextContent(WITH_REGISTRATION);
  });

  it("adds registration details when the current results hold a registered firearm", async () => {
    const user = userEvent.setup();
    listing(null, "Machine gun");
    renderDialog([{ registeredAs: null }], filtered);
    expect(screen.getByRole("note")).toHaveTextContent(PLAIN);

    await user.click(await screen.findByRole("radio", { name: /Current results/ }));
    expect(screen.getByRole("note")).toHaveTextContent(WITH_REGISTRATION);
  });
});
