import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CommandFailure } from "../../services/tauriClient";
import { RestoreDialog } from "./RestoreDialog";
import type { Firearm } from "./types";

const firearm = {
  id: 1,
  make: "Glock",
  model: "19",
  nickname: "Old Faithful",
  status: "disposed",
  dispositionType: "sold",
  dispositionRecipient: "Jane Doe",
  dispositionDate: "2025-06-15",
  dispositionPrice: 40000,
} as Firearm;

function renderDialog(onRestore = vi.fn().mockResolvedValue(undefined)) {
  const onOpenChange = vi.fn();
  render(
    <RestoreDialog open onOpenChange={onOpenChange} firearm={firearm} onRestore={onRestore} />,
  );
  return { onRestore, onOpenChange };
}

describe("RestoreDialog (FR-033)", () => {
  it("asks what to do with the disposition and never picks for the user", async () => {
    const user = userEvent.setup();
    const { onRestore } = renderDialog();

    // The details being decided on are shown.
    expect(screen.getByText(/Jane Doe/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Restore to collection" })).toBeDisabled();

    await user.click(screen.getByRole("radio", { name: /Keep as history/ }));
    await user.click(screen.getByRole("button", { name: "Restore to collection" }));

    expect(onRestore).toHaveBeenCalledWith({ history: "keep" });
  });

  it("can discard the disposition instead", async () => {
    const user = userEvent.setup();
    const { onRestore } = renderDialog();

    await user.click(screen.getByRole("radio", { name: /Discard/ }));
    await user.click(screen.getByRole("button", { name: "Restore to collection" }));

    expect(onRestore).toHaveBeenCalledWith({ history: "discard" });
  });

  it("names the conflicting record and offers a new nickname when the nickname is taken", async () => {
    const user = userEvent.setup();
    const message = 'That nickname is already used by Sig P226 "Old Faithful".';
    const onRestore = vi
      .fn()
      .mockRejectedValueOnce(
        new CommandFailure({
          code: "VALIDATION_ERROR",
          message,
          fieldErrors: { nickname: message },
        }),
      )
      .mockResolvedValueOnce(undefined);
    const { onOpenChange } = renderDialog(onRestore);

    await user.click(screen.getByRole("radio", { name: /Keep as history/ }));
    await user.click(screen.getByRole("button", { name: "Restore to collection" }));

    expect(await screen.findByText(message)).toBeInTheDocument();
    expect(onOpenChange).not.toHaveBeenCalledWith(false);

    const rename = screen.getByLabelText("New nickname");
    await user.clear(rename);
    await user.type(rename, "Old Faithful II");
    await user.click(screen.getByRole("button", { name: "Restore to collection" }));

    expect(onRestore).toHaveBeenLastCalledWith({ history: "keep", nickname: "Old Faithful II" });
  });

  it("shows a make/model/serial clash without a rename field", async () => {
    const user = userEvent.setup();
    const message = "Glock 19 (serial ABC123) already has this make, model and serial number.";
    const onRestore = vi.fn().mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message,
        fieldErrors: { serialNumber: message },
      }),
    );
    renderDialog(onRestore);

    await user.click(screen.getByRole("radio", { name: /Discard/ }));
    await user.click(screen.getByRole("button", { name: "Restore to collection" }));

    expect(await screen.findByText(message)).toBeInTheDocument();
    expect(screen.queryByLabelText("New nickname")).not.toBeInTheDocument();
  });

  it("shows an identity clash naming the year exception, the same as the form", async () => {
    // specs/002-firearm-identification FR-008
    const user = userEvent.setup();
    const message =
      "Glock 19 (serial ABC123) already has this make, model and serial number. " +
      "Or record a year of manufacture on each firearm: two firearms with the same " +
      "marks are accepted when both have a year and the years differ.";
    const onRestore = vi.fn().mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message,
        fieldErrors: { serialNumber: message },
      }),
    );
    renderDialog(onRestore);

    await user.click(screen.getByRole("radio", { name: /Discard/ }));
    await user.click(screen.getByRole("button", { name: "Restore to collection" }));

    expect(await screen.findByText(message)).toBeInTheDocument();
  });

  it("offers 'Restore anyway' on an original-marks match and resends confirmed (US3-9)", async () => {
    const user = userEvent.setup();
    const message =
      "Ridgeline Arms Hi-Power (serial RA-1) already has these original maker's marks.";
    const onRestore = vi
      .fn()
      .mockRejectedValueOnce(new CommandFailure({ code: "ORIGINAL_MARKS_MATCH", message }))
      .mockResolvedValueOnce(undefined);
    const { onOpenChange } = renderDialog(onRestore);

    await user.click(screen.getByRole("radio", { name: /Keep as history/ }));
    await user.click(screen.getByRole("button", { name: "Restore to collection" }));

    expect(await screen.findByText(message)).toBeInTheDocument();
    expect(onOpenChange).not.toHaveBeenCalledWith(false);

    const restoreAnyway = await screen.findByRole("button", { name: "Restore anyway" });
    await user.click(restoreAnyway);

    expect(onRestore).toHaveBeenLastCalledWith({ history: "keep", confirmedWarnings: true });
  });
});
