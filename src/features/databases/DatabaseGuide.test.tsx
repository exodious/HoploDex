import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SessionContext } from "../session/sessionStore";
import type { SessionState } from "../session/sessionStore";
import { DatabaseGuide, DatabaseGuideLink } from "./DatabaseGuide";
import type { DatabaseStatus } from "./types";

// 003 contracts/ui-databases.md §11, FR-030
describe("DatabaseGuide", () => {
  it("is a dialog titled About databases and security", () => {
    render(<DatabaseGuide open onOpenChange={vi.fn()} />);

    expect(
      screen.getByRole("dialog", { name: "About databases and security" }),
    ).toBeInTheDocument();
  });

  it("has a headed section for each topic, in order", () => {
    render(<DatabaseGuide open onOpenChange={vi.fn()} />);

    const headings = within(screen.getByRole("dialog"))
      .getAllByRole("heading", { level: 3 })
      .map((heading) => heading.textContent);
    expect(headings).toEqual([
      "Your passphrase",
      "Remembering the passphrase on this computer",
      "Locking",
      "Backups",
      "Secure deletion",
      "Using a database on more than one computer",
      "Whole-disk encryption",
    ]);
  });

  it("explains passphrase length, that there is no recovery, and what protects a copied file", () => {
    render(<DatabaseGuide open onOpenChange={vi.fn()} />);

    const section = screen.getByRole("region", { name: "Your passphrase" });
    expect(section).toHaveTextContent(/length matters most/i);
    expect(section).toHaveTextContent(/no limit on attempts/i);
    expect(section).toHaveTextContent(/cannot be recovered/i);
    expect(section).toHaveTextContent(/copy of the file is protected only by the passphrase/i);
  });

  it("says what a saved passphrase allows, and that locking then no longer needs it", () => {
    render(<DatabaseGuide open onOpenChange={vi.fn()} />);

    const section = screen.getByRole("region", {
      name: "Remembering the passphrase on this computer",
    });
    expect(section).toHaveTextContent(/off unless you turn it on/i);
    expect(section).toHaveTextContent(/anyone who can use this computer account/i);
    expect(section).toHaveTextContent(/locking no longer needs the passphrase/i);
  });

  it("describes locking, the idle and sleep locks, pending changes and the screen-lock caveat", () => {
    render(<DatabaseGuide open onOpenChange={vi.fn()} />);

    const section = screen.getByRole("region", { name: "Locking" });
    expect(section).toHaveTextContent(/closes the database completely/i);
    expect(section).toHaveTextContent(/10 minutes/);
    expect(section).toHaveTextContent(/goes to sleep/i);
    expect(section).toHaveTextContent(/kept inside the database/i);
    expect(section).toHaveTextContent(/window manager/i);
  });

  it("covers where backups go, what they hold, their passphrase, deleted records, same disk and cloud folders", () => {
    render(<DatabaseGuide open onOpenChange={vi.fn()} />);

    const section = screen.getByRole("region", { name: "Backups" });
    expect(section).toHaveTextContent(/HoploDex backups/);
    expect(section).toHaveTextContent(/complete copy of the collection/i);
    expect(section).toHaveTextContent(/passphrase the database had when the backup was made/i);
    expect(section).toHaveTextContent(/deleted stay in backups made before/i);
    expect(section).toHaveTextContent(/same disk/i);
    expect(section).toHaveTextContent(/synced by a cloud service/i);
    expect(section).toHaveTextContent(/spreadsheet export is not a backup/i);
  });

  it("says secure deletion is best effort, and why an old copy is still protected", () => {
    render(<DatabaseGuide open onOpenChange={vi.fn()} />);

    const section = screen.getByRole("region", { name: "Secure deletion" });
    expect(section).toHaveTextContent(/best effort/i);
    for (const where of [/SSD/, /journaling/i, /copy-on-write/i, /snapshots/i, /cloud/i]) {
      expect(section).toHaveTextContent(where);
    }
    expect(section).toHaveTextContent(/still protected by the passphrase it had/i);
    expect(section).toHaveTextContent(/made earlier, or elsewhere, are not touched/i);
  });

  it("covers one computer at a time, the open marker and take-over", () => {
    render(<DatabaseGuide open onOpenChange={vi.fn()} />);

    const section = screen.getByRole("region", {
      name: "Using a database on more than one computer",
    });
    expect(section).toHaveTextContent(/one computer at a time/i);
    expect(section).toHaveTextContent(/records which computer has it open/i);
    expect(section).toHaveTextContent(/Take over/);
    expect(section).toHaveTextContent(/changes can be lost/i);
  });

  describe("the Flatpak paragraph (011 FR-009, 011 research.md §11)", () => {
    const LINUX = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko)";
    const MACOS = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15";
    const WINDOWS = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Edg/120.0";

    afterEach(() => vi.restoreAllMocks());

    function several(userAgent: string) {
      vi.spyOn(window.navigator, "userAgent", "get").mockReturnValue(userAgent);
      render(<DatabaseGuide open onOpenChange={vi.fn()} />);
      return screen.getByRole("region", { name: "Using a database on more than one computer" });
    }

    it("is shown on Linux, in the section on several computers", () => {
      const section = several(LINUX);

      expect(section).toHaveTextContent("If you have installed HoploDex as a Flatpak");
      expect(section).toHaveTextContent(/separate recent lists, settings and saved passphrases/i);
      expect(section).toHaveTextContent(/\.deb or \.rpm/);
      expect(section).toHaveTextContent(/a passphrase saved in one is not seen by the other/i);
      expect(section).not.toHaveTextContent(/shared when both reach the same keyring/i);
      expect(section).toHaveTextContent(
        "By default a Flatpak reaches only the HoploDex folder in your Documents folder and removable drives.",
      );
      expect(section).not.toHaveTextContent(/home folder/i);
      expect(section).toHaveTextContent(
        "flatpak override --user --filesystem=<folder> io.github.exodious.HoploDex and restart HoploDex",
      );
    });

    it.each([
      ["macOS", MACOS],
      ["Windows", WINDOWS],
    ])("is not shown on %s", (_name, userAgent) => {
      const section = several(userAgent);

      expect(section).not.toHaveTextContent("Flatpak");
      expect(screen.queryByText(/Flatpak/)).not.toBeInTheDocument();
    });
  });

  it("recommends whole-disk encryption and names each system's", () => {
    render(<DatabaseGuide open onOpenChange={vi.fn()} />);

    const section = screen.getByRole("region", { name: "Whole-disk encryption" });
    for (const name of ["BitLocker", "FileVault", "LUKS"]) {
      expect(section).toHaveTextContent(name);
    }
  });
});

// The open database's settings, beside the defaults.
describe("DatabaseGuide with a database open", () => {
  const status = {
    name: "Main collection",
    screenLockSupported: true,
    settings: {
      backups: {
        enabled: true,
        keepCount: 10,
        location: { kind: "custom", path: "/mnt/usb/HoploDex backups", available: false },
      },
      lock: { idleEnabled: true, idleMinutes: 10, onScreenLock: true },
    },
  } as DatabaseStatus;

  function renderOpen() {
    render(
      <SessionContext.Provider value={{ status } as SessionState}>
        <DatabaseGuide open onOpenChange={vi.fn()} />
      </SessionContext.Provider>,
    );
  }

  function rows(section: string): string[] {
    const group = within(screen.getByRole("region", { name: section })).getByRole("group", {
      name: "How this database is set up",
    });
    return within(group)
      .getAllByRole("definition")
      .map((value) => value.textContent ?? "");
  }

  it("shows each backup setting, with its default when it was changed", () => {
    renderOpen();

    expect(rows("Backups")).toEqual([
      "On (default)",
      "10 (default: 5)",
      "/mnt/usb/HoploDex backups (default: next to the database)Not available on this computer",
    ]);
  });

  // 011 FR-004: the guide's note on the folder reads like the settings dialog's
  it("says this copy can't reach a backup folder out of the Flatpak's reach", () => {
    const outOfReach = {
      ...status,
      settings: {
        ...status.settings,
        backups: {
          ...status.settings.backups,
          location: {
            kind: "custom",
            path: "/srv/hoplodex-backups",
            available: false,
            unavailableReason: "outOfReach",
          },
        },
      },
    } as DatabaseStatus;
    render(
      <SessionContext.Provider value={{ status: outOfReach } as SessionState}>
        <DatabaseGuide open onOpenChange={vi.fn()} />
      </SessionContext.Provider>,
    );

    const folder = rows("Backups")[2];
    expect(folder).toContain("This copy of HoploDex can't reach this folder");
    expect(folder).toContain("flatpak override --user --filesystem=/srv/hoplodex-backups");
    expect(folder).not.toContain("Not available on this computer");
  });

  it("shows each lock setting the same way", () => {
    renderOpen();

    expect(rows("Locking")).toEqual([
      "After 10 minutes (default)",
      "On (default)",
      "On (default: off)",
    ]);
  });

  it("shows no settings with no database open", () => {
    render(<DatabaseGuide open onOpenChange={vi.fn()} />);

    expect(screen.queryByRole("group")).not.toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Backups" })).toHaveTextContent(
      /By default it keeps the latest 5/,
    );
  });
});

describe("DatabaseGuideLink", () => {
  it("opens the guide, and closing it returns focus to the link", async () => {
    const user = userEvent.setup();
    render(<DatabaseGuideLink />);

    const link = screen.getByRole("button", { name: "About databases and security" });
    await user.click(link);
    expect(
      screen.getByRole("dialog", { name: "About databases and security" }),
    ).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await waitFor(() => expect(link).toHaveFocus());
  });

  it("takes its own label", () => {
    render(<DatabaseGuideLink>Why?</DatabaseGuideLink>);

    expect(screen.getByRole("button", { name: "Why?" })).toBeInTheDocument();
  });
});
