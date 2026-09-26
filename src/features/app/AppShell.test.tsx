import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { FirearmSummary } from "../browse/types";
import type { FirearmDetail } from "../firearms/types";
import type { InsurancePolicy } from "../insurance/types";
import { SessionContext } from "../session/sessionStore";
import type { SessionState } from "../session/sessionStore";
import { AppShell } from "./AppShell";
import { CollectionContext } from "./collectionStore";
import type { CollectionState } from "./collectionStore";

const getFirearm = vi.fn();
const listFirearms = vi.fn();

vi.mock("../firearms/firearmsService", () => ({ getFirearm: (id: number) => getFirearm(id) }));
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
  firearmTypeName: "Handgun",
  status: "active",
  thumbnailPhotoId: null,
  genericThumbnailKey: "handgun",
  estimatedValue: 1250,
  insuranceWarning: "none",
  insurancePolicyId: policy.id,
  scheduledCoverageAmount: 1250,
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
  createdAt: "2025-01-01 00:00:00",
  updatedAt: "2025-01-01 00:00:00",
  dispositionHistory: [],
};

const collection: CollectionState = {
  firearms: [summary, uninsured],
  firearmsById: new Map([
    [summary.id, summary],
    [uninsured.id, uninsured],
  ]),
  summary: null,
  policies: [policy],
  policiesById: new Map([[policy.id, policy]]),
  loaded: true,
  error: null,
  revision: 1,
  refresh: async () => {},
};

// An open database whose notes were all dismissed.
const session = {
  status: { notes: { diskEncryption: false } },
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
