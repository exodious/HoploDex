// What the FirearmForm.*.test.tsx files share (#77 step 10). The form's tests
// were one 3,200-line file that held up the whole run; they are four files by
// topic now, and these fixtures and helpers are the part they have in common.
//
// Import this module before ./FirearmForm in a test file: the `vi.mock` calls
// below must be registered before the form (which imports the mocked
// services) is loaded.
import { vi } from "vitest";
import { fireEvent, screen } from "@testing-library/react";
import type { ReactElement } from "react";
import userEvent from "@testing-library/user-event";
import { todayIso } from "../../lib/dates";
import { FIREARM_TYPES, REGISTRATION_CLASSES } from "../../test/collectionFixtures";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import type { DerivedCaliber } from "./types";
import { render as renderBase } from "../../test/sessionScope";

// specs/005-regulated-item-types: the form reads the firearm types from the
// collection store, so every render gets the fixture's five seeded types
// unless a test provides its own collection.
const typesCollection = {
  firearmTypes: { types: FIREARM_TYPES },
  registrationClasses: { classes: REGISTRATION_CLASSES },
} as unknown as CollectionState;
export function render(ui: ReactElement) {
  return renderBase(ui, {
    wrapper: ({ children }) => (
      <CollectionContext.Provider value={typesCollection}>{children}</CollectionContext.Provider>
    ),
  });
}

// specs/004-cartridges-action-types: the form settles an entry through the
// backend (`settle_entry`) and lists suggestions (`suggest_entries`); here
// stand-ins derive a few known calibers and suggest nothing unless a test says.
export const settleEntry = vi.fn();
export const suggestEntries = vi.fn();
vi.mock("./firearmsService", () => ({
  settleEntry: (_scope: unknown, field: string, text: string) => settleEntry(field, text),
  suggestEntries: (_scope: unknown, field: string, text: string, make?: string | null) =>
    suggestEntries(field, text, make),
}));

// 006 US2: the Mounted on chooser searches the mount
// candidates through the mounts service.
export const listMountCandidates = vi.fn();
vi.mock("../mounts/mountsService", () => ({
  listMountCandidates: (_scope: unknown, input: unknown) => listMountCandidates(input),
}));

const DERIVED: Record<string, DerivedCaliber> = {
  "9x19mm Parabellum": { caliber: "9mm", source: "catalog" },
  "9mm Luger": { caliber: "9mm", source: "catalog" },
  ".45 ACP": { caliber: ".45", source: "catalog" },
  ".300 Winchester Magnum": { caliber: ".30", source: "catalog" },
  ".30 Custom Improved": { caliber: ".30", source: "guess" },
  "6.5x47 Wildcat": { caliber: "6.5mm", source: "guess" },
};

/** Every test file passes this to `beforeEach`. */
export function resetServiceMocks() {
  suggestEntries.mockReset();
  suggestEntries.mockResolvedValue([]);
  settleEntry.mockReset();
  settleEntry.mockImplementation(async (field: string, text: string) => ({
    value: text.trim(),
    changedBy: null,
    derivedCaliber: field === "cartridge" ? (DERIVED[text.trim()] ?? null) : null,
  }));
  listMountCandidates.mockReset();
  listMountCandidates.mockResolvedValue({ candidates: [] });
}

/** The "Origin and year of manufacture" disclosure's button. Closed, its
 * name also carries the summary line, so match on the title alone. */
export const originGroupButton = () =>
  screen.getByRole("button", { name: /^Origin and year of manufacture/ });

/** Renders the form with the origin group open (it starts closed on a record
 * with none of its fields recorded). */
export function renderWithOriginGroup(ui: ReactElement) {
  const result = render(ui);
  if (originGroupButton().getAttribute("aria-expanded") === "false") {
    fireEvent.click(originGroupButton());
  }
  return result;
}

/** The "Physical details" disclosure's button, matched on its title alone. */
export const physicalGroupButton = () => screen.getByRole("button", { name: /^Physical details/ });

/** Renders the form with the physical details group open. */
export function renderWithPhysicalGroup(ui: ReactElement) {
  const result = render(ui);
  if (physicalGroupButton().getAttribute("aria-expanded") === "false") {
    fireEvent.click(physicalGroupButton());
  }
  return result;
}

export async function selectFirearmType(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("radio", { name: "Handgun" }));
}

export async function fillRequired(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByLabelText("Make"), "Glock");
  await user.type(screen.getByLabelText("Model"), "19");
  await user.type(screen.getByLabelText("Caliber"), "9mm");
  await selectFirearmType(user);
  await user.type(screen.getByLabelText("Serial number"), "ABC123");
}

export function tomorrowIso(): string {
  const next = new Date(`${todayIso()}T00:00:00Z`);
  next.setUTCDate(next.getUTCDate() + 1);
  return next.toISOString().slice(0, 10);
}
