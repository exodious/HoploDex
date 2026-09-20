import { describe, expect, it, vi } from "vitest";
import { notifySaveWarnings } from "./saveWarnings";

describe("notifySaveWarnings (FR-032b)", () => {
  it("says nothing when the save drew no warnings", () => {
    const notify = vi.fn();
    notifySaveWarnings(notify, []);
    expect(notify).not.toHaveBeenCalled();
  });

  it("shows the warnings as one warning toast", () => {
    const notify = vi.fn();
    notifySaveWarnings(notify, ["Matches Colt 1911 (serial 123).", "Something else."]);
    expect(notify).toHaveBeenCalledWith(
      "Matches Colt 1911 (serial 123). Something else.",
      "warning",
    );
  });
});
