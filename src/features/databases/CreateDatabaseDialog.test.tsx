import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CommandFailure } from "../../services/tauriClient";
import { CreateDatabaseDialog } from "./CreateDatabaseDialog";
import type { CreateDatabaseInput } from "./types";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

const suggested = { folder: "/home/sam/Documents/HoploDex", name: "My collection" };
const PASSPHRASE = "vivid otter ledger crane";

function renderDialog(onCreate: (input: CreateDatabaseInput) => Promise<void>) {
  const onOpenChange = vi.fn();
  render(
    <CreateDatabaseDialog
      open
      onOpenChange={onOpenChange}
      suggested={suggested}
      onCreate={onCreate}
    />,
  );
  return { onOpenChange };
}

async function fillPassphrases(
  user: ReturnType<typeof userEvent.setup>,
  first: string,
  second = first,
) {
  await user.type(screen.getByLabelText("Passphrase"), first);
  await user.type(screen.getByLabelText("Confirm passphrase"), second);
}

function createButton() {
  return screen.getByRole("button", { name: /Create database|Creating…/ });
}

describe("CreateDatabaseDialog (contracts/ui-databases.md §2)", () => {
  let onCreate: ReturnType<typeof vi.fn<(input: CreateDatabaseInput) => Promise<void>>>;

  beforeEach(() => {
    onCreate = vi.fn<(input: CreateDatabaseInput) => Promise<void>>().mockResolvedValue(undefined);
  });

  it("starts from the suggested name and folder and says where the file goes", async () => {
    const user = userEvent.setup();
    renderDialog(onCreate);

    expect(screen.getByRole("dialog", { name: "Create a database" })).toBeInTheDocument();
    expect(screen.getByLabelText("Name")).toHaveValue("My collection");
    expect(screen.getByLabelText("Folder")).toHaveValue("/home/sam/Documents/HoploDex");
    expect(screen.getByText("This is also the file name.")).toBeInTheDocument();
    expect(
      screen.getByText("Saved as /home/sam/Documents/HoploDex/My collection.hoplodex"),
    ).toBeInTheDocument();

    await user.clear(screen.getByLabelText("Name"));
    await user.type(screen.getByLabelText("Name"), "Grandad's");
    expect(
      screen.getByText("Saved as /home/sam/Documents/HoploDex/Grandad's.hoplodex"),
    ).toBeInTheDocument();
  });

  it("discloses the backups before the database exists (FR-024)", async () => {
    const user = userEvent.setup();
    renderDialog(onCreate);

    const note = screen.getByRole("note", { name: "Backups" });
    expect(note).toHaveTextContent("Backups are on.");
    expect(note).toHaveTextContent("/home/sam/Documents/HoploDex/HoploDex backups");
    expect(note).toHaveTextContent("at most once a day");
    expect(note).toHaveTextContent("keeping the latest 5");
    expect(note).toHaveTextContent("When you close My collection after changing it");

    await user.clear(screen.getByLabelText("Folder"));
    await user.type(screen.getByLabelText("Folder"), "/mnt/usb");
    expect(note).toHaveTextContent("/mnt/usb/HoploDex backups");
  });

  it("keeps Create database disabled until the no-recovery acknowledgement is ticked (FR-004)", async () => {
    const user = userEvent.setup();
    renderDialog(onCreate);

    expect(createButton()).toBeDisabled();
    await user.click(
      screen.getByRole("checkbox", { name: /I have stored this passphrase somewhere safe/ }),
    );
    expect(createButton()).toBeEnabled();
    expect(
      screen.getByText(
        /nobody, including HoploDex, can open this database or recover the collection/,
      ),
    ).toBeInTheDocument();
  });

  it("refuses a short passphrase and a mismatched confirmation, and clears both fields", async () => {
    const user = userEvent.setup();
    renderDialog(onCreate);
    await user.click(screen.getByRole("checkbox", { name: /I have stored this passphrase/ }));

    await fillPassphrases(user, "too short");
    await user.click(createButton());
    expect(screen.getByText("Use at least 12 characters.")).toBeInTheDocument();
    expect(screen.getByLabelText("Passphrase")).toHaveValue("");
    expect(screen.getByLabelText("Confirm passphrase")).toHaveValue("");

    await fillPassphrases(user, PASSPHRASE, `${PASSPHRASE}!`);
    await user.click(createButton());
    expect(screen.getByText("The passphrases don't match.")).toBeInTheDocument();
    expect(screen.queryByText("Use at least 12 characters.")).not.toBeInTheDocument();

    expect(onCreate).not.toHaveBeenCalled();
  });

  it("sends the details with the passphrase read from the field, then clears it", async () => {
    const user = userEvent.setup();
    renderDialog(onCreate);
    await fillPassphrases(user, PASSPHRASE);
    await user.click(screen.getByRole("checkbox", { name: /I have stored this passphrase/ }));
    await user.click(createButton());

    expect(onCreate).toHaveBeenCalledWith({
      folder: "/home/sam/Documents/HoploDex",
      name: "My collection",
      passphrase: PASSPHRASE,
      acknowledgedUnrecoverable: true,
    });
    expect(screen.getByLabelText("Passphrase")).toHaveValue("");
    expect(screen.getByLabelText("Confirm passphrase")).toHaveValue("");
  });

  it("shows Creating… at once and disables the form until the command returns", async () => {
    const user = userEvent.setup();
    let finish: () => void = () => {};
    onCreate.mockReturnValue(new Promise<void>((resolve) => (finish = resolve)));
    renderDialog(onCreate);
    await fillPassphrases(user, PASSPHRASE);
    await user.click(screen.getByRole("checkbox", { name: /I have stored this passphrase/ }));

    await user.click(createButton());

    const pending = screen.getByRole("button", { name: "Creating…" });
    expect(pending).toHaveAttribute("aria-busy", "true");
    expect(pending).toBeDisabled();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
    expect(screen.getByLabelText("Name")).toBeDisabled();
    expect(screen.getByLabelText("Folder")).toBeDisabled();
    expect(screen.getByLabelText("Passphrase")).toBeDisabled();
    expect(screen.getByLabelText("Confirm passphrase")).toBeDisabled();

    finish();
    expect(await screen.findByRole("button", { name: "Create database" })).toBeInTheDocument();
  });

  it("puts the backend's field errors on their fields", async () => {
    const user = userEvent.setup();
    onCreate.mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message: "Check the new database's details.",
        fieldErrors: {
          name: "A name can't end with a space or a dot.",
          folder: "HoploDex can't write to this folder.",
        },
      }),
    );
    renderDialog(onCreate);
    await fillPassphrases(user, PASSPHRASE);
    await user.click(screen.getByRole("checkbox", { name: /I have stored this passphrase/ }));
    await user.click(createButton());

    expect(await screen.findByText("A name can't end with a space or a dot.")).toBeInTheDocument();
    expect(screen.getByLabelText("Name")).toHaveAccessibleDescription(
      /A name can't end with a space or a dot\./,
    );
    expect(screen.getByLabelText("Folder")).toHaveAccessibleDescription(
      /HoploDex can't write to this folder\./,
    );
  });

  it("says so on the name when the file already exists", async () => {
    const user = userEvent.setup();
    onCreate.mockRejectedValue(
      new CommandFailure({
        code: "DATABASE_EXISTS",
        message: "A file with this name already exists in that folder.",
        details: { path: "/home/sam/Documents/HoploDex/My collection.hoplodex" },
      }),
    );
    renderDialog(onCreate);
    await fillPassphrases(user, PASSPHRASE);
    await user.click(screen.getByRole("checkbox", { name: /I have stored this passphrase/ }));
    await user.click(createButton());

    expect(await screen.findByText(/already exists in that folder/)).toBeInTheDocument();
    expect(screen.getByLabelText("Name")).toHaveAttribute("aria-invalid", "true");
  });
});
