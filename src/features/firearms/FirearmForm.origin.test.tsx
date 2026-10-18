import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  fillRequired,
  originGroupButton,
  physicalGroupButton,
  render,
  renderWithOriginGroup,
  renderWithPhysicalGroup,
  resetServiceMocks,
} from "./firearmFormTestSupport";
import { getDirtyForm } from "../session/usePendingDraft";
import { FORM_VERSION, FirearmForm } from "./FirearmForm";
import { CommandFailure } from "../../services/tauriClient";
import type { Firearm } from "./types";
import { setResumedDraft } from "../../test/sessionScope";

beforeEach(resetServiceMocks);

// 002 contracts/ui-identification.md §1-§3
describe("FirearmForm origin control (002 US1)", () => {
  it("offers Domestic/Imported/Re-imported/Unspecified with their one-line descriptions", () => {
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    expect(
      screen.getByRole("radio", { name: /^Domestic Made in the U\.S\.$/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("radio", { name: /^Imported Made abroad and brought in$/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("radio", {
        name: /^Re-imported Made in the U\.S\., exported, then brought back in$/,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("radio", { name: /^Unspecified Leave this if you're not sure\.$/ }),
    ).toBeInTheDocument();
  });

  it("starts a new record on Unspecified", () => {
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);
    expect(screen.getByRole("radio", { name: /^Unspecified/ })).toBeChecked();
  });

  it("selecting Imported reveals Country of manufacture and Importer, both optional", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Imported/ }));

    expect(screen.getByLabelText("Country of manufacture")).not.toBeRequired();
    expect(screen.getByLabelText("Importer")).not.toBeRequired();
    expect(screen.queryByText("Country of manufacture: United States")).not.toBeInTheDocument();
  });

  it("selecting Re-imported reveals only Importer plus a read-only United States country line", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Re-imported/ }));

    expect(screen.getByLabelText("Importer")).toBeInTheDocument();
    expect(screen.queryByLabelText("Country of manufacture")).not.toBeInTheDocument();
    expect(screen.getByText("Country of manufacture: United States")).toBeInTheDocument();
  });

  it("shows neither field for Domestic or Unspecified", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));
    expect(screen.queryByLabelText("Country of manufacture")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Importer")).not.toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: /^Unspecified/ }));
    expect(screen.queryByLabelText("Country of manufacture")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Importer")).not.toBeInTheDocument();
  });

  it("shows the Domestic cue to consider Re-imported", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    expect(
      screen.queryByText(
        "Made in the U.S. but stamped with an importer's name? Choose Re-imported.",
      ),
    ).not.toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));
    expect(
      screen.getByText("Made in the U.S. but stamped with an importer's name? Choose Re-imported."),
    ).toBeInTheDocument();
  });

  it("shows Year of manufacture for every origin and validates a four-digit range", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderWithOriginGroup(<FirearmForm onSubmit={onSubmit} />);

    const currentYear = new Date().getFullYear();
    const year = screen.getByLabelText("Year of manufacture");
    await user.type(year, "43");
    await user.tab();

    expect(
      screen.getByText(
        `Year of manufacture must be a four-digit year from 1400 to ${currentYear}.`,
      ),
    ).toBeInTheDocument();

    await fillRequired(user);
    await user.click(screen.getByRole("button", { name: "Add firearm" }));
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("accepts a valid year and submits it as a number", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderWithOriginGroup(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(screen.getByLabelText("Year of manufacture"), "1943");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit.mock.calls[0][0].yearOfManufacture).toBe(1943);
  });

  it("asks before discarding importer and country when moving away from an import-marked origin", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Imported/ }));
    await user.type(screen.getByLabelText("Country of manufacture"), "Belgium");
    await user.type(screen.getByLabelText("Importer"), "Global Arms Import Co.");

    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));

    expect(screen.getByRole("alertdialog")).toBeInTheDocument();
    expect(screen.getByText("Discard importer and original marks?")).toBeInTheDocument();
    // Cancelling keeps everything, including the origin.
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getByRole("radio", { name: /^Imported/ })).toBeChecked();
    expect(screen.getByLabelText("Country of manufacture")).toHaveValue("Belgium");

    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));
    await user.click(screen.getByRole("button", { name: "Discard and change" }));
    expect(screen.getByRole("radio", { name: /^Domestic/ })).toBeChecked();
    expect(screen.queryByLabelText("Country of manufacture")).not.toBeInTheDocument();
  });

  it("asks before discarding only the country when moving from Imported to Re-imported", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Imported/ }));
    await user.type(screen.getByLabelText("Country of manufacture"), "Belgium");
    await user.type(screen.getByLabelText("Importer"), "Global Arms Import Co.");

    await user.click(screen.getByRole("radio", { name: /^Re-imported/ }));

    expect(screen.getByText("Discard the country of manufacture?")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Discard and change" }));

    expect(screen.getByRole("radio", { name: /^Re-imported/ })).toBeChecked();
    // Importer carries over.
    expect(screen.getByLabelText("Importer")).toHaveValue("Global Arms Import Co.");
  });

  it("does not ask when nothing would be lost", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Imported/ }));
    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));

    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.getByRole("radio", { name: /^Domestic/ })).toBeChecked();
  });

  // 002 SC-007, US1-7 (the guide opens from the form)
  it("opens the origin guide from the How do I record this? button", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "How do I record this?" }));
    expect(
      screen.getByRole("dialog", {
        name: "How to record where a firearm came from and how it's registered",
      }),
    ).toBeInTheDocument();
  });
});

// 002 contracts/ui-identification.md §2, US2
describe("FirearmForm original maker's marks (002 US2)", () => {
  const group = () => screen.getByRole("group", { name: "Original maker's marks" });

  it("shows the fieldset for an import-marked origin, with a hint and all three fields optional", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Imported/ }));

    expect(
      screen.getByText(
        "Only if the original maker's marks differ from the make, model and serial number above, or you want both.",
      ),
    ).toBeInTheDocument();
    expect(within(group()).getByLabelText("Original maker")).not.toBeRequired();
    expect(within(group()).getByLabelText("Original model")).not.toBeRequired();
    expect(within(group()).getByLabelText("Original serial number")).not.toBeRequired();
  });

  it("shows the fieldset for a re-imported origin too", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Re-imported/ }));

    expect(within(group()).getByLabelText("Original maker")).toBeInTheDocument();
  });

  it("shows no original-marks fields for a domestic or unspecified origin", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    expect(screen.queryByRole("group", { name: "Original maker's marks" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));
    expect(screen.queryByRole("group", { name: "Original maker's marks" })).not.toBeInTheDocument();
  });

  it("submits a partial set with no message", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderWithOriginGroup(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(screen.getByRole("radio", { name: /^Imported/ }));
    await user.type(within(group()).getByLabelText("Original maker"), "Fabrique Nationale");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      originalMake: "Fabrique Nationale",
      originalModel: null,
      originalSerialNumber: null,
    });
  });

  it("submits all three fields, and null when left blank", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderWithOriginGroup(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(screen.getByRole("radio", { name: /^Imported/ }));
    await user.type(within(group()).getByLabelText("Original maker"), "Fabrique Nationale");
    await user.type(within(group()).getByLabelText("Original model"), "High Power");
    await user.type(within(group()).getByLabelText("Original serial number"), "FN-99001");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      originalMake: "Fabrique Nationale",
      originalModel: "High Power",
      originalSerialNumber: "FN-99001",
    });
  });

  it("discards original marks (with importer) when moving away from an import-marked origin", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Imported/ }));
    await user.type(within(group()).getByLabelText("Original maker"), "Fabrique Nationale");

    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));
    expect(screen.getByRole("alertdialog")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Discard and change" }));

    expect(screen.queryByRole("group", { name: "Original maker's marks" })).not.toBeInTheDocument();
  });

  it("carries original marks over when moving between Imported and Re-imported", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Imported/ }));
    await user.type(within(group()).getByLabelText("Original maker"), "Fabrique Nationale");

    await user.click(screen.getByRole("radio", { name: /^Re-imported/ }));

    expect(within(group()).getByLabelText("Original maker")).toHaveValue("Fabrique Nationale");
  });
});

describe("FirearmForm original-marks warning (002 US3, FR-009)", () => {
  it("opens a confirm-to-save dialog on ORIGINAL_MARKS_MATCH and resends confirmed on Save anyway", async () => {
    const user = userEvent.setup();
    const message =
      "Ridgeline Arms Hi-Power (serial RA-1) already has these original maker's marks.";
    const onSubmit = vi
      .fn()
      .mockRejectedValueOnce(new CommandFailure({ code: "ORIGINAL_MARKS_MATCH", message }))
      .mockResolvedValueOnce(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(
      await screen.findByText("Another firearm has the same original marks"),
    ).toBeInTheDocument();
    expect(screen.getByText(message)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Save anyway" }));

    expect(onSubmit).toHaveBeenCalledTimes(2);
    expect(onSubmit.mock.calls[1][1]).toBe(true);
  });

  it("cancelling the warning saves nothing", async () => {
    const user = userEvent.setup();
    const message =
      "Ridgeline Arms Hi-Power (serial RA-1) already has these original maker's marks.";
    const onSubmit = vi
      .fn()
      .mockRejectedValueOnce(new CommandFailure({ code: "ORIGINAL_MARKS_MATCH", message }));
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(await screen.findByText(message)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
  });
});

// The origin fields fold into one disclosure so the form stays short for the
// many records that never use them; closing it must never hide a value or an
// error.
describe("FirearmForm origin group disclosure", () => {
  const imported = {
    id: 1,
    make: "Glock",
    model: "17",
    status: "active",
    origin: "imported",
    yearOfManufacture: 1998,
    countryOfManufacture: "Austria",
    importerName: "Glock Inc.",
    originalMake: "Glock GmbH",
  } as Firearm;

  it("starts closed on a new record, saying what it holds", () => {
    render(<FirearmForm onSubmit={vi.fn()} />);

    expect(originGroupButton()).toHaveAttribute("aria-expanded", "false");
    expect(originGroupButton()).toHaveTextContent(
      "Optional: where and when it was made, and who imported it.",
    );
    expect(screen.queryByLabelText("Year of manufacture")).not.toBeInTheDocument();
    expect(screen.queryByRole("radio", { name: /^Domestic/ })).not.toBeInTheDocument();
  });

  it("starts open on a record with any of its fields recorded", () => {
    render(<FirearmForm initialValues={imported} onSubmit={vi.fn()} />);

    expect(originGroupButton()).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByLabelText("Year of manufacture")).toHaveValue("1998");
  });

  it("reads the recorded values back when closed", async () => {
    const user = userEvent.setup();
    render(<FirearmForm initialValues={imported} onSubmit={vi.fn()} />);

    await user.click(originGroupButton());

    expect(originGroupButton()).toHaveAttribute("aria-expanded", "false");
    expect(originGroupButton()).toHaveTextContent(
      "Imported from Austria by Glock Inc. Made in 1998. Original maker's marks recorded.",
    );
  });

  it("still submits values recorded in a closed group", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <FirearmForm
        initialValues={{ ...imported, caliber: "9mm", firearmTypeId: 1, serialNumber: "A1" }}
        onSubmit={onSubmit}
      />,
    );

    await user.click(originGroupButton());
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      origin: "imported",
      yearOfManufacture: 1998,
      importerName: "Glock Inc.",
    });
  });

  it("opens and focuses an invalid year when a save is tried with the group closed", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(originGroupButton());
    await user.type(screen.getByLabelText("Year of manufacture"), "1200");
    await user.click(originGroupButton());
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit).not.toHaveBeenCalled();
    expect(originGroupButton()).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByLabelText("Year of manufacture")).toHaveFocus();
  });

  it("opens on a server error for one of its fields", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message: "The firearm record has validation errors.",
        fieldErrors: { importerName: "Importer is too long." },
      }),
    );
    render(
      <FirearmForm
        initialValues={{ ...imported, caliber: "9mm", firearmTypeId: 1, serialNumber: "A1" }}
        onSubmit={onSubmit}
      />,
    );

    await user.click(originGroupButton());
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    expect(originGroupButton()).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Importer is too long.")).toBeInTheDocument();
    // The collapsed group opened first, so the refused field can take the focus.
    expect(screen.getByLabelText("Importer")).toHaveFocus();
  });

  it("opens and brings Year of manufacture into view on an identity clash with no year", async () => {
    const user = userEvent.setup();
    Element.prototype.scrollIntoView = vi.fn();
    const message =
      "Glock 19 (serial ABC123) already has these marks. Or record a year of manufacture on each firearm: two firearms with the same marks are accepted when both have a year and the years differ.";
    const onSubmit = vi.fn().mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message,
        fieldErrors: { serialNumber: message },
      }),
    );
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(originGroupButton()).toHaveAttribute("aria-expanded", "true");
    expect(Element.prototype.scrollIntoView).toHaveBeenCalledTimes(1);
    expect(vi.mocked(Element.prototype.scrollIntoView).mock.contexts[0]).toHaveAttribute(
      "data-field",
      "yearOfManufacture",
    );
    // The place to act takes the focus.
    expect(screen.getByLabelText("Year of manufacture")).toHaveFocus();
  });
});

describe("FirearmForm physical details disclosure", () => {
  const measured = {
    id: 1,
    make: "Glock",
    model: "17",
    caliber: "9mm",
    firearmTypeId: 1,
    serialNumber: "A1",
    status: "active",
    barrelLengthHundredths: 449,
    overallLengthHundredths: 802,
    weightTenthsOz: 400,
    capacity: 17,
    finish: "nDLC",
    condition: "excellent",
  } as Firearm;

  it("starts closed on a new record, saying what it holds", () => {
    render(<FirearmForm onSubmit={vi.fn()} />);

    expect(physicalGroupButton()).toHaveAttribute("aria-expanded", "false");
    expect(physicalGroupButton()).toHaveTextContent(
      "Optional: lengths, weight, capacity, finish and condition.",
    );
    expect(screen.queryByLabelText("Barrel length (in)")).not.toBeInTheDocument();
  });

  it("starts open on a record with any physical detail recorded", () => {
    render(
      <FirearmForm
        initialValues={
          { id: 1, make: "Colt", model: "Python", status: "active", capacity: 6 } as Firearm
        }
        onSubmit={vi.fn()}
      />,
    );

    expect(physicalGroupButton()).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByLabelText("Capacity")).toHaveValue("6");
  });

  it("says how precisely lengths and weight are saved", () => {
    renderWithPhysicalGroup(<FirearmForm onSubmit={vi.fn()} />);

    expect(screen.getByText("Saved to the nearest 0.01 in.")).toBeInTheDocument();
    expect(
      screen.getByText("Fill in either or both. Saved to the nearest 0.1 oz."),
    ).toBeInTheDocument();
  });

  it("reads the recorded values back when closed", async () => {
    const user = userEvent.setup();
    render(<FirearmForm initialValues={measured} onSubmit={vi.fn()} />);

    await user.click(physicalGroupButton());

    expect(physicalGroupButton()).toHaveAttribute("aria-expanded", "false");
    expect(physicalGroupButton()).toHaveTextContent(
      "4.49 in barrel, 8.02 in overall. 2 lb 8 oz. 17 rounds. Finish: nDLC. Condition: Excellent.",
    );
  });

  it("still submits values recorded in a closed group", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm initialValues={measured} onSubmit={onSubmit} />);

    await user.click(physicalGroupButton());
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      barrelLengthHundredths: 449,
      weightTenthsOz: 400,
      capacity: 17,
      condition: "excellent",
    });
  });

  it("opens and focuses an invalid field when a save is tried with the group closed", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(physicalGroupButton());
    await user.type(screen.getByLabelText("Capacity"), "0");
    await user.click(physicalGroupButton());
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit).not.toHaveBeenCalled();
    expect(physicalGroupButton()).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByLabelText("Capacity")).toHaveFocus();
  });

  it("opens on a server error for one of its fields", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message: "The firearm record has validation errors.",
        fieldErrors: { weightTenthsOz: "Weight must be greater than 0." },
      }),
    );
    render(<FirearmForm initialValues={measured} onSubmit={onSubmit} />);

    await user.click(physicalGroupButton());
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    expect(physicalGroupButton()).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Weight must be greater than 0.")).toBeInTheDocument();
  });
});

describe("FirearmForm resuming pending changes (003 FR-039)", () => {
  afterEach(() => setResumedDraft(null));

  const beretta = {
    id: 4,
    make: "Beretta",
    model: "92FS",
    serialNumber: "BER92-0417",
    noSerialAttested: false,
    caliber: "9mm",
    firearmTypeId: 1,
    status: "active",
    notes: null,
  } as Firearm;

  it("opens with a draft that is short of fields, or has one of another kind, taking the record's values for them", () => {
    // As the human-testing seed keeps it: only some of the form's fields,
    // which once left the others undefined and blanked the window.
    setResumedDraft({
      formVersion: FORM_VERSION,
      kind: "firearm",
      mode: "edit",
      targetId: 4,
      label: "Beretta 92FS — edit",
      values: { make: "Beretta", model: 92, notes: "Swapped the grips; half typed" },
    });

    render(<FirearmForm initialValues={beretta} onSubmit={vi.fn()} />);

    expect(screen.getByLabelText(/Notes/)).toHaveValue("Swapped the grips; half typed");
    expect(screen.getByLabelText(/^Model/)).toHaveValue("92FS");
    expect(screen.getByLabelText(/^Caliber/)).toHaveValue("9mm");
    expect(getDirtyForm()?.label).toBe("Beretta 92FS (edit)");
  });
});
