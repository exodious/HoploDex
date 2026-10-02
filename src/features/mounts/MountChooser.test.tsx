import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { MountChooser } from "./MountChooser";
import type { RecordLabel, RecordRef } from "./types";

// specs/006-accessory-links contracts/ui-accessories.md §4 (FR-010, FR-012):
// `MountChooser` is a `Combobox` field labelled "Mounted on" that searches
// `list_mount_candidates`. Its props, as these tests assume them:
// `value: RecordLabel | null` (what is chosen), `onChange(label | null)`,
// `record: RecordRef | null` (the record being placed, null for a new one),
// `role?: "host" | "item"` (default "host"), and the field's `hint` and `error`.

const listMountCandidates = vi.fn();

vi.mock("./mountsService", () => ({
  listMountCandidates: (input: unknown) => listMountCandidates(input),
}));

const deerRifle: RecordLabel = {
  record: { kind: "firearm", id: 7 },
  make: "Winchester",
  model: "Model 70",
  nickname: "Deer rifle",
  typeName: "Rifle",
  serialNumber: "W70-123",
  status: "active",
};

const upper: RecordLabel = {
  record: { kind: "accessory", id: 11 },
  make: "BCM",
  model: "upper",
  nickname: null,
  typeName: "Upper receiver",
  serialNumber: "U-100",
  status: "active",
};

const optic: RecordLabel = {
  record: { kind: "accessory", id: 12 },
  make: "Leupold",
  model: "Mark 5HD",
  nickname: null,
  typeName: "Optic",
  serialNumber: null,
  status: "active",
};

type Candidate = { label: RecordLabel; mountedOn: RecordLabel | null };

function answer(...candidates: Candidate[]) {
  listMountCandidates.mockResolvedValue({ candidates });
}

function Harness({
  initial = null,
  record = null,
  role,
  onChange,
  placeholder,
  hint,
  error,
}: {
  initial?: RecordLabel | null;
  record?: RecordRef | null;
  role?: "host" | "item";
  onChange?: (label: RecordLabel | null, mountedOn?: RecordLabel | null) => void;
  placeholder?: string;
  hint?: string;
  error?: string;
}) {
  const [value, setValue] = useState<RecordLabel | null>(initial);
  return (
    <MountChooser
      value={value}
      record={record}
      {...(role ? { role } : {})}
      {...(placeholder ? { placeholder } : {})}
      hint={hint}
      error={error}
      onChange={(...args) => {
        setValue(args[0]);
        onChange?.(...args);
      }}
    />
  );
}

const field = () => screen.getByRole("combobox", { name: /^Mounted on/ });

beforeEach(() => {
  listMountCandidates.mockReset();
  answer({ label: deerRifle, mountedOn: null }, { label: upper, mountedOn: null });
});

describe("MountChooser field (§4)", () => {
  it("is a combobox labelled 'Mounted on'", () => {
    render(<Harness />);
    expect(field()).toBeInTheDocument();
  });

  it("reads 'Not mounted' when empty", () => {
    render(<Harness />);
    expect(field()).toHaveValue("");
    expect(field()).toHaveAttribute("placeholder", "Not mounted");
  });

  it("shows what is chosen, by its name", () => {
    render(<Harness initial={deerRifle} />);
    expect(field()).toHaveDisplayValue(/Winchester Model 70 “Deer rifle”/);
  });

  it("shows a chosen accessory by its name, kind included", () => {
    render(<Harness initial={upper} />);
    expect(field()).toHaveDisplayValue(/BCM upper · Upper receiver/);
  });

  it("shows the hint and a field error it is given", () => {
    render(
      <Harness hint="Only if it is mounted." error="Choose an active firearm or accessory." />,
    );
    expect(field()).toHaveAccessibleDescription(expect.stringContaining("Only if it is mounted."));
    expect(screen.getByText("Choose an active firearm or accessory.")).toBeInTheDocument();
  });

  it("gives the listbox the label 'Mounted on'", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.click(field());

    expect(await screen.findByRole("listbox", { name: "Mounted on" })).toBeInTheDocument();
  });
});

describe("MountChooser searching (§4, FR-012)", () => {
  it("asks list_mount_candidates for hosts when the list opens, for a new record", async () => {
    const user = userEvent.setup();
    render(<Harness record={null} />);

    await user.click(field());

    await waitFor(() => expect(listMountCandidates).toHaveBeenCalled());
    expect(listMountCandidates).toHaveBeenLastCalledWith(
      expect.objectContaining({ role: "host", record: null, query: "" }),
    );
  });

  it("passes the record being placed, so the backend leaves it and what is on it out (US2-6, US2-16)", async () => {
    const user = userEvent.setup();
    render(<Harness record={{ kind: "firearm", id: 4 }} />);

    await user.click(field());

    await waitFor(() =>
      expect(listMountCandidates).toHaveBeenLastCalledWith(
        expect.objectContaining({ role: "host", record: { kind: "firearm", id: 4 } }),
      ),
    );
  });

  it("searches what is typed, as the last text typed", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.click(field());
    await user.type(field(), "deer");

    await waitFor(() =>
      expect(listMountCandidates).toHaveBeenLastCalledWith(
        expect.objectContaining({ role: "host", query: "deer" }),
      ),
    );
  });

  it("never lets a slow answer for earlier text replace a fresh one (debounced as EntryField is)", async () => {
    const user = userEvent.setup();
    let resolveSlow: (value: { candidates: Candidate[] }) => void = () => {};
    listMountCandidates.mockImplementation(({ query }: { query: string }) =>
      query === "d"
        ? new Promise((resolve) => {
            resolveSlow = resolve;
          })
        : Promise.resolve({ candidates: [{ label: deerRifle, mountedOn: null }] }),
    );
    render(<Harness />);

    await user.click(field());
    await user.type(field(), "de");
    await screen.findByRole("option", { name: /Deer rifle/ });
    resolveSlow({ candidates: [{ label: optic, mountedOn: null }] });

    await waitFor(() => expect(listMountCandidates.mock.calls.length).toBeGreaterThanOrEqual(3));
    expect(screen.queryByRole("option", { name: /Leupold/ })).not.toBeInTheDocument();
    expect(screen.getByRole("option", { name: /Deer rifle/ })).toBeInTheDocument();
  });

  it("asks for the role it is given", async () => {
    const user = userEvent.setup();
    render(<Harness role="item" record={{ kind: "firearm", id: 1 }} />);

    await user.click(field());

    await waitFor(() =>
      expect(listMountCandidates).toHaveBeenLastCalledWith(
        expect.objectContaining({ role: "item", record: { kind: "firearm", id: 1 } }),
      ),
    );
  });
});

describe("MountChooser options (§4)", () => {
  async function openList() {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(field());
    return { user, options: await screen.findAllByRole("option") };
  }

  it("lists 'Not mounted' first, then each candidate", async () => {
    const { options } = await openList();

    expect(options).toHaveLength(3);
    expect(options[0]).toHaveTextContent(/^Not mounted/);
    expect(options[1]).toHaveTextContent("Winchester Model 70 “Deer rifle”");
    expect(options[2]).toHaveTextContent("BCM upper · Upper receiver");
  });

  it("shows a firearm's name, then its type and serial number in muted text", async () => {
    const { options } = await openList();

    const row = options[1];
    expect(row).toHaveTextContent("Winchester Model 70 “Deer rifle”");
    expect(row).toHaveTextContent("Rifle");
    expect(row).toHaveTextContent("W70-123");
    // The name comes first, then the type, then the serial number.
    const text = row.textContent!;
    expect(text.indexOf("Deer rifle")).toBeLessThan(
      text.indexOf("Rifle", text.indexOf("Deer rifle") + 10),
    );
    expect(text.indexOf("Rifle", text.indexOf("Deer rifle") + 10)).toBeLessThan(
      text.indexOf("W70-123"),
    );
  });

  it("shows an accessory's name and its serial number", async () => {
    const { options } = await openList();

    expect(options[2]).toHaveTextContent("BCM upper · Upper receiver");
    expect(options[2]).toHaveTextContent("U-100");
  });

  it("says 'Mounted on {its host}' on a second line for a candidate that is mounted", async () => {
    answer({ label: optic, mountedOn: upper }, { label: deerRifle, mountedOn: null });
    const { options } = await openList();

    expect(options[1]).toHaveTextContent("Leupold Mark 5HD · Optic");
    expect(options[1]).toHaveTextContent("Mounted on BCM upper · Upper receiver");
    expect(options[2]).not.toHaveTextContent(/Mounted on/);
  });

  it("never says 'item' or 'host'", async () => {
    answer({ label: optic, mountedOn: upper });
    await openList();

    expect(document.body.textContent).not.toMatch(/\b(items?|hosts?)\b/i);
  });
});

describe("MountChooser choosing and clearing (§4, FR-012)", () => {
  it("chooses a candidate by click and shows it", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);

    await user.click(field());
    await user.click(await screen.findByRole("option", { name: /Deer rifle/ }));

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange.mock.calls[0][0]).toEqual(deerRifle);
    expect(field()).toHaveDisplayValue(/Deer rifle/);
  });

  it("asks nothing when the candidate is already mounted elsewhere (FR-012)", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    answer({ label: optic, mountedOn: upper });
    render(<Harness onChange={onChange} />);

    await user.click(field());
    await user.click(await screen.findByRole("option", { name: /Leupold/ }));

    expect(onChange.mock.calls[0][0]).toEqual(optic);
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("passes where the candidate is mounted now as a second argument (FR-012)", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    answer({ label: optic, mountedOn: upper }, { label: deerRifle, mountedOn: null });
    render(<Harness onChange={onChange} />);

    await user.click(field());
    await user.click(await screen.findByRole("option", { name: /Leupold/ }));
    expect(onChange).toHaveBeenLastCalledWith(optic, upper);

    await user.click(field());
    await user.click(await screen.findByRole("option", { name: /Deer rifle/ }));
    expect(onChange).toHaveBeenLastCalledWith(deerRifle, null);
  });

  it("says what the empty field says as its placeholder, 'Not mounted' unless told otherwise", () => {
    const { unmount } = render(<Harness />);
    expect(field()).toHaveAttribute("placeholder", "Not mounted");
    unmount();

    render(<Harness placeholder="Search by make, model, nickname or serial number" />);
    expect(field()).toHaveAttribute(
      "placeholder",
      "Search by make, model, nickname or serial number",
    );
  });

  it("clears with the × button", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness initial={deerRifle} onChange={onChange} />);

    await user.click(screen.getByRole("button", { name: /clear|×/i }));

    expect(onChange).toHaveBeenCalledWith(null);
    expect(field()).toHaveValue("");
    expect(screen.queryByRole("button", { name: /clear|×/i })).not.toBeInTheDocument();
  });

  it("has no × button while nothing is chosen", () => {
    render(<Harness />);
    expect(screen.queryByRole("button", { name: /clear|×/i })).not.toBeInTheDocument();
  });

  it("clears by choosing 'Not mounted' at the top of the list", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness initial={deerRifle} onChange={onChange} />);

    await user.click(field());
    const options = await screen.findAllByRole("option");
    await user.click(options[0]);

    expect(options[0]).toHaveTextContent(/^Not mounted/);
    expect(onChange).toHaveBeenCalledWith(null);
    expect(field()).toHaveValue("");
  });
});

describe("MountChooser keyboard (§4)", () => {
  it("moves with ↓ ↑ and chooses with Enter", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);

    await user.click(field());
    await screen.findAllByRole("option");
    // Nothing is highlighted at first: ↓ reaches 'Not mounted', then the first
    // candidate, then the second.
    await user.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}");
    expect(field()).toHaveAttribute("aria-activedescendant", screen.getAllByRole("option")[2].id);
    await user.keyboard("{ArrowUp}{Enter}");

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange.mock.calls[0][0]).toEqual(deerRifle);
  });

  it("closes the list on Esc and keeps what is typed", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.click(field());
    await user.type(field(), "bcm");
    await screen.findAllByRole("option");
    await user.keyboard("{Escape}");

    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(field()).toHaveValue("bcm");
  });

  it("opens the list with ↓ from the closed field", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    field().focus();
    await user.keyboard("{ArrowDown}");

    expect(await screen.findByRole("listbox", { name: "Mounted on" })).toBeInTheDocument();
  });
});
