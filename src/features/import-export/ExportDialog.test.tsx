import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { ExportDialog } from "./ExportDialog";
import * as importExportService from "./importExportService";

vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("./importExportService");

const collection = { firearms: [] } as unknown as CollectionState;
const browse = { query: "", groupBy: undefined, includeDisposed: false, view: "list" } as const;

function renderDialog() {
  render(
    <CollectionContext.Provider value={collection}>
      <ExportDialog open onOpenChange={vi.fn()} browse={browse} />
    </CollectionContext.Provider>,
  );
}

describe("ExportDialog disclosure (Constitution V: what leaves the device, and where)", () => {
  beforeEach(() => {
    vi.mocked(importExportService.exportCollection).mockReset();
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
