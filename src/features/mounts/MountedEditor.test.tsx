import { beforeEach, describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MountedEditor } from "./MountedEditor";
import { NO_MOUNTED_CHANGES, mountChangesInput, readMountedChanges } from "./mountedChanges";
import type { MountedChanges } from "./mountedChanges";
import type { RecordLabel, RecordRef } from "./types";
import { render } from "../../test/sessionScope";

// 014 US5, contracts/ui.md §5: the edit form's Mounted list. Nothing here
// saves: the form sends the list's changes with its Save (014 FR-027).

const listMountCandidates = vi.fn();
vi.mock("./mountsService", () => ({
  listMountCandidates: (_scope: unknown, input: unknown) => listMountCandidates(input),
}));

function label(record: RecordRef, make: string, model: string, typeName: string): RecordLabel {
  return { record, make, model, nickname: null, typeName, serialNumber: null, status: "active" };
}

const rifle = label({ kind: "firearm", id: 1 }, "Ruger", "American", "Rifle");
const pistol = label({ kind: "firearm", id: 2 }, "Glock", "19", "Handgun");
const scope = label({ kind: "accessory", id: 11 }, "Leupold", "VX-3HD", "Optic");
const redDot = label({ kind: "accessory", id: 12 }, "Trijicon", "RMR", "Optic");

let latest: MountedChanges = NO_MOUNTED_CHANGES;

function Harness({ mountedOn = null }: { mountedOn?: RecordLabel | null }) {
  const [changes, setChanges] = useState<MountedChanges>(NO_MOUNTED_CHANGES);
  latest = changes;
  return (
    <MountedEditor
      record={rifle.record}
      saved={[scope]}
      changes={changes}
      onChange={setChanges}
      mountedOn={mountedOn}
    />
  );
}

beforeEach(() => {
  listMountCandidates.mockReset().mockResolvedValue({
    candidates: [{ label: redDot, mountedOn: pistol }],
  });
  latest = NO_MOUNTED_CHANGES;
});

const addField = () => screen.getByRole("combobox", { name: /^Add to Mounted/ });

describe("014 US5 - Change What Is Mounted, and Save It with the Record (the form)", () => {
  it("lists what is mounted, with a Remove each and a way to add (014 US5-2)", () => {
    render(<Harness />);
    expect(screen.getByText("Leupold VX-3HD · Optic")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Remove Leupold VX-3HD · Optic" }),
    ).toBeInTheDocument();
    expect(addField()).toBeInTheDocument();
    expect(screen.queryByText("Changed")).not.toBeInTheDocument();
  });

  it("marks a removal, says it will be unmounted, and asks and saves nothing (014 US5-3)", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: /^Remove/ }));
    expect(screen.getByText("Will be unmounted.")).toBeInTheDocument();
    expect(screen.getByText("Changed")).toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(mountChangesInput(latest, [scope])).toEqual([{ item: scope.record, mounted: false }]);
  });

  it("adds one mounted elsewhere, saying it will move, and offers only what fits (014 US5-4, US5-8)", async () => {
    const user = userEvent.setup();
    render(<Harness mountedOn={pistol} />);
    await user.click(addField());
    await user.click(await screen.findByRole("option", { name: /Trijicon RMR/ }));
    expect(
      screen.getByText("Will be mounted here, moving from Glock 19 with everything mounted on it."),
    ).toBeInTheDocument();
    expect(listMountCandidates).toHaveBeenCalledWith(
      expect.objectContaining({
        role: "item",
        record: rifle.record,
        hostMountedOn: { on: pistol.record },
      }),
    );
    expect(mountChangesInput(latest, [scope])).toEqual([{ item: redDot.record, mounted: true }]);
  });

  it("reverts each row on its own, and a round trip is no change (014 US5-5, US5-6)", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: /^Remove/ }));
    await user.click(
      screen.getByRole("button", { name: "Revert Leupold VX-3HD · Optic to “Mounted here”" }),
    );
    expect(screen.queryByText("Will be unmounted.")).not.toBeInTheDocument();
    expect(latest).toEqual(NO_MOUNTED_CHANGES);

    await user.click(addField());
    await user.click(await screen.findByRole("option", { name: /Trijicon RMR/ }));
    await user.click(
      screen.getByRole("button", { name: "Revert Trijicon RMR · Optic to “Not mounted here”" }),
    );
    expect(screen.queryByText(/Will be mounted here/)).not.toBeInTheDocument();
    expect(latest).toEqual(NO_MOUNTED_CHANGES);
    await waitFor(() => expect(addField()).toHaveFocus());
  });

  it("reads a kept draft's list, and drops one of another shape (014 Edge Cases)", () => {
    expect(
      readMountedChanges({ adds: [{ label: redDot, from: null }], removes: ["accessory:11"] }),
    ).toEqual({ adds: [{ label: redDot, from: null }], removes: ["accessory:11"] });
    expect(readMountedChanges({ adds: "nope", removes: [] })).toEqual(NO_MOUNTED_CHANGES);
    expect(readMountedChanges(undefined)).toEqual(NO_MOUNTED_CHANGES);
  });
});
