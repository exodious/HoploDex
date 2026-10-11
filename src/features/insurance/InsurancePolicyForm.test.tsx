import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
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

  it("submits a limit in whole dollars, making it a blanket policy", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<InsurancePolicyForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(screen.getByLabelText("Blanket coverage limit"), "25,000");
    await user.click(screen.getByRole("button", { name: "Add policy" }));

    expect(onSubmit.mock.calls[0][0].blanketCoverageLimit).toBe(25_000);
  });

  it("refuses a limit above $99,999,999 before sending, and accepts exactly that (#67)", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<InsurancePolicyForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(screen.getByLabelText("Blanket coverage limit"), "100000000");
    await user.click(screen.getByRole("button", { name: "Add policy" }));

    expect(screen.getByText("Enter an amount of $99,999,999 or less.")).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();

    const limit = screen.getByLabelText("Blanket coverage limit");
    await user.clear(limit);
    await user.type(limit, "99999999");
    await user.click(screen.getByRole("button", { name: "Add policy" }));
    expect(onSubmit.mock.calls[0][0].blanketCoverageLimit).toBe(99_999_999);
  });

  it("explains what the limit means", () => {
    render(<InsurancePolicyForm onSubmit={vi.fn()} />);

    expect(
      screen.getByText(/every firearm and accessory not scheduled individually/i, { exact: false }),
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
          notes: null,
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

  it("submits blank notes as null", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<InsurancePolicyForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(screen.getByLabelText("Notes"), "   ");
    await user.click(screen.getByRole("button", { name: "Add policy" }));

    expect(onSubmit.mock.calls[0][0].notes).toBeNull();
  });

  it("submits entered notes, keeping line breaks", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<InsurancePolicyForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(
      screen.getByLabelText("Notes"),
      "Renews in January.{Enter}Ask about the rider.",
    );
    await user.click(screen.getByRole("button", { name: "Add policy" }));

    expect(onSubmit.mock.calls[0][0].notes).toBe("Renews in January.\nAsk about the rider.");
  });

  it("prefills the notes from an existing policy", () => {
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
          notes: "Keep the appraisal with this policy.",
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

    expect(screen.getByLabelText("Notes")).toHaveValue("Keep the appraisal with this policy.");
  });

  // specs/008-hardening-batch research.md §10, contracts/ui.md §3.
  it.each([
    ["Policy name", 200],
    ["Policy number", 200],
    ["Insurance company", 200],
    ["Company contact", 200],
    ["Agent name", 200],
    ["Agent contact", 200],
    ["Notes", 4000],
  ])("shows %s's maximum of %i at once and refuses to save", async (label, max) => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<InsurancePolicyForm onSubmit={onSubmit} />);
    await fillRequired(user);
    const message = `${label} can be at most ${max.toLocaleString("en-US")} characters.`;

    const field = screen.getByLabelText(new RegExp(`^${label}`));
    fireEvent.change(field, { target: { value: "x".repeat(max) } });
    expect(screen.queryByText(message)).not.toBeInTheDocument();

    // Pasted text is never cut off, and the error shows before leaving the field.
    const long = "x".repeat(max + 1);
    fireEvent.change(field, { target: { value: long } });
    expect(screen.getByText(message)).toBeInTheDocument();
    expect(field).toHaveValue(long);
    expect(field).toHaveAccessibleDescription(expect.stringContaining(message));

    await user.click(screen.getByRole("button", { name: "Add policy" }));
    expect(onSubmit).not.toHaveBeenCalled();

    fireEvent.change(field, { target: { value: "x".repeat(max) } });
    expect(screen.queryByText(message)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Add policy" }));
    expect(onSubmit).toHaveBeenCalledTimes(1);
  });

  it("counts characters, after trimming, and puts no maxLength on any field", () => {
    const { container } = render(<InsurancePolicyForm onSubmit={vi.fn()} />);
    const name = screen.getByLabelText(/Policy name/);

    fireEvent.change(name, { target: { value: `  ${"\u{1F52B}".repeat(200)}  ` } });
    expect(screen.queryByText(/can be at most/)).not.toBeInTheDocument();

    expect(container.querySelectorAll("[maxlength]")).toHaveLength(0);
  });

  it("shows a count under the notes from 3,600 characters, as the error style over 4,000", () => {
    render(<InsurancePolicyForm onSubmit={vi.fn()} />);
    const notes = screen.getByLabelText("Notes");

    fireEvent.change(notes, { target: { value: "x".repeat(3599) } });
    expect(screen.queryByText(/of 4,000/)).not.toBeInTheDocument();
    fireEvent.change(notes, { target: { value: "x".repeat(3600) } });
    expect(screen.getByText("3,600 of 4,000")).toBeInTheDocument();
    fireEvent.change(notes, { target: { value: "x".repeat(4001) } });
    expect(screen.getByText("4,001 of 4,000")).toBeInTheDocument();
  });
});
