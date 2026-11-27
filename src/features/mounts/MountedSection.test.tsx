import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ToastProvider } from "../../components";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { NavigationContext } from "../app/navigation";
import type { Navigation } from "../app/navigation";
import { MountedSection } from "./MountedSection";
import type { MountedEntry, RecordLabel, RecordRef } from "./types";
import { render } from "../../test/sessionScope";

// 006 contracts/ui-accessories.md §5 (006 FR-012, FR-013, US2-10), as 014
// FR-028 amends it: the Mounted section lists what is mounted, read-only.
// Mounting and unmounting an existing record happen in the edit form's
// Mounted list (MountedEditor.test.tsx); nothing here calls `mountRecord`.

const mountRecord = vi.fn();
const listMountCandidates = vi.fn();

vi.mock("./mountsService", () => ({
  mountRecord: (_scope: unknown, item: RecordRef, host: RecordRef | null) =>
    mountRecord(item, host),
  listMountCandidates: (_scope: unknown, input: unknown) => listMountCandidates(input),
}));

function label(
  record: RecordRef,
  make: string,
  model: string,
  typeName: string,
  extra: Partial<RecordLabel> = {},
): RecordLabel {
  return {
    record,
    make,
    model,
    nickname: null,
    typeName,
    serialNumber: null,
    status: "active",
    ...extra,
  };
}

// The record whose page this is, and what is mounted on it, as in
// 006 contracts/ui-accessories.md §5's picture: a nested outline.
const rifle = label({ kind: "firearm", id: 1 }, "LaRue", "PredatAR", "Rifle");
const upper = label({ kind: "accessory", id: 11 }, "BCM", "upper", "Upper receiver");
const scope = label({ kind: "accessory", id: 12 }, "Leupold", "Mark 5HD 5-25x56", "Optic");
const redDot = label({ kind: "accessory", id: 13 }, "Trijicon", "RMR", "Optic");
const light = label({ kind: "accessory", id: 14 }, "SureFire", "M600", "Light or laser");
const suppressor = label({ kind: "firearm", id: 20 }, "Gemtech", "GM-45", "Suppressor", {
  nickname: "Quiet one",
});

const entries: MountedEntry[] = [
  { label: upper, host: rifle.record, depth: 1 },
  { label: scope, host: upper.record, depth: 2 },
  { label: redDot, host: scope.record, depth: 3 },
  { label: light, host: upper.record, depth: 2 },
  { label: suppressor, host: rifle.record, depth: 1 },
];

const NAMES = {
  rifle: "LaRue PredatAR",
  upper: "BCM upper · Upper receiver",
  scope: "Leupold Mark 5HD 5-25x56 · Optic",
  redDot: "Trijicon RMR · Optic",
  light: "SureFire M600 · Light or laser",
  suppressor: "Gemtech GM-45 “Quiet one”",
};

const refresh = vi.fn();
const open = vi.fn();
const onNewAccessory = vi.fn();

const collection = { refresh } as unknown as CollectionState;

function renderSection(mounted: MountedEntry[] = entries, record: RecordLabel = rifle) {
  const navigation: Navigation = {
    route: { page: "firearm", id: 1, from: "collection" },
    navigate: () => {},
    open,
    back: null,
    openDialog: () => {},
  };
  return render(
    <ToastProvider>
      <CollectionContext.Provider value={collection}>
        <NavigationContext.Provider value={navigation}>
          <MountedSection
            record={record}
            mounted={mounted}
            onNewAccessory={onNewAccessory}
            onChangeMounted={onChangeMounted}
          />
        </NavigationContext.Provider>
      </CollectionContext.Provider>
    </ToastProvider>,
  );
}

/** A link to a record, whether rendered as a button or an anchor. */
function recordLink(name: string): HTMLElement {
  const link = screen.queryByRole("link", { name }) ?? screen.queryByRole("button", { name });
  if (!link) throw new Error(`no link named ${name}`);
  return link;
}

const mountMenu = () => screen.getByRole("button", { name: /^Mount/ });

const onChangeMounted = vi.fn();

beforeEach(() => {
  refresh.mockReset().mockResolvedValue(undefined);
  open.mockReset();
  onNewAccessory.mockReset();
  onChangeMounted.mockReset();
});

describe("MountedSection list (006 contracts/ui-accessories.md §5)", () => {
  it("is a ul with one li per entry, depth-first", () => {
    renderSection();

    const list = screen.getByRole("list");
    expect(list.tagName).toBe("UL");
    const items = within(list).getAllByRole("listitem");
    expect(items).toHaveLength(5);
    const order = [NAMES.upper, NAMES.scope, NAMES.redDot, NAMES.light, NAMES.suppressor];
    order.forEach((name, index) => expect(items[index]).toHaveTextContent(name));
  });

  it("names a firearm's entry with its nickname and type, as an accessory's reads by 006 FR-005", () => {
    renderSection();

    const items = within(screen.getByRole("list")).getAllByRole("listitem");
    expect(items[4]).toHaveTextContent("Gemtech GM-45 “Quiet one” · Suppressor");
    expect(items[0]).toHaveTextContent("BCM upper · Upper receiver");
  });

  it("says 'on {host}' under a deeper entry, and nothing under a direct one", () => {
    renderSection();

    const items = within(screen.getByRole("list")).getAllByRole("listitem");
    expect(items[1]).toHaveTextContent(`on ${NAMES.upper}`);
    expect(items[2]).toHaveTextContent(`on ${NAMES.scope}`);
    expect(items[3]).toHaveTextContent(`on ${NAMES.upper}`);
    expect(items[0]).not.toHaveTextContent(`on ${NAMES.rifle}`);
    expect(items[4]).not.toHaveTextContent(`on ${NAMES.rifle}`);
  });

  it("makes the 'on …' line the link's accessible description, so a screen reader hears it", () => {
    renderSection();

    expect(recordLink(NAMES.redDot)).toHaveAccessibleDescription(`on ${NAMES.scope}`);
    expect(recordLink(NAMES.scope)).toHaveAccessibleDescription(`on ${NAMES.upper}`);
    expect(recordLink(NAMES.redDot)).toHaveAttribute("aria-describedby");
  });

  it("makes every name a link to its record, through navigation.open", async () => {
    const user = userEvent.setup();
    renderSection();

    for (const name of Object.values(NAMES).filter((n) => n !== NAMES.rifle)) {
      const link =
        screen.queryByRole("link", { name: new RegExp(`^${name}`) }) ??
        screen.queryByRole("button", { name: new RegExp(`^${name}`) });
      expect(link, name).not.toBeNull();
    }
    await user.click(recordLink(NAMES.scope));
    expect(open).toHaveBeenLastCalledWith(expect.objectContaining({ page: "accessory", id: 12 }));
    await user.click(screen.getByRole("button", { name: /^Gemtech GM-45 “Quiet one”/ }));
    expect(open).toHaveBeenLastCalledWith(expect.objectContaining({ page: "firearm", id: 20 }));
  });

  it("says 'Nothing mounted.' beside the Mount control when empty (006 US2-10)", () => {
    renderSection([]);

    expect(screen.getByText("Nothing mounted.")).toBeInTheDocument();
    expect(mountMenu()).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Unmount/ })).not.toBeInTheDocument();
  });

  it("does not say 'Nothing mounted.' when something is", () => {
    renderSection();
    expect(screen.queryByText("Nothing mounted.")).not.toBeInTheDocument();
  });
});

describe("014 US5 - Change What Is Mounted, and Save It with the Record (the page)", () => {
  it("lists what is mounted with no Unmount, and changes it in the edit form (014 US5-1, FR-028)", async () => {
    const user = userEvent.setup();
    renderSection();

    expect(screen.queryByRole("button", { name: /^Unmount/ })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Change what's mounted…" }));
    expect(onChangeMounted).toHaveBeenCalledTimes(1);
    expect(mountRecord).not.toHaveBeenCalled();
  });

  it("keeps only New accessory… in the Mount menu, left to the page (014 US5-10)", async () => {
    const user = userEvent.setup();
    renderSection();

    await user.click(mountMenu());
    const items = (await screen.findAllByRole("menuitem")).map((item) => item.textContent?.trim());
    expect(items).toEqual(["New accessory…"]);
    await user.click(screen.getByRole("menuitem", { name: "New accessory…" }));
    expect(onNewAccessory).toHaveBeenCalledTimes(1);
  });

  it("never says 'item' or 'host' (006 FR-012, constitution III)", async () => {
    const user = userEvent.setup();
    renderSection();
    expect(document.body.textContent).not.toMatch(/\b(items?|hosts?)\b/i);
    await user.click(mountMenu());
    await screen.findAllByRole("menuitem");
    expect(document.body.textContent).not.toMatch(/\b(items?|hosts?)\b/i);
  });
});
