import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ToastProvider } from "../../components";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { NavigationContext } from "../app/navigation";
import type { Navigation } from "../app/navigation";
import { MountedSection } from "./MountedSection";
import type { MountedEntry, RecordLabel, RecordRef } from "./types";

// specs/006-accessory-links contracts/ui-accessories.md §5 (FR-012, FR-013,
// US2-3a, US2-10): the Mounted section. Its props, as these tests assume
// them: `record: RecordLabel` (the record whose page it is on), `mounted:
// MountedEntry[]` (`MountDetail.mounted`, depth-first) and `onNewAccessory()`
// (the page opens the accessory form with Mounted on preset). It mounts and
// unmounts (after asking) through `mountRecord` and refreshes the collection
// afterwards.

const mountRecord = vi.fn();
const listMountCandidates = vi.fn();

vi.mock("./mountsService", () => ({
  mountRecord: (item: RecordRef, host: RecordRef | null) => mountRecord(item, host),
  listMountCandidates: (input: unknown) => listMountCandidates(input),
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

// The record whose page this is, and what is mounted on it, as in §5's
// picture: a nested outline.
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
          <MountedSection record={record} mounted={mounted} onNewAccessory={onNewAccessory} />
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

async function openExistingDialog(user: ReturnType<typeof userEvent.setup>) {
  await user.click(mountMenu());
  await user.click(await screen.findByRole("menuitem", { name: "Existing accessory or firearm…" }));
  return screen.findByRole("dialog", { name: /^Mount on / });
}

beforeEach(() => {
  mountRecord.mockReset().mockResolvedValue({ item: upper, host: rifle });
  listMountCandidates.mockReset().mockResolvedValue({ candidates: [] });
  refresh.mockReset().mockResolvedValue(undefined);
  open.mockReset();
  onNewAccessory.mockReset();
});

describe("MountedSection list (§5)", () => {
  it("is a ul with one li per entry, depth-first", () => {
    renderSection();

    const list = screen.getByRole("list");
    expect(list.tagName).toBe("UL");
    const items = within(list).getAllByRole("listitem");
    expect(items).toHaveLength(5);
    const order = [NAMES.upper, NAMES.scope, NAMES.redDot, NAMES.light, NAMES.suppressor];
    order.forEach((name, index) => expect(items[index]).toHaveTextContent(name));
  });

  it("names a firearm's entry with its nickname and type, as an accessory's reads by FR-005", () => {
    renderSection();

    const items = within(screen.getByRole("list")).getAllByRole("listitem");
    expect(items[4]).toHaveTextContent("Gemtech GM-45 “Quiet one” · Suppressor");
    expect(items[0]).toHaveTextContent("BCM upper · Upper receiver");
  });

  it("gives each direct entry an Unmount button named 'Unmount {name}'", () => {
    renderSection();

    expect(screen.getByRole("button", { name: `Unmount ${NAMES.upper}` })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /^Unmount Gemtech GM-45 “Quiet one”/ }),
    ).toBeInTheDocument();
  });

  it("gives deeper entries no Unmount button: only what is directly on this record can be unmounted here", () => {
    renderSection();

    expect(screen.getAllByRole("button", { name: /^Unmount/ })).toHaveLength(2);
    expect(screen.queryByRole("button", { name: /^Unmount Leupold/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Unmount Trijicon/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Unmount SureFire/ })).not.toBeInTheDocument();
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

  it("says 'Nothing mounted.' beside the Mount control when empty (US2-10)", () => {
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

describe("MountedSection Mount menu (§5, FR-012)", () => {
  it("offers New accessory… and Existing accessory or firearm…", async () => {
    const user = userEvent.setup();
    renderSection();

    await user.click(mountMenu());

    const items = (await screen.findAllByRole("menuitem")).map((item) => item.textContent?.trim());
    expect(items).toEqual(["New accessory…", "Existing accessory or firearm…"]);
  });

  it("leaves New accessory… to the page, which opens the form with Mounted on preset", async () => {
    const user = userEvent.setup();
    renderSection();

    await user.click(mountMenu());
    await user.click(await screen.findByRole("menuitem", { name: "New accessory…" }));

    expect(onNewAccessory).toHaveBeenCalledTimes(1);
  });

  it("opens a dialog titled 'Mount on {name}' from Existing accessory or firearm…", async () => {
    const user = userEvent.setup();
    renderSection();

    const dialog = await openExistingDialog(user);

    expect(dialog).toHaveAccessibleName(`Mount on ${NAMES.rifle}`);
    expect(within(dialog).getByRole("combobox")).toBeInTheDocument();
  });

  it("searches the records that can be mounted here, with role item and this record", async () => {
    const user = userEvent.setup();
    renderSection();

    const dialog = await openExistingDialog(user);
    await user.click(within(dialog).getByRole("combobox"));
    await user.type(within(dialog).getByRole("combobox"), "aim");

    await waitFor(() =>
      expect(listMountCandidates).toHaveBeenLastCalledWith(
        expect.objectContaining({ role: "item", record: rifle.record, query: "aim" }),
      ),
    );
  });
});

describe("MountedSection mounting an existing record (FR-012, US2-3a)", () => {
  const aimpoint = label({ kind: "accessory", id: 30 }, "Aimpoint", "T-2", "Optic");
  const belongsToAr = label({ kind: "accessory", id: 31 }, "Warne", "rail", "Mount or rail");
  const AIMPOINT = "Aimpoint T-2 · Optic";

  async function pick(user: ReturnType<typeof userEvent.setup>, text: RegExp) {
    const dialog = await openExistingDialog(user);
    await user.click(within(dialog).getByRole("combobox"));
    await user.click(await screen.findByRole("option", { name: text }));
  }

  it("mounts an unmounted record at once, with the toast '{name} mounted on {this record}.'", async () => {
    const user = userEvent.setup();
    listMountCandidates.mockResolvedValue({
      candidates: [{ label: aimpoint, mountedOn: null }],
    });
    mountRecord.mockResolvedValue({ item: aimpoint, host: rifle });
    renderSection();

    await pick(user, /Aimpoint/);

    await waitFor(() => expect(mountRecord).toHaveBeenCalledTimes(1));
    expect(mountRecord).toHaveBeenCalledWith(aimpoint.record, rifle.record);
    // Nothing is asked first.
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(await screen.findByText(`${AIMPOINT} mounted on ${NAMES.rifle}.`)).toBeInTheDocument();
    expect(refresh).toHaveBeenCalled();
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: /^Mount on / })).not.toBeInTheDocument(),
    );
  });

  describe("a record already mounted elsewhere", () => {
    beforeEach(() => {
      listMountCandidates.mockResolvedValue({
        candidates: [{ label: aimpoint, mountedOn: belongsToAr }],
      });
      mountRecord.mockResolvedValue({ item: aimpoint, host: rifle });
    });

    it("lists it with 'Mounted on {its host}'", async () => {
      const user = userEvent.setup();
      renderSection();

      const dialog = await openExistingDialog(user);
      await user.click(within(dialog).getByRole("combobox"));

      expect(await screen.findByRole("option", { name: /Aimpoint/ })).toHaveTextContent(
        "Mounted on Warne rail · Mount or rail",
      );
    });

    it("first asks 'Move {name}?', naming where it is, and mounts nothing yet", async () => {
      const user = userEvent.setup();
      renderSection();

      await pick(user, /Aimpoint/);

      const ask = await screen.findByRole("alertdialog", { name: `Move ${AIMPOINT}?` });
      expect(ask).toHaveAccessibleDescription(
        "It is mounted on Warne rail · Mount or rail. Moving it takes everything mounted on it along.",
      );
      expect(within(ask).getByRole("button", { name: "Move" })).toBeInTheDocument();
      expect(within(ask).getByRole("button", { name: "Cancel" })).toBeInTheDocument();
      expect(mountRecord).not.toHaveBeenCalled();
    });

    it("moves it on Move, with the mounted toast", async () => {
      const user = userEvent.setup();
      renderSection();

      await pick(user, /Aimpoint/);
      const ask = await screen.findByRole("alertdialog", { name: `Move ${AIMPOINT}?` });
      await user.click(within(ask).getByRole("button", { name: "Move" }));

      await waitFor(() => expect(mountRecord).toHaveBeenCalledTimes(1));
      expect(mountRecord).toHaveBeenCalledWith(aimpoint.record, rifle.record);
      expect(await screen.findByText(`${AIMPOINT} mounted on ${NAMES.rifle}.`)).toBeInTheDocument();
      expect(refresh).toHaveBeenCalled();
    });

    it("mounts nothing on Cancel, leaving it where it is (US2-3a)", async () => {
      const user = userEvent.setup();
      renderSection();

      await pick(user, /Aimpoint/);
      const ask = await screen.findByRole("alertdialog", { name: `Move ${AIMPOINT}?` });
      await user.click(within(ask).getByRole("button", { name: "Cancel" }));

      await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
      expect(mountRecord).not.toHaveBeenCalled();
      expect(refresh).not.toHaveBeenCalled();
    });
  });
});

describe("MountedSection Unmount (§5)", () => {
  it("first asks 'Unmount {name}?', saying what stays mounted on it, and unmounts nothing yet", async () => {
    const user = userEvent.setup();
    renderSection();

    await user.click(screen.getByRole("button", { name: `Unmount ${NAMES.upper}` }));

    const ask = await screen.findByRole("alertdialog", { name: `Unmount ${NAMES.upper}?` });
    expect(ask).toHaveAccessibleDescription(
      `It will no longer be mounted on ${NAMES.rifle}. Everything mounted on it stays mounted on it.`,
    );
    expect(within(ask).getByRole("button", { name: "Unmount" })).toBeInTheDocument();
    expect(within(ask).getByRole("button", { name: "Cancel" })).toBeInTheDocument();
    expect(ask.textContent).not.toMatch(/\b(items?|hosts?)\b/i);
    expect(mountRecord).not.toHaveBeenCalled();
  });

  it("says only where it is mounted when nothing is mounted on it", async () => {
    const user = userEvent.setup();
    renderSection();

    await user.click(screen.getByRole("button", { name: /^Unmount Gemtech/ }));

    const ask = await screen.findByRole("alertdialog", { name: `Unmount ${NAMES.suppressor}?` });
    expect(ask).toHaveAccessibleDescription(`It will no longer be mounted on ${NAMES.rifle}.`);
  });

  it("unmounts on Unmount and toasts '{name} unmounted.'", async () => {
    const user = userEvent.setup();
    mountRecord.mockResolvedValue({ item: upper, host: null });
    renderSection();

    await user.click(screen.getByRole("button", { name: `Unmount ${NAMES.upper}` }));
    const ask = await screen.findByRole("alertdialog", { name: `Unmount ${NAMES.upper}?` });
    await user.click(within(ask).getByRole("button", { name: "Unmount" }));

    await waitFor(() => expect(mountRecord).toHaveBeenCalledTimes(1));
    expect(mountRecord).toHaveBeenCalledWith(upper.record, null);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(await screen.findByText(`${NAMES.upper} unmounted.`)).toBeInTheDocument();
    expect(refresh).toHaveBeenCalled();
  });

  it("unmounts a firearm entry the same way", async () => {
    const user = userEvent.setup();
    mountRecord.mockResolvedValue({ item: suppressor, host: null });
    renderSection();

    await user.click(screen.getByRole("button", { name: /^Unmount Gemtech/ }));
    const ask = await screen.findByRole("alertdialog", { name: `Unmount ${NAMES.suppressor}?` });
    await user.click(within(ask).getByRole("button", { name: "Unmount" }));

    await waitFor(() => expect(mountRecord).toHaveBeenCalledWith(suppressor.record, null));
    expect(await screen.findByText("Gemtech GM-45 “Quiet one” unmounted.")).toBeInTheDocument();
  });

  it("unmounts nothing on Cancel, leaving it mounted", async () => {
    const user = userEvent.setup();
    renderSection();

    await user.click(screen.getByRole("button", { name: `Unmount ${NAMES.upper}` }));
    const ask = await screen.findByRole("alertdialog", { name: `Unmount ${NAMES.upper}?` });
    await user.click(within(ask).getByRole("button", { name: "Cancel" }));

    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(mountRecord).not.toHaveBeenCalled();
    expect(refresh).not.toHaveBeenCalled();
  });
});

describe("MountedSection wording (FR-012, constitution III)", () => {
  it("never says 'item' or 'host', in the section, the menu or the dialog", async () => {
    const user = userEvent.setup();
    listMountCandidates.mockResolvedValue({
      candidates: [
        {
          label: label({ kind: "accessory", id: 30 }, "Aimpoint", "T-2", "Optic"),
          mountedOn: upper,
        },
      ],
    });
    renderSection();
    expect(document.body.textContent).not.toMatch(/\b(items?|hosts?)\b/i);

    await user.click(mountMenu());
    await screen.findAllByRole("menuitem");
    expect(document.body.textContent).not.toMatch(/\b(items?|hosts?)\b/i);

    await user.click(screen.getByRole("menuitem", { name: "Existing accessory or firearm…" }));
    const dialog = await screen.findByRole("dialog", { name: /^Mount on / });
    await user.click(within(dialog).getByRole("combobox"));
    await screen.findByRole("option", { name: /Aimpoint/ });
    expect(document.body.textContent).not.toMatch(/\b(items?|hosts?)\b/i);

    await user.click(screen.getByRole("option", { name: /Aimpoint/ }));
    await screen.findByRole("alertdialog", { name: /^Move / });
    expect(document.body.textContent).not.toMatch(/\b(items?|hosts?)\b/i);
  });
});
