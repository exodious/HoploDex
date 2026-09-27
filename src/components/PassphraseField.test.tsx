import { createRef } from "react";
import { describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CLEAR_PASSPHRASE_FIELDS, PassphraseField } from "./PassphraseField";
import type { PassphraseFieldHandle } from "./PassphraseField";

/** The backend's events, sent by the test. */
const events = vi.hoisted(() => {
  const listeners = new Map<string, Set<(event: { payload: unknown }) => void>>();
  return {
    listen: (name: string, handler: (event: { payload: unknown }) => void) => {
      const forName = listeners.get(name) ?? new Set();
      forName.add(handler);
      listeners.set(name, forName);
      return Promise.resolve(() => forName.delete(handler));
    },
    send: (name: string) => listeners.get(name)?.forEach((handler) => handler({ payload: {} })),
  };
});
vi.mock("@tauri-apps/api/event", () => ({ listen: events.listen }));

describe("PassphraseField (contracts/ui-databases.md §0, FR-007)", () => {
  it("is a password input that the browser won't spell-check, capitalize or remember as text", () => {
    render(<PassphraseField label="Passphrase" autoComplete="new-password" />);

    const input = screen.getByLabelText("Passphrase");
    expect(input).toHaveAttribute("type", "password");
    expect(input).toHaveAttribute("spellcheck", "false");
    expect(input).toHaveAttribute("autocapitalize", "off");
    expect(input).toHaveAttribute("autocomplete", "new-password");
  });

  it("shows and hides the passphrase with a pressed toggle", async () => {
    const user = userEvent.setup();
    render(<PassphraseField label="Passphrase" autoComplete="current-password" />);
    const input = screen.getByLabelText("Passphrase");
    const toggle = screen.getByRole("button", { name: "Show passphrase" });
    expect(toggle).toHaveAttribute("aria-pressed", "false");

    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-pressed", "true");
    expect(input).toHaveAttribute("type", "text");

    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-pressed", "false");
    expect(input).toHaveAttribute("type", "password");
  });

  it("is read through its handle, and reset empties and hides it", async () => {
    const user = userEvent.setup();
    const ref = createRef<PassphraseFieldHandle>();
    render(<PassphraseField ref={ref} label="Passphrase" autoComplete="current-password" />);
    const input = screen.getByLabelText("Passphrase");

    await user.type(input, "correct horse battery");
    await user.click(screen.getByRole("button", { name: "Show passphrase" }));
    expect(ref.current?.read()).toBe("correct horse battery");

    act(() => ref.current?.reset());
    expect(input).toHaveValue("");
    expect(input).toHaveAttribute("type", "password");
    expect(ref.current?.read()).toBe("");
  });

  it("keeps the value out of React: nothing re-renders it into the DOM elsewhere", async () => {
    const user = userEvent.setup();
    const { container } = render(
      <PassphraseField label="Passphrase" autoComplete="new-password" strength />,
    );

    await user.type(screen.getByLabelText("Passphrase"), "zebra-lantern-quartz");

    // The only place the text exists is the input's own value.
    expect(container.innerHTML).not.toContain("zebra-lantern-quartz");
  });

  it("shows the lazily loaded strength hint, which never blocks", async () => {
    const user = userEvent.setup();
    render(<PassphraseField label="Passphrase" autoComplete="new-password" strength />);

    expect(
      screen.getByText("Longer is stronger: several unrelated words make a good passphrase."),
    ).toBeInTheDocument();

    await user.type(screen.getByLabelText("Passphrase"), "password");
    const meter = await screen.findByRole("meter", { name: "Passphrase strength" });
    expect(meter).toHaveAttribute("aria-valuemin", "0");
    expect(meter).toHaveAttribute("aria-valuemax", "4");
    expect(meter.querySelectorAll(".hd-strength__step")).toHaveLength(5);
    expect(await screen.findByText("Very weak")).toBeInTheDocument();
    // zxcvbn's own advice for a common password.
    expect(screen.getByTestId("strength-suggestion")).not.toBeEmptyDOMElement();

    await user.clear(screen.getByLabelText("Passphrase"));
    await user.type(screen.getByLabelText("Passphrase"), "vivid otter ledger crane mosaic");
    expect(await screen.findByText("Very strong")).toBeInTheDocument();
    expect(screen.getByLabelText("Passphrase")).not.toHaveAttribute("aria-invalid");
  });

  it("shows field errors in the standard slot", () => {
    render(
      <PassphraseField
        label="Passphrase"
        autoComplete="new-password"
        error="Use at least 12 characters."
      />,
    );

    const input = screen.getByLabelText("Passphrase");
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByRole("alert")).toHaveTextContent("Use at least 12 characters.");
    expect(input).toHaveAccessibleDescription(/Use at least 12 characters\./);
  });

  it("empties every field when the screen locks or the computer sleeps (FR-007)", async () => {
    const user = userEvent.setup();
    render(
      <>
        <PassphraseField label="Passphrase" autoComplete="new-password" />
        <PassphraseField label="Confirm passphrase" autoComplete="new-password" />
      </>,
    );
    await user.type(screen.getByLabelText("Passphrase"), "correct horse battery");
    await user.type(screen.getByLabelText("Confirm passphrase"), "correct horse battery");
    await user.click(screen.getAllByRole("button", { name: "Show passphrase" })[0]);
    // Subscribed once the effect has run.
    await act(async () => {});

    act(() => events.send(CLEAR_PASSPHRASE_FIELDS));

    expect(screen.getByLabelText("Passphrase")).toHaveValue("");
    expect(screen.getByLabelText("Confirm passphrase")).toHaveValue("");
    expect(screen.getByLabelText("Passphrase")).toHaveAttribute("type", "password");
  });
});
