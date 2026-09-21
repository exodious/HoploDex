import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { FirearmRecordPage } from "./FirearmRecordPage";
import type { FirearmDetail } from "./types";

const getFirearm = vi.fn();

vi.mock("./firearmsService", () => ({ getFirearm: (id: number) => getFirearm(id) }));
// The media panels and the thumbnail talk to Tauri; they aren't under test.
vi.mock("../media/PhotoGallery", () => ({ PhotoGallery: () => null }));
vi.mock("../media/DocumentList", () => ({ DocumentList: () => null }));
vi.mock("../browse/FirearmThumbnail", () => ({ FirearmThumbnail: () => null }));

const firearm: FirearmDetail = {
  id: 1,
  make: "Colt",
  model: "Python",
  nickname: null,
  serialNumber: "V1",
  noSerialAttested: false,
  caliber: ".357",
  firearmTypeId: 1,
  notes: null,
  accessories: null,
  status: "active",
  estimatedValue: 1250,
  acquisitionSource: null,
  acquisitionDate: null,
  acquisitionPrice: null,
  dispositionType: null,
  dispositionRecipient: null,
  dispositionDate: null,
  dispositionPrice: null,
  thumbnailPhotoId: null,
  insurancePolicyId: null,
  scheduledCoverageAmount: null,
  createdAt: "2025-01-01 00:00:00",
  updatedAt: "2025-01-01 00:00:00",
  dispositionHistory: [],
};

const collection: CollectionState = {
  firearms: [],
  firearmsById: new Map(),
  summary: null,
  policies: [],
  policiesById: new Map(),
  loaded: true,
  error: null,
  revision: 1,
  refresh: async () => {},
};

function renderPage() {
  render(
    <CollectionContext.Provider value={collection}>
      <FirearmRecordPage id={1} />
    </CollectionContext.Provider>,
  );
}

describe("FirearmRecordPage 'Add' links (FR-038, US1 Acceptance Scenario 15)", () => {
  beforeEach(() => {
    getFirearm.mockReset().mockResolvedValue(firearm);
    Element.prototype.scrollIntoView = vi.fn();
  });

  it("opens the edit form on the notes field, focused and its section highlighted", async () => {
    const user = userEvent.setup();
    renderPage();

    const notesRow = (await screen.findByText("No notes recorded.")).closest("p")!;
    await user.click(notesRow.querySelector("button")!);

    const notes = await screen.findByLabelText("Notes");
    await waitFor(() => expect(notes).toHaveFocus());
    expect(notes.closest("[data-highlight]")).not.toBeNull();
    expect(Element.prototype.scrollIntoView).toHaveBeenCalled();
  });

  it("opens the edit form on the accessories field when that Add link is followed", async () => {
    const user = userEvent.setup();
    renderPage();

    const row = (await screen.findByText("No accessories recorded.")).closest("p")!;
    await user.click(row.querySelector("button")!);

    const accessories = await screen.findByLabelText("Accessories");
    await waitFor(() => expect(accessories).toHaveFocus());
    expect(accessories.closest("[data-highlight]")).not.toBeNull();
  });

  it("opens the plain edit form, with no highlight, from the Edit button", async () => {
    const user = userEvent.setup();
    renderPage();

    await user.click(await screen.findByRole("button", { name: /^Edit/ }));

    const make = await screen.findByLabelText(/^Make/);
    await waitFor(() => expect(make).toHaveFocus());
    expect(document.querySelector("[data-highlight]")).toBeNull();
  });
});
