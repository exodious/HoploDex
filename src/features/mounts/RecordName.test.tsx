import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { NavigationContext } from "../app/navigation";
import type { Navigation } from "../app/navigation";
import { RecordName } from "./RecordName";
import type { RecordLabel } from "./types";

// specs/006-accessory-links FR-005, contracts/ui-accessories.md (rules): a
// record of either kind is named by `RecordName`. A firearm is named as
// `FirearmName` names it (make, model and “nickname”, 001 FR-031); an
// accessory by "{make} {model} · {kind}" (both required, FR-001).

function firearmLabel(overrides: Partial<RecordLabel> = {}): RecordLabel {
  return {
    record: { kind: "firearm", id: 1 },
    make: "Glock",
    model: "19",
    nickname: "Old Faithful",
    typeName: "Handgun",
    serialNumber: "G19-1",
    status: "active",
    ...overrides,
  };
}

function accessoryLabel(overrides: Partial<RecordLabel> = {}): RecordLabel {
  return {
    record: { kind: "accessory", id: 5 },
    make: "Leupold",
    model: "VX-5HD 3-15x44",
    nickname: null,
    typeName: "Optic",
    serialNumber: null,
    status: "active",
    ...overrides,
  };
}

describe("RecordName for a firearm", () => {
  it("renders make, model and nickname as FirearmName does", () => {
    const { container } = render(<RecordName label={firearmLabel()} />);
    expect(container).toHaveTextContent(/^Glock 19 “Old Faithful”$/);
    expect(container.querySelector(".hd-nickname")).toHaveTextContent("“Old Faithful”");
  });

  it("omits the nickname when there is none", () => {
    const { container } = render(<RecordName label={firearmLabel({ nickname: null })} />);
    expect(container).toHaveTextContent(/^Glock 19$/);
  });

  it("adds the type, after a dot, where asked", () => {
    const { container } = render(<RecordName label={firearmLabel()} withType />);
    expect(container).toHaveTextContent(/^Glock 19 “Old Faithful” · Handgun$/);
  });

  it("adds the type to a firearm with no nickname", () => {
    const { container } = render(<RecordName label={firearmLabel({ nickname: null })} withType />);
    expect(container).toHaveTextContent(/^Glock 19 · Handgun$/);
  });
});

describe("RecordName for an accessory (FR-005)", () => {
  it("reads make, model and kind", () => {
    const { container } = render(<RecordName label={accessoryLabel()} />);
    expect(container).toHaveTextContent(/^Leupold VX-5HD 3-15x44 · Optic$/);
  });

  it("reads the same with the type asked for, which an accessory always has", () => {
    const { container } = render(<RecordName label={accessoryLabel()} withType />);
    expect(container).toHaveTextContent(/^Leupold VX-5HD 3-15x44 · Optic$/);
  });

  it("never shows a nickname", () => {
    const { container } = render(
      <RecordName label={accessoryLabel({ nickname: "Should not show" })} />,
    );
    expect(container).not.toHaveTextContent("Should not show");
  });
});

describe("RecordName as a link", () => {
  /** The link, whether it is a button or an anchor. */
  const linkElement = () => document.querySelector<HTMLElement>("button, a")!;

  function renderLinked(label: RecordLabel) {
    const open = vi.fn();
    const navigation: Navigation = {
      route: { page: "collection" },
      navigate: () => {},
      open,
      back: null,
      openDialog: () => {},
    };
    render(
      <NavigationContext.Provider value={navigation}>
        <RecordName label={label} link />
      </NavigationContext.Provider>,
    );
    return open;
  }

  it("is plain text without the link prop", () => {
    render(<RecordName label={accessoryLabel()} />);
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
  });

  it("opens an accessory's record through navigation.open, remembering the page it is on", async () => {
    const user = userEvent.setup();
    const open = renderLinked(accessoryLabel());

    await user.click(linkElement());

    expect(open).toHaveBeenCalledWith({ page: "accessory", id: 5, from: "collection" });
  });

  it("opens a firearm's record through navigation.open", async () => {
    const user = userEvent.setup();
    const open = renderLinked(firearmLabel());

    await user.click(linkElement());

    expect(open).toHaveBeenCalledWith({ page: "firearm", id: 1, from: "collection" });
  });

  it("names the link by the whole name, nickname included", () => {
    renderLinked(firearmLabel());
    expect(linkElement()).toHaveAccessibleName("Glock 19 “Old Faithful”");
  });
});
