import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CommandFailure } from "../../services/tauriClient";
import { InsurancePolicyForm } from "./InsurancePolicyForm";

async function fillRequired(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByLabelText(/Policy name/), "Homeowner's rider");
  await user.type(screen.getByLabelText(/Policy number/), "HR-1");
  await user.type(screen.getByLabelText(/Insurance company/), "Acme");
  await user.type(screen.getByLabelText(/Coverage starts/), "2026-01-01");
  await user.type(screen.getByLabelText(/Coverage ends/), "2026-12-31");
}

describe("InsurancePolicyForm (FR-027, FR-036)", () => {
  it("makes the blanket limit optional: blank submits null, a schedule-only policy", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<InsurancePolicyForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(screen.getByRole("button", { name: "Add policy" }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit.mock.calls[0][0].blanketCoverageLimit).toBeNull();
  });

  it("submits a limit in cents, making it a blanket policy", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<InsurancePolicyForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(screen.getByLabelText("Blanket coverage limit"), "25,000");
    await user.click(screen.getByRole("button", { name: "Add policy" }));

    expect(onSubmit.mock.calls[0][0].blanketCoverageLimit).toBe(2_500_000);
  });

  it("explains what the limit means", () => {
    render(<InsurancePolicyForm onSubmit={vi.fn()} />);

    expect(
      screen.getByText(/every firearm not scheduled individually/i, { exact: false }),
    ).toBeInTheDocument();
  });

  it("shows the backend's overlap message, naming the other policy, on the start date", async () => {
    const user = userEvent.setup();
    const message = "These dates overlap the blanket policy Old policy (2025-01-01 to 2026-01-01).";
    const onSubmit = vi.fn().mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message,
        fieldErrors: { effectiveStartDate: message },
      }),
    );
    render(<InsurancePolicyForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(screen.getByLabelText("Blanket coverage limit"), "1000");
    await user.click(screen.getByRole("button", { name: "Add policy" }));

    expect(await screen.findByText(message)).toBeInTheDocument();
  });

  it("prefills an existing schedule-only policy with a blank limit", () => {
    render(
      <InsurancePolicyForm
        initialValues={{
          id: 1,
          name: "Rider",
          policyNumber: "R-1",
          insuranceCompany: "Acme",
          companyContact: null,
          agentName: null,
          agentContact: null,
          blanketCoverageLimit: null,
          effectiveStartDate: "2026-01-01",
          effectiveEndDate: "2026-12-31",
          createdAt: "",
          updatedAt: "",
          isInForce: true,
          isExpired: false,
          isExpiringSoon: false,
          expiringWarning: false,
          expiredWarning: false,
        }}
        onSubmit={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("Blanket coverage limit")).toHaveValue("");
  });
});
