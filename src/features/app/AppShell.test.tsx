import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { AccessoryDetail, AccessorySummary } from "../accessories/types";
import type { FirearmSummary } from "../browse/types";
import type { FirearmDetail } from "../firearms/types";
import type { InsurancePolicy } from "../insurance/types";
import { SessionContext } from "../session/sessionStore";
import type { SessionState } from "../session/sessionStore";
import { peekResumedDraft, setResumedDraft } from "../session/usePendingDraft";
import type { Draft } from "../databases/types";
import { FORM_VERSION as FIREARM_FORM_VERSION } from "../firearms/FirearmForm";
import { FORM_VERSION as DISPOSE_FORM_VERSION } from "../firearms/DisposeDialog";
import { AppShell } from "./AppShell";
import { CollectionContext } from "./collectionStore";
import type { CollectionState } from "./collectionStore";
import { ACCESSORY_KINDS, FIREARM_TYPES } from "../../test/collectionFixtures";

const getFirearm = vi.fn();
const listFirearms = vi.fn();
const getAccessory = vi.fn();
const listAccessories = vi.fn();

// The accessory form's make, model, cartridge and caliber use the shared
// entry commands (specs/006-accessory-links FR-003).
vi.mock("../firearms/firearmsService", () => ({
  getFirearm: (id: number) => getFirearm(id),
  settleEntry: async (_field: string, text: string) => ({
    value: text.trim(),
    changedBy: null,
    derivedCaliber: null,
  }),
  suggestEntries: async () => [],
}));
vi.mock("../accessories/accessoriesService", () => ({
  getAccessory: (id: number) => getAccessory(id),
  listAccessories: (input: unknown) => listAccessories(input),
}));
vi.mock("../browse/browseService", () => ({ listFirearms: () => listFirearms() }));
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

function renderShell() {
  render(
    <SessionContext.Provider value={session}>
      <CollectionContext.Provider value={collection}>
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

  it("returns from the Insurance page when a link led there", async () => {
    const user = userEvent.setup();
    renderShell();
    await user.click(await screen.findByRole("button", { name: "Review insurance" }));
    expect(screen.getByRole("heading", { level: 1, name: "Insurance" })).toBeInTheDocument();

    await user.keyboard("{Escape}");
    onCollection();
  });

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
