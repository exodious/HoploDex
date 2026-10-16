import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { AccessoryDetail, AccessorySummary } from "../accessories/types";
import type { FirearmSummary } from "../browse/types";
import type { FirearmDetail } from "../firearms/types";
import type { InsurancePolicy } from "../insurance/types";
import { SessionContext } from "../session/sessionStore";
import type { SessionState } from "../session/sessionStore";
import type { DatabaseStatus, Draft } from "../databases/types";
import { FORM_VERSION as FIREARM_FORM_VERSION } from "../firearms/FirearmForm";
import { FORM_VERSION as DISPOSE_FORM_VERSION } from "../firearms/DisposeDialog";
import { AppShell } from "./AppShell";
import { CollectionContext } from "./collectionStore";
import type { CollectionState } from "./collectionStore";
import { ACCESSORY_KINDS, FIREARM_TYPES } from "../../test/collectionFixtures";
import { peekResumedDraft, render, setResumedDraft } from "../../test/sessionScope";

const getFirearm = vi.fn();
const deleteFirearm = vi.fn();
const listFirearms = vi.fn();
const getAccessory = vi.fn();
const listAccessories = vi.fn();

// The accessory form's make, model, cartridge and caliber use the shared
// entry commands (specs/006-accessory-links FR-003).
vi.mock("../firearms/firearmsService", () => ({
  getFirearm: (_scope: unknown, id: number) => getFirearm(id),
  deleteFirearm: (_scope: unknown, id: number, confirmed: boolean) => deleteFirearm(id, confirmed),
  settleEntry: async (_scope: unknown, _field: string, text: string) => ({
    value: text.trim(),
    changedBy: null,
    derivedCaliber: null,
  }),
  suggestEntries: async () => [],
}));
vi.mock("../accessories/accessoriesService", () => ({
  getAccessory: (_scope: unknown, id: number) => getAccessory(id),
  listAccessories: (_scope: unknown, input: unknown) => listAccessories(input),
}));
vi.mock("../browse/browseService", () => ({
  listFirearms: (_scope: unknown, input: unknown) => listFirearms(input),
}));
// The media panels and thumbnails talk to Tauri; they aren't under test.
vi.mock("../media/PhotoGallery", () => ({ PhotoGallery: () => null }));
// Stands in for a text field and a menu on the page itself, outside any
// dialog, so the shell's own guards are what keep Escape from going back.
vi.mock("../media/DocumentList", async () => {
  const { DateField } = await import("../../components");
  return {
    DocumentList: () => (
      <>
        <input aria-label="Page field" />
        <textarea aria-label="Page note" />
        <DateField label="Page date" value="" onValueChange={() => {}} />
      </>
    ),
  };
});
vi.mock("../browse/FirearmThumbnail", () => ({ FirearmThumbnail: () => null }));

const policy: InsurancePolicy = {
  id: 7,
  name: "Collector Floater",
  policyNumber: "CF-100",
  insuranceCompany: "Acme Mutual",
  companyContact: null,
  agentName: null,
  agentContact: null,
  notes: null,
  blanketCoverageLimit: null,
  effectiveStartDate: "2026-01-01",
  effectiveEndDate: "2027-01-01",
  createdAt: "2026-01-01 00:00:00",
  updatedAt: "2026-01-01 00:00:00",
  isInForce: true,
  isExpired: false,
  isExpiringSoon: false,
  expiringWarning: false,
  expiredWarning: false,
};

const summary: FirearmSummary = {
  id: 1,
  make: "Colt",
  model: "Python",
  nickname: null,
  serialNumber: "V1",
  caliber: ".357",
  cartridge: null,
  firearmTypeName: "Handgun",
  actionTypeName: null,
  registeredAs: null,
  status: "active",
  thumbnailPhotoId: null,
  genericThumbnailKey: "handgun",
  estimatedValue: 1250,
  insuranceWarning: "none",
  insurancePolicyId: policy.id,
  scheduledCoverageAmount: 1250,
  mountedOn: null,
  mountedCounts: { firearms: 0, accessories: 0 },
};

// An uninsured one, so the collection offers "Review insurance".
const uninsured: FirearmSummary = {
  ...summary,
  id: 2,
  make: "Ruger",
  model: "10/22",
  serialNumber: "R2",
  insuranceWarning: "uninsured",
  insurancePolicyId: null,
  scheduledCoverageAmount: null,
};

const detail: FirearmDetail = {
  id: 1,
  make: "Colt",
  model: "Python",
  nickname: null,
  serialNumber: "V1",
  noSerialAttested: false,
  caliber: ".357",
  firearmTypeId: 1,
  cartridge: null,
  actionTypeId: null,
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
  insurancePolicyId: policy.id,
  scheduledCoverageAmount: 1250,
  barrelLengthHundredths: null,
  overallLengthHundredths: null,
  weightTenthsOz: null,
  capacity: null,
  finish: null,
  condition: null,
  origin: null,
  yearOfManufacture: null,
  countryOfManufacture: null,
  importerName: null,
  originalMake: null,
  originalModel: null,
  originalSerialNumber: null,
  registrationClassId: null,
  registrationForm: null,
  registrationApproved: null,
  registeredTo: null,
  createdAt: "2025-01-01 00:00:00",
  updatedAt: "2025-01-01 00:00:00",
  mountedOn: null,
  mount: { chain: [], mounted: [] },
  dispositionHistory: [],
};

const opticSummary: AccessorySummary = {
  id: 3,
  accessoryKindId: 1,
  kindName: "Optic",
  genericThumbnailKey: "optic",
  make: "Leupold",
  model: "VX-5HD 3-15x44",
  serialNumber: "L-5521",
  caliber: null,
  cartridge: null,
  status: "active",
  thumbnailPhotoId: null,
  estimatedValue: 1000,
  insuranceWarning: "none",
  insurancePolicyId: null,
  scheduledCoverageAmount: null,
  mountedOn: null,
  mountedCounts: { firearms: 0, accessories: 0 },
};

const opticDetail: AccessoryDetail = {
  id: 3,
  accessoryKindId: 1,
  make: "Leupold",
  model: "VX-5HD 3-15x44",
  serialNumber: "L-5521",
  caliber: null,
  cartridge: null,
  notes: null,
  status: "active",
  estimatedValue: 1000,
  acquisitionSource: null,
  acquisitionDate: null,
  acquisitionPrice: null,
  dispositionType: null,
  dispositionRecipient: null,
  dispositionDate: null,
  dispositionPrice: null,
  insurancePolicyId: null,
  scheduledCoverageAmount: null,
  mountedOn: null,
  thumbnailPhotoId: null,
  createdAt: "2025-01-01 00:00:00",
  updatedAt: "2025-01-01 00:00:00",
  dispositionHistory: [],
  mount: { chain: [], mounted: [] },
};

const OPTIC = "Leupold VX-5HD 3-15x44 · Optic";

const collection: CollectionState = {
  firearms: [summary, uninsured],
  firearmsById: new Map([
    [summary.id, summary],
    [uninsured.id, uninsured],
  ]),
  accessories: [opticSummary],
  accessoriesById: new Map([[opticSummary.id, opticSummary]]),
  accessoryKinds: { kinds: ACCESSORY_KINDS },
  accessoryKindsFailed: false,
  summary: null,
  policies: [policy],
  policiesById: new Map([[policy.id, policy]]),
  actionTypes: { actions: [], allowedByFirearmType: {} },
  actionTypesFailed: false,
  firearmTypes: { types: FIREARM_TYPES },
  firearmTypesFailed: false,
  registrationClasses: { classes: [] },
  registrationClassesFailed: false,
  loaded: true,
  error: null,
  revision: 1,
  refresh: async () => {},
};

// An open database whose notes were all dismissed.
const session = {
  status: {
    name: "Main collection",
    notes: { diskEncryption: false },
    screenLockSupported: true,
    settings: {
      backups: {
        enabled: true,
        keepCount: 5,
        location: { kind: "default", path: "/tmp/HoploDex backups", available: true },
      },
      lock: { idleEnabled: true, idleMinutes: 10, onScreenLock: false },
    },
  },
} as unknown as SessionState;

function renderShell(state: CollectionState = collection) {
  render(
    <SessionContext.Provider value={session}>
      <CollectionContext.Provider value={state}>
        <AppShell />
      </CollectionContext.Provider>
    </SessionContext.Provider>,
  );
}

async function openColt(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole("button", { name: "Colt Python" }));
  await screen.findByRole("heading", { level: 1, name: "Colt Python" });
}

const backLink = () => document.querySelector(".hd-backlink");

const onCollection = () =>
  expect(screen.getByRole("heading", { level: 1, name: "Collection" })).toBeInTheDocument();

// 001 US1 Scenario 18
describe("Escape goes back wherever a back link shows", () => {
  beforeEach(() => {
    // jsdom has no scrolling; the shell scrolls on every page change.
    window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
    getFirearm.mockReset().mockResolvedValue(detail);
    listFirearms.mockReset().mockResolvedValue({ groups: [{ key: "", firearms: [summary] }] });
  });

  it("returns from a firearm to the collection, and does nothing on the collection", async () => {
    const user = userEvent.setup();
    renderShell();
    await openColt(user);
    expect(backLink()).toHaveTextContent("Collection");
    expect(backLink()).toHaveAttribute("aria-keyshortcuts", "Escape");
    // The shortcut is shown on the control, not only announced (FR-040).
    expect(backLink()?.querySelector("kbd")).toHaveTextContent("Esc");

    await user.keyboard("{Escape}");
    onCollection();

    await user.keyboard("{Escape}");
    onCollection();
  });

  // 001 US3 Scenario 17
  it("retraces firearm → policy one step at a time", async () => {
    const user = userEvent.setup();
    renderShell();
    await openColt(user);
    await user.click(screen.getByRole("button", { name: policy.name }));
    await screen.findByRole("heading", { name: policy.name });
    expect(backLink()).toHaveTextContent("Colt Python");

    await user.keyboard("{Escape}");
    await screen.findByRole("heading", { level: 1, name: "Colt Python" });

    await user.keyboard("{Escape}");
    onCollection();
  });

  // 001 US3 Scenario 17
  it("returns from the Insurance page when a link led there", async () => {
    const user = userEvent.setup();
    renderShell();
    await user.click(await screen.findByRole("button", { name: "Review insurance" }));
    expect(screen.getByRole("heading", { level: 1, name: "Insurance" })).toBeInTheDocument();

    await user.keyboard("{Escape}");
    onCollection();
  });

  // 001 US3 Scenario 17
  it("closes an open dialog without leaving the page", async () => {
    const user = userEvent.setup();
    renderShell();
    await openColt(user);
    await user.click(screen.getByRole("button", { name: "Edit" }));
    expect(await screen.findByRole("dialog")).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "Colt Python" })).toBeInTheDocument();
  });
});

// 001 US1 Scenario 18
describe("Escape stays on a record while something else wants it (FR-040, US1/AC18)", () => {
  beforeEach(() => {
    window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
    getFirearm.mockReset().mockResolvedValue(detail);
    listFirearms.mockReset().mockResolvedValue({ groups: [{ key: "", firearms: [summary] }] });
  });

  // The page is aria-hidden behind an open modal, but still the page.
  const onColt = () => {
    expect(
      screen.getByRole("heading", { level: 1, name: "Colt Python", hidden: true }),
    ).toBeInTheDocument();
    expect(backLink()).toHaveTextContent("Collection");
  };

  async function openEdit(user: ReturnType<typeof userEvent.setup>) {
    await user.click(screen.getByRole("button", { name: "Edit" }));
    return screen.findByRole("dialog");
  }

  it("does not go back while the user is typing in a field on the page", async () => {
    const user = userEvent.setup();
    renderShell();
    await openColt(user);

    await user.click(screen.getByLabelText("Page field"));
    await user.keyboard("abc{Escape}");
    onColt();
    expect(screen.getByLabelText("Page field")).toHaveValue("abc");

    await user.click(screen.getByLabelText("Page note"));
    await user.keyboard("{Escape}");
    onColt();

    // Out of the field, the same key goes back.
    await user.click(screen.getByRole("heading", { level: 1, name: "Colt Python" }));
    await user.keyboard("{Escape}");
    onCollection();
  });

  it("closes a menu open on the page itself without leaving the page", async () => {
    const user = userEvent.setup();
    renderShell();
    await openColt(user);
    // The calendar keeps focus on its trigger button, not in a text field.
    await user.click(screen.getByRole("button", { name: "Choose page date from a calendar" }));
    expect(document.querySelector(".hd-popover")).not.toBeNull();
    expect(document.querySelector(".hd-dialog__content")).toBeNull();

    await user.keyboard("{Escape}");
    expect(document.querySelector(".hd-popover")).toBeNull();
    onColt();

    // Closing it returns focus to the date's text field, which still holds Escape.
    expect(screen.getByLabelText("Page date")).toHaveFocus();
    await user.keyboard("{Escape}");
    onColt();

    // Out of the field, the same key goes back.
    await user.click(screen.getByRole("heading", { level: 1, name: "Colt Python" }));
    await user.keyboard("{Escape}");
    onCollection();
  });

  it("closes the edit dialog from a text field inside it without leaving the page", async () => {
    const user = userEvent.setup();
    renderShell();
    await openColt(user);
    const dialog = await openEdit(user);
    const make = within(dialog).getByLabelText(/^Make/);
    await user.click(make);
    expect(make).toHaveFocus();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    onColt();
  });

  it("closes an open select list, then the dialog, without leaving the page", async () => {
    const user = userEvent.setup();
    renderShell();
    await openColt(user);
    const dialog = await openEdit(user);
    const physical = within(dialog).getByRole("button", { name: /Physical details/ });
    if (physical.getAttribute("aria-expanded") === "false") await user.click(physical);
    await user.click(within(dialog).getByRole("combobox", { name: "Condition" }));
    expect(await screen.findByRole("listbox")).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    onColt();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    onColt();
  });

  it("closes an open calendar, then the dialog, without leaving the page", async () => {
    const user = userEvent.setup();
    renderShell();
    await openColt(user);
    const dialog = await openEdit(user);
    await user.click(
      within(dialog).getByRole("button", { name: "Choose date acquired from a calendar" }),
    );
    const calendar = document.querySelector(".hd-popover");
    expect(calendar).not.toBeNull();

    await user.keyboard("{Escape}");
    expect(document.querySelector(".hd-popover")).toBeNull();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    onColt();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    onColt();
  });
});

describe("Resumed pending changes open where their form is (FR-039)", () => {
  beforeEach(() => {
    window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
    getFirearm.mockReset().mockResolvedValue(detail);
    listFirearms.mockReset().mockResolvedValue({ groups: [{ key: "", firearms: [summary] }] });
  });
  afterEach(() => setResumedDraft(null));

  /** The Colt's edit form, as a draft keeps it, with new notes. */
  const coltValues = {
    make: "Colt",
    model: "Python",
    nickname: "",
    caliber: ".357",
    firearmTypeId: "1",
    serialNumber: "V1",
    noSerialAttested: false,
    notes: "Kept at the lock",
    accessories: "",
    barrelLength: "",
    overallLength: "",
    weightPounds: "",
    weightOunces: "",
    capacity: "",
    finish: "",
    condition: "",
    estimatedValue: "1,250",
    acquisitionSource: "",
    acquisitionDate: "",
    acquisitionPrice: "",
    dispositionType: "",
    dispositionRecipient: "",
    dispositionDate: "",
    dispositionPrice: "",
    origin: "",
    yearOfManufacture: "",
    countryOfManufacture: "",
    importerName: "",
    originalMake: "",
    originalModel: "",
    originalSerialNumber: "",
  };

  /** Each form keeps drafts of its own version; the firearm form's is
   * `FIREARM_FORM_VERSION`, the others' 1 (the dispose dialog's, its own). */
  function resume(draft: Omit<Draft, "formVersion" | "label">) {
    const formVersion =
      draft.kind === "firearm" && (draft.mode === "add" || draft.mode === "edit")
        ? FIREARM_FORM_VERSION
        : draft.mode === "dispose"
          ? DISPOSE_FORM_VERSION
          : 1;
    setResumedDraft({ formVersion, label: "Colt Python", ...draft });
  }

  // 003 US6 Scenario 6: resuming reopens the edit form with the kept input.
  it("an edit reopens the firearm's edit form with the changes", async () => {
    resume({ kind: "firearm", mode: "edit", targetId: 1, values: coltValues });
    renderShell();

    const form = await screen.findByRole("dialog", { name: "Edit Colt Python" });
    expect(within(form).getByLabelText("Notes")).toHaveValue("Kept at the lock");
    // On the firearm's record, behind the modal form.
    expect(
      screen.getByRole("heading", { level: 1, name: "Colt Python", hidden: true }),
    ).toBeInTheDocument();
    expect(peekResumedDraft()).toBeNull();
  });

  it("a new firearm reopens the add form with the changes", async () => {
    resume({
      kind: "firearm",
      mode: "add",
      targetId: null,
      values: { ...coltValues, make: "Sako", model: "85", serialNumber: "" },
    });
    renderShell();

    const form = await screen.findByRole("dialog", { name: "Add firearm" });
    expect(within(form).getByLabelText(/^Make/)).toHaveValue("Sako");
  });

  it("a coverage change reopens the coverage dialog with it", async () => {
    resume({
      kind: "firearm",
      mode: "coverage",
      targetId: 1,
      values: { policyId: String(policy.id), amount: "2,000" },
    });
    renderShell();

    const dialog = await screen.findByRole("dialog", { name: "Insurance coverage" });
    expect(within(dialog).getByLabelText(/Scheduled amount/)).toHaveValue("2,000");
  });

  it("a policy edit reopens the policy's edit form with the changes", async () => {
    setResumedDraft({
      formVersion: 1,
      kind: "policy",
      mode: "edit",
      targetId: policy.id,
      label: "Collector Floater (edit)",
      values: {
        name: "Collector Floater",
        policyNumber: "CF-100",
        insuranceCompany: "Acme Mutual",
        companyContact: "",
        agentName: "Dana",
        agentContact: "",
        notes: "",
        blanketCoverageLimit: "",
        effectiveStartDate: "2026-01-01",
        effectiveEndDate: "2027-01-01",
      },
    });
    renderShell();

    const form = await screen.findByRole("dialog", { name: "Edit Collector Floater" });
    expect(within(form).getByLabelText("Agent name")).toHaveValue("Dana");
  });
});

// specs/006-accessory-links User Story 1, contracts/ui-accessories.md §1.
describe("The Accessories tab (FR-016)", () => {
  beforeEach(() => {
    window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
    getFirearm.mockReset().mockResolvedValue(detail);
    listFirearms.mockReset().mockResolvedValue({ groups: [{ key: "", firearms: [summary] }] });
    getAccessory.mockReset().mockResolvedValue(opticDetail);
    listAccessories.mockReset().mockResolvedValue({
      groups: [{ key: "All", host: null, accessories: [opticSummary] }],
    });
  });

  it("puts the tabs in the order Collection, Accessories, Insurance", () => {
    renderShell();

    const tabs = within(screen.getByRole("navigation", { name: "Sections" }))
      .getAllByRole("button")
      .map((tab) => /^(Collection|Accessories|Insurance)/.exec(tab.textContent ?? "")?.[1]);
    expect(tabs).toEqual(["Collection", "Accessories", "Insurance"]);
  });

  it("opens the Accessories page from its tab, and marks the tab current", async () => {
    const user = userEvent.setup();
    renderShell();

    const tab = within(screen.getByRole("navigation", { name: "Sections" })).getByRole("button", {
      name: /^Accessories/,
    });
    await user.click(tab);

    expect(
      await screen.findByRole("heading", { level: 1, name: "Accessories" }),
    ).toBeInTheDocument();
    expect(tab).toHaveAttribute("aria-current", "page");
    expect(await screen.findByRole("button", { name: OPTIC })).toBeInTheDocument();
  });

  it("opens the add-accessory form from Add accessory", async () => {
    const user = userEvent.setup();
    renderShell();
    await user.click(
      within(screen.getByRole("navigation", { name: "Sections" })).getByRole("button", {
        name: /^Accessories/,
      }),
    );

    await user.click(await screen.findByRole("button", { name: "Add accessory" }));

    const form = await screen.findByRole("dialog", { name: "Add accessory" });
    expect(within(form).getByRole("combobox", { name: /^Kind/ })).toBeInTheDocument();
  });

  it("opens an accessory's record from its name, and Back returns to the list", async () => {
    const user = userEvent.setup();
    renderShell();
    await user.click(
      within(screen.getByRole("navigation", { name: "Sections" })).getByRole("button", {
        name: /^Accessories/,
      }),
    );

    await user.click(await screen.findByRole("button", { name: OPTIC }));
    await screen.findByRole("heading", { level: 1, name: OPTIC });
    expect(getAccessory).toHaveBeenCalledWith(3);
    expect(backLink()).toHaveTextContent("Accessories");
    // The Accessories tab stays current on its records.
    expect(
      within(screen.getByRole("navigation", { name: "Sections" })).getByRole("button", {
        name: /^Accessories/,
      }),
    ).toHaveAttribute("aria-current", "page");

    await user.keyboard("{Escape}");
    expect(
      await screen.findByRole("heading", { level: 1, name: "Accessories" }),
    ).toBeInTheDocument();
  });
});

describe("Resumed accessory pending changes open where their form is (FR-027, FR-039)", () => {
  beforeEach(() => {
    window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
    getFirearm.mockReset().mockResolvedValue(detail);
    listFirearms.mockReset().mockResolvedValue({ groups: [{ key: "", firearms: [summary] }] });
    getAccessory.mockReset().mockResolvedValue(opticDetail);
    listAccessories.mockReset().mockResolvedValue({
      groups: [{ key: "All", host: null, accessories: [opticSummary] }],
    });
  });
  afterEach(() => setResumedDraft(null));

  /** Each accessory form and dialog keeps drafts of version 1, the dispose
   * dialog's of its own. */
  function resume(draft: Omit<Draft, "formVersion" | "label">, label = `${OPTIC} (edit)`) {
    setResumedDraft({
      formVersion: draft.mode === "dispose" ? DISPOSE_FORM_VERSION : 1,
      label,
      ...draft,
    });
  }

  it("a new accessory reopens the add form with the changes", async () => {
    resume(
      { kind: "accessory", mode: "add", targetId: null, values: { make: "Walther" } },
      "New accessory",
    );
    renderShell();

    const form = await screen.findByRole("dialog", { name: "Add accessory" });
    expect(within(form).getByLabelText(/^Make/)).toHaveValue("Walther");
    expect(peekResumedDraft()).toBeNull();
  });

  it("an edit reopens the accessory's edit form on its record, with the changes", async () => {
    resume({
      kind: "accessory",
      mode: "edit",
      targetId: 3,
      values: { notes: "Kept at the lock" },
    });
    renderShell();

    const form = await screen.findByRole("dialog", { name: `Edit ${OPTIC}` });
    expect(within(form).getByLabelText(/^Notes/)).toHaveValue("Kept at the lock");
    expect(getAccessory).toHaveBeenCalledWith(3);
    // On the accessory's record, behind the modal form.
    expect(
      screen.getByRole("heading", { level: 1, name: OPTIC, hidden: true }),
    ).toBeInTheDocument();
    expect(peekResumedDraft()).toBeNull();
  });

  it("a disposal reopens the dispose dialog with the changes", async () => {
    resume(
      {
        kind: "accessory",
        mode: "dispose",
        targetId: 3,
        values: {
          dispositionType: "sold",
          recipient: "Jane Doe",
          date: "2025-06-15",
          price: "800",
        },
      },
      `${OPTIC} (disposal)`,
    );
    renderShell();

    const dialog = await screen.findByRole("dialog", { name: "Mark as disposed" });
    expect(within(dialog).getByLabelText("Transferred to")).toHaveValue("Jane Doe");
    expect(within(dialog).getByLabelText(/^Price received/)).toHaveValue("800");
    expect(dialog).toHaveTextContent(OPTIC);
  });

  it("a restore reopens the restore dialog on a disposed accessory", async () => {
    getAccessory.mockResolvedValue({
      ...opticDetail,
      status: "disposed",
      dispositionType: "sold",
      dispositionRecipient: "Jane Doe",
      dispositionDate: "2025-06-15",
      dispositionPrice: 800,
    });
    resume(
      {
        kind: "accessory",
        mode: "restore",
        targetId: 3,
        values: { history: "keep", renaming: false, nickname: "" },
      },
      `${OPTIC} (restore)`,
    );
    renderShell();

    const dialog = await screen.findByRole("alertdialog", { name: "Restore to the collection?" });
    expect(within(dialog).getByRole("radio", { name: /Keep as history/ })).toBeChecked();
    expect(dialog).toHaveTextContent(OPTIC);
  });

  it("a coverage change reopens the coverage dialog with it", async () => {
    resume(
      {
        kind: "accessory",
        mode: "coverage",
        targetId: 3,
        values: { policyId: String(policy.id), amount: "900" },
      },
      `${OPTIC} (coverage)`,
    );
    renderShell();

    const dialog = await screen.findByRole("dialog", { name: "Insurance coverage" });
    expect(within(dialog).getByLabelText(/Scheduled amount/)).toHaveValue("900");
    expect(dialog).toHaveTextContent(OPTIC);
  });
});

// #59, contracts/ui-databases.md §10: the database's notes head the three list
// pages, not the record pages, and dismissing one hides it on all of them.
describe("The database's notes (#59)", () => {
  const NOTES: DatabaseStatus["notes"] = {
    diskEncryption: true,
    restoredWithPassphraseOf: "2026-09-20T09:00:00Z",
    openedBackup: { backupOfName: "Main collection", madeAt: "2026-09-20T09:00:00Z" },
    damagedFileKeptAt: null,
  };

  /** A session that keeps its notes in state, as the provider's status does. */
  function renderWithNotes() {
    function Harness() {
      const [notes, setNotes] = useState(NOTES);
      const value = {
        status: { ...session.status, notes },
        dismissNote: async (note: "diskEncryption" | "openedBackup" | "restored") =>
          setNotes((current) => ({
            ...current,
            ...(note === "diskEncryption" ? { diskEncryption: false } : {}),
            ...(note === "openedBackup" ? { openedBackup: null } : {}),
            ...(note === "restored" ? { restoredWithPassphraseOf: null } : {}),
          })),
      } as unknown as SessionState;
      return (
        <SessionContext.Provider value={value}>
          <CollectionContext.Provider value={collection}>
            <AppShell />
          </CollectionContext.Provider>
        </SessionContext.Provider>
      );
    }
    render(<Harness />);
  }

  const tab = (name: RegExp) =>
    within(screen.getByRole("navigation", { name: "Sections" })).getByRole("button", { name });
  const noteRegions = () =>
    ["Restored from a backup", "A backup", "Disk encryption"].map((name) =>
      screen.queryByRole("region", { name }),
    );

  beforeEach(() => {
    window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
    getFirearm.mockReset().mockResolvedValue(detail);
    listFirearms.mockReset().mockResolvedValue({ groups: [{ key: "", firearms: [summary] }] });
    getAccessory.mockReset().mockResolvedValue(opticDetail);
    listAccessories.mockReset().mockResolvedValue({
      groups: [{ key: "All", host: null, accessories: [opticSummary] }],
    });
  });

  it("shows all three notes at the top of the Collection, Accessories and Insurance pages", async () => {
    const user = userEvent.setup();
    renderWithNotes();

    await screen.findByRole("heading", { level: 1, name: "Collection" });
    expect(noteRegions().every(Boolean)).toBe(true);

    await user.click(tab(/^Accessories/));
    await screen.findByRole("heading", { level: 1, name: "Accessories" });
    expect(noteRegions().every(Boolean)).toBe(true);

    await user.click(tab(/^Insurance/));
    await screen.findByRole("heading", { level: 1, name: "Insurance" });
    expect(noteRegions().every(Boolean)).toBe(true);
  });

  it("shows none of them on a firearm, accessory or policy page", async () => {
    const user = userEvent.setup();
    renderWithNotes();

    await openColt(user);
    expect(noteRegions().some(Boolean)).toBe(false);

    await user.click(screen.getByRole("button", { name: policy.name }));
    await screen.findByRole("heading", { name: policy.name });
    expect(noteRegions().some(Boolean)).toBe(false);

    await user.click(tab(/^Accessories/));
    await user.click(await screen.findByRole("button", { name: OPTIC }));
    await screen.findByRole("heading", { level: 1, name: OPTIC });
    expect(noteRegions().some(Boolean)).toBe(false);
  });

  it("hides a note on every page once it is dismissed on one", async () => {
    const user = userEvent.setup();
    renderWithNotes();
    await user.click(tab(/^Accessories/));
    await screen.findByRole("heading", { level: 1, name: "Accessories" });

    const disk = screen.getByRole("region", { name: "Disk encryption" });
    await user.click(within(disk).getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("region", { name: "Disk encryption" })).not.toBeInTheDocument();
    // The others stay until each is dismissed.
    expect(screen.getByRole("region", { name: "A backup" })).toBeInTheDocument();

    await user.click(tab(/^Collection/));
    await screen.findByRole("heading", { level: 1, name: "Collection" });
    expect(screen.queryByRole("region", { name: "Disk encryption" })).not.toBeInTheDocument();
    expect(screen.getByRole("region", { name: "A backup" })).toBeInTheDocument();

    await user.click(tab(/^Insurance/));
    await screen.findByRole("heading", { level: 1, name: "Insurance" });
    expect(screen.queryByRole("region", { name: "Disk encryption" })).not.toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Restored from a backup" })).toBeInTheDocument();
  });
});

// 001 US2, the "keeps the search when returning from a record" regression:
// opening a record used to discard the search, grouping and view the user had
// set up. They live in the shell, so the page returned to has them.
describe("The collection's search, grouping and view survive opening a record", () => {
  beforeEach(() => {
    window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
    getFirearm.mockReset().mockResolvedValue(detail);
    listFirearms.mockReset().mockImplementation(async (input: { query?: string }) => ({
      groups: [{ key: ".357", firearms: input.query ? [summary] : [summary, uninsured] }],
    }));
  });
  afterEach(() => localStorage.clear());

  it("returns to the same search, grouping and tiles", async () => {
    const user = userEvent.setup();
    renderShell();
    await user.type(await screen.findByRole("searchbox"), "python");
    await user.click(screen.getByRole("button", { name: "Group by, None" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "Caliber" }));
    await user.click(screen.getByRole("radio", { name: "Tiles" }));
    await waitFor(() =>
      expect(listFirearms).toHaveBeenLastCalledWith(
        expect.objectContaining({ query: "python", groupBy: "caliber" }),
      ),
    );
    await user.click(await screen.findByRole("button", { name: /Colt Python/ }));
    await screen.findByRole("heading", { level: 1, name: "Colt Python" });

    await user.click(backLink() as HTMLElement);

    expect(await screen.findByRole("searchbox")).toHaveValue("python");
    expect(screen.getByRole("button", { name: "Group by, Caliber" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Tiles" })).toBeChecked();
    expect(await screen.findByRole("button", { name: /Colt Python/ })).toBeInTheDocument();
    expect(document.querySelector(".hd-tile")).not.toBeNull();
    expect(screen.queryByRole("button", { name: /Ruger 10\/22/ })).not.toBeInTheDocument();
  });
});

const insuranceTab = () =>
  within(screen.getByRole("navigation", { name: "Sections" })).getByRole("button", {
    name: /^Insurance/,
  });

// 001 ui-review (PR #1 regression): the Insurance tab's number counts the
// active firearms that are uninsured or under-insured, nothing else.
describe("The Insurance tab's count", () => {
  beforeEach(() => {
    window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
    getFirearm.mockReset().mockResolvedValue(detail);
    deleteFirearm.mockReset().mockResolvedValue(undefined);
    listFirearms.mockReset().mockResolvedValue({ groups: [{ key: "", firearms: [summary] }] });
  });

  const underInsured: FirearmSummary = { ...summary, insuranceWarning: "under_insured" };
  // Disposed firearms are left out of coverage checks, whatever their flag.
  const disposed: FirearmSummary = {
    ...uninsured,
    id: 4,
    model: "Mark IV",
    serialNumber: "R4",
    status: "disposed",
  };
  // A policy that is about to lapse is called out on the Insurance page, not
  // counted in the tab.
  const lapsing: InsurancePolicy = { ...policy, id: 8, name: "Lapsing", expiringWarning: true };

  /** The number on the Insurance tab, or 0 when it shows none. */
  const tabCount = () =>
    Number(insuranceTab().querySelector(".hd-tab__alert")?.textContent?.replace(/\D/g, "") ?? 0);

  /** The shell over a collection whose `refresh` drops the deleted firearm,
   * as the real one does after the backend deletes it. */
  function ShellOver({ all }: { all: FirearmSummary[] }) {
    const [firearms, setFirearms] = useState(all);
    const state: CollectionState = {
      ...collection,
      firearms,
      firearmsById: new Map(firearms.map((f) => [f.id, f])),
      policies: [policy, lapsing],
      policiesById: new Map([
        [policy.id, policy],
        [lapsing.id, lapsing],
      ]),
      refresh: async () => setFirearms((list) => list.filter((f) => f.id !== underInsured.id)),
    };
    return (
      <SessionContext.Provider value={session}>
        <CollectionContext.Provider value={state}>
          <AppShell />
        </CollectionContext.Provider>
      </SessionContext.Provider>
    );
  }

  it("counts only active firearms with a warning, and recounts after a delete", async () => {
    const user = userEvent.setup();
    render(<ShellOver all={[underInsured, uninsured, disposed]} />);

    // One under-insured and one uninsured firearm: not the disposed one, and
    // not the policy that is about to lapse.
    expect(tabCount()).toBe(2);

    await openColt(user);
    await user.click(screen.getByRole("button", { name: "Delete" }));
    await user.click(await screen.findByRole("button", { name: "Delete firearm" }));

    await waitFor(() => expect(deleteFirearm).toHaveBeenCalledWith(1, true));
    await waitFor(() => expect(tabCount()).toBe(1));
  });
});

// 001 ui-review (PR #1 regression): a link from one page to another is
// followed by a Back that lands on the page the user came from, and a policy
// opens on its own page.
describe("Following a link to a policy or a firearm, and Back", () => {
  beforeEach(() => {
    window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
    getFirearm.mockReset().mockResolvedValue(detail);
    listFirearms.mockReset().mockResolvedValue({ groups: [{ key: "", firearms: [summary] }] });
  });

  const onePolicyPage = () => {
    expect(document.querySelectorAll(".hd-policy")).toHaveLength(1);
    expect(document.querySelector(".hd-policy__name")).toHaveTextContent(policy.name);
    // It is the policy's own page, not the Insurance listing.
    expect(screen.queryByRole("heading", { level: 1, name: "Insurance" })).not.toBeInTheDocument();
  };

  it("goes policy → firearm → Back to that policy's own page", async () => {
    const user = userEvent.setup();
    renderShell();
    await openColt(user);
    await user.click(screen.getByRole("button", { name: policy.name }));
    await screen.findByRole("heading", { name: policy.name });
    onePolicyPage();
    expect(backLink()).toHaveTextContent("Colt Python");

    const card = document.querySelector<HTMLElement>(".hd-policy")!;
    await user.click(within(card).getByRole("button", { name: "Colt Python" }));
    await screen.findByRole("heading", { level: 1, name: "Colt Python" });
    expect(backLink()).toHaveTextContent(policy.name);

    await user.click(backLink() as HTMLElement);
    await screen.findByRole("heading", { name: policy.name });
    onePolicyPage();
  });

  it("opens a policy's own page from the Insurance list, and Back returns to the list", async () => {
    const user = userEvent.setup();
    renderShell();
    await user.click(insuranceTab());
    expect(screen.getByRole("heading", { level: 1, name: "Insurance" })).toBeInTheDocument();

    await user.click(document.querySelector<HTMLElement>("button.hd-policy__open")!);
    await screen.findByRole("heading", { name: policy.name });
    onePolicyPage();
    // On its own page the name is the heading, not a link to itself.
    expect(document.querySelector(".hd-policy__open")).toBeNull();
    expect(backLink()).toHaveTextContent("Insurance");

    await user.click(backLink() as HTMLElement);
    expect(await screen.findByRole("heading", { level: 1, name: "Insurance" })).toBeInTheDocument();
  });
});

// #108: below 900px the top bar drops the wordmark and shows Import and
// Export as icons, so it fits the window's 800px minimum. The CSS does the
// hiding, so what the names rely on must stay in the markup.
describe("The top bar's narrow form keeps its accessible names (#108)", () => {
  beforeEach(() => {
    listFirearms.mockReset().mockResolvedValue({ groups: [{ key: "", firearms: [summary] }] });
  });

  it("names the brand, Import and Export, with a tooltip on the two buttons", () => {
    renderShell();
    const bar = within(document.querySelector<HTMLElement>(".hd-topbar")!);

    expect(
      bar.getByRole("button", { name: "HoploDex — go to the collection" }),
    ).toBeInTheDocument();
    for (const name of ["Import", "Export"]) {
      const button = bar.getByRole("button", { name });
      expect(button).toHaveAttribute("title", name);
      // The word stays in the page for assistive technology; CSS hides it.
      expect(button.querySelector(".hd-topbar__tool-label")).toHaveTextContent(name);
    }
    // Only the wordmark is dropped when the bar narrows: the owl is not.
    expect(document.querySelector(".hd-brand__name--wide")).toHaveTextContent("HoploDex");
    expect(document.querySelector(".hd-brand .hd-brand__mark")).not.toBeNull();
  });
});
