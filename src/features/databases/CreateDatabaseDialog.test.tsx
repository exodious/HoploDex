import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
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

describe("CreateDatabaseDialog (003 contracts/ui-databases.md §2)", () => {
  let onCreate: ReturnType<typeof vi.fn<(input: CreateDatabaseInput) => Promise<void>>>;

  beforeEach(() => {
    onCreate = vi.fn<(input: CreateDatabaseInput) => Promise<void>>().mockResolvedValue(undefined);
  });

  // 003 US2-1
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

  it("discloses the backups before the database exists (003 FR-024)", async () => {
    const user = userEvent.setup();
    renderDialog(onCreate);

    const note = screen.getByRole("note", { name: "Backups" });
    expect(note).toHaveTextContent("Backups are on.");
    expect(note).toHaveTextContent("/home/sam/Documents/HoploDex/HoploDex backups");
    expect(note).toHaveTextContent("at most once a day");
    expect(note).toHaveTextContent("keeping the latest 5");
    expect(note).toHaveTextContent("When “My collection” locks or HoploDex quits after a change");

    await user.clear(screen.getByLabelText("Folder"));
    await user.type(screen.getByLabelText("Folder"), "/mnt/usb");
    expect(note).toHaveTextContent("/mnt/usb/HoploDex backups");
  });

  // 003 US1-3
  it("keeps Create database disabled until the no-recovery acknowledgement is ticked (003 FR-004)", async () => {
    const user = userEvent.setup();
    renderDialog(onCreate);
    await fillPassphrases(user, PASSPHRASE);

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

  // 003 US1-2
  it("checks the passphrases as they are typed, and waits until they are long enough and match", async () => {
    const user = userEvent.setup();
    renderDialog(onCreate);
    await user.click(screen.getByRole("checkbox", { name: /I have stored this passphrase/ }));
    expect(createButton()).toBeDisabled();

    await fillPassphrases(user, "too short");
    expect(await screen.findByText("Use at least 12 characters.")).toBeInTheDocument();
    expect(createButton()).toBeDisabled();

    await user.clear(screen.getByLabelText("Passphrase"));
    await user.clear(screen.getByLabelText("Confirm passphrase"));
    await fillPassphrases(user, PASSPHRASE, `${PASSPHRASE}!`);
    expect(await screen.findByText("The passphrases don't match.")).toBeInTheDocument();
    expect(screen.queryByText("Use at least 12 characters.")).not.toBeInTheDocument();
    expect(createButton()).toBeDisabled();

    await user.type(screen.getByLabelText("Confirm passphrase"), "{Backspace}");
    expect(screen.queryByText("The passphrases don't match.")).not.toBeInTheDocument();
    expect(createButton()).toBeEnabled();
    expect(onCreate).not.toHaveBeenCalled();
  });

  it("says a confirmation left a character short doesn't match", async () => {
    const user = userEvent.setup();
    renderDialog(onCreate);
    await user.click(screen.getByRole("checkbox", { name: /I have stored this passphrase/ }));

    await fillPassphrases(user, PASSPHRASE, PASSPHRASE.slice(0, -1));
    expect(await screen.findByText("The passphrases don't match.")).toBeInTheDocument();
    expect(createButton()).toBeDisabled();
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

  // The constitution's UI consistency: a refused field takes the focus, as in every form.
  it("moves focus to the first field the backend refused", async () => {
    const user = userEvent.setup();
    onCreate.mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message: "Check the new database's details.",
        fieldErrors: {
          folder: "HoploDex can't write to this folder.",
          name: "A name can't end with a space or a dot.",
        },
      }),
    );
    renderDialog(onCreate);
    await fillPassphrases(user, PASSPHRASE);
    await user.click(screen.getByRole("checkbox", { name: /I have stored this passphrase/ }));
    await user.click(createButton());

    await screen.findByText("A name can't end with a space or a dot.");
    await waitFor(() => expect(screen.getByLabelText("Name")).toHaveFocus());
  });

  // 011 US1-8, 011 FR-004: the Flatpak refuses a folder it can't reach, before writing anything.
  it("puts a location out of the Flatpak's reach on the folder, and focuses it", async () => {
    const user = userEvent.setup();
    const message =
      "This copy of HoploDex is a Flatpak, and it can only reach the HoploDex folder in your Documents folder (/home/sam/Documents/HoploDex) and removable drives, so it can't use /srv/hoplodex-check/My collection.hoplodex. Move the file into /home/sam/Documents/HoploDex or onto a removable drive, or give HoploDex access to /srv/hoplodex-check by running: flatpak override --user --filesystem=/srv/hoplodex-check io.github.exodious.HoploDex and then restarting HoploDex.";
    onCreate.mockRejectedValue(
      new CommandFailure({
        code: "LOCATION_OUT_OF_REACH",
        message,
        fieldErrors: { folder: message },
        details: {
          path: "/srv/hoplodex-check/My collection.hoplodex",
          folder: "/srv/hoplodex-check",
          grants: ["/home/sam/Documents/HoploDex", "/media", "/run/media"],
        },
      }),
    );
    renderDialog(onCreate);
    await user.clear(screen.getByLabelText("Folder"));
    await user.type(screen.getByLabelText("Folder"), "/srv/hoplodex-check");
    await fillPassphrases(user, PASSPHRASE);
    await user.click(screen.getByRole("checkbox", { name: /I have stored this passphrase/ }));
    await user.click(createButton());

    expect(await screen.findByText(message)).toBeInTheDocument();
    expect(screen.getByLabelText("Folder")).toHaveAccessibleDescription(
      expect.stringContaining(message),
    );
    await waitFor(() => expect(screen.getByLabelText("Folder")).toHaveFocus());
    expect(screen.getByRole("dialog", { name: "Create a database" })).toBeInTheDocument();
  });

  it("moves focus to the name when the file already exists", async () => {
    const user = userEvent.setup();
    onCreate.mockRejectedValue(
      new CommandFailure({ code: "DATABASE_EXISTS", message: "A file with this name exists." }),
    );
    renderDialog(onCreate);
    await fillPassphrases(user, PASSPHRASE);
    await user.click(screen.getByRole("checkbox", { name: /I have stored this passphrase/ }));
    await user.click(createButton());

    await screen.findByText("A file with this name exists.");
    await waitFor(() => expect(screen.getByLabelText("Name")).toHaveFocus());
  });

  // 003 data-model.md "Validation rules", the create target (the name field takes the
  // refusal: M2, the field-error mechanism)
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

  // Issue #110: a full disk is told as it is, in the banner, with the form kept.
  it("shows what to do when the disk is full, and keeps the form for another try", async () => {
    const user = userEvent.setup();
    const message =
      "The disk is full. Free some space on the drive that holds /home/sam/Documents/HoploDex/My collection.hoplodex, then try again.";
    onCreate.mockRejectedValue(
      new CommandFailure({
        code: "DISK_FULL",
        message,
        details: { path: "/home/sam/Documents/HoploDex/My collection.hoplodex" },
      }),
    );
    const { onOpenChange } = renderDialog(onCreate);
    await fillPassphrases(user, PASSPHRASE);
    await user.click(screen.getByRole("checkbox", { name: /I have stored this passphrase/ }));
    await user.click(createButton());

    expect(await screen.findByRole("alert")).toHaveTextContent(message);
    expect(onOpenChange).not.toHaveBeenCalled();
    expect(screen.getByLabelText("Name")).toHaveValue("My collection");
  });
});
