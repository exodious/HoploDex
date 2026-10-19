import { act, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import type { FileDropEvent } from "../../services/tauriClient";
import { SessionScopeContext, createSessionScope } from "../session/sessionScope";
import { DocumentList } from "./DocumentList";
import { isPhotoPath } from "./filePaths";
import { PhotoGallery } from "./PhotoGallery";
import { useFileDrop } from "./useFileDrop";

let emit: (event: FileDropEvent) => void = () => {};
const unlisten = vi.fn();
// The backend is pinned at the IPC boundary, as PhotoGallery.test.tsx does it: the
// scope is real, so a command's name and arguments are what the backend would get.
const backend = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  invoke: backend.invoke,
  listenForFileDrops: (handler: (event: FileDropEvent) => void) => {
    emit = handler;
    return unlisten;
  },
}));

function Probe({ onDrop }: { onDrop: (paths: string[]) => void }) {
  const dragging = useFileDrop(isPhotoPath, onDrop);
  return <p>{dragging ? "drop here" : "idle"}</p>;
}

describe("useFileDrop", () => {
  beforeEach(() => unlisten.mockClear());
  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("lights up only while acceptable files are dragged over the window", () => {
    render(<Probe onDrop={() => {}} />);
    expect(screen.getByText("idle")).toBeInTheDocument();

    act(() => emit({ type: "enter", paths: ["/a/gun.png"] }));
    expect(screen.getByText("drop here")).toBeInTheDocument();

    act(() => emit({ type: "leave" }));
    expect(screen.getByText("idle")).toBeInTheDocument();

    act(() => emit({ type: "enter", paths: ["/a/receipt.pdf"] }));
    expect(screen.getByText("idle")).toBeInTheDocument();
  });

  it("hands over just the files it accepts when they are dropped", () => {
    const onDrop = vi.fn();
    render(<Probe onDrop={onDrop} />);

    act(() => emit({ type: "drop", paths: ["/a/gun.png", "/a/receipt.pdf", "/a/gun2.JPG"] }));

    expect(onDrop).toHaveBeenCalledWith(["/a/gun.png", "/a/gun2.JPG"]);
    expect(screen.getByText("idle")).toBeInTheDocument();
  });

  it("ignores drops that hold nothing it accepts", () => {
    const onDrop = vi.fn();
    render(<Probe onDrop={onDrop} />);

    act(() => emit({ type: "drop", paths: ["/a/receipt.pdf"] }));

    expect(onDrop).not.toHaveBeenCalled();
  });

  it("ignores drops while a dialog is open", () => {
    const onDrop = vi.fn();
    render(<Probe onDrop={onDrop} />);
    const dialog = document.createElement("div");
    dialog.setAttribute("role", "dialog");
    document.body.appendChild(dialog);

    act(() => emit({ type: "enter", paths: ["/a/gun.png"] }));
    expect(screen.getByText("idle")).toBeInTheDocument();
    act(() => emit({ type: "drop", paths: ["/a/gun.png"] }));

    expect(onDrop).not.toHaveBeenCalled();
  });

  it("stops listening when it unmounts", () => {
    const { unmount } = render(<Probe onDrop={() => {}} />);
    unmount();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});

// 011 US1-5, 011 FR-004: inside a Flatpak a dropped file outside every grant is refused
// by the add commands with `LOCATION_OUT_OF_REACH` (011 contracts/tauri-commands.md, "Commands
// that check reach"); the record page's drop-error toast shows the backend's message.
describe("a drop refused as out of reach (011 US1-5, 011 FR-004)", () => {
  const message =
    "This copy of HoploDex is a Flatpak, and it can only reach your home folder and removable drives, so it can't use /srv/outside/gun.png. Move the file into your home folder or onto a removable drive, or give HoploDex access to /srv/outside by running: flatpak override --user --filesystem=/srv/outside io.github.exodious.HoploDex and then restarting HoploDex.";
  const refusal = () =>
    new CommandFailure({
      code: "LOCATION_OUT_OF_REACH",
      message,
      details: {
        path: "/srv/outside/gun.png",
        folder: "/srv/outside",
        grants: ["your home folder"],
      },
    });
  const owner = { kind: "firearm", id: 1 } as const;

  beforeEach(() => {
    backend.invoke.mockReset();
    backend.invoke.mockImplementation(async (command: string) => {
      if (command === "list_photos" || command === "list_documents") return [];
      if (command === "list_document_types") return [];
      throw refusal();
    });
  });

  function wrap(children: ReactNode) {
    return render(
      <ToastProvider>
        <SessionScopeContext.Provider value={createSessionScope(1)}>
          {children}
        </SessionScopeContext.Provider>
      </ToastProvider>,
    );
  }

  it("shows the message when a dropped photo is refused", async () => {
    wrap(<PhotoGallery owner={owner} thumbnailPhotoId={null} onChanged={vi.fn()} />);
    await screen.findByText(/Drop photos here/);

    act(() => emit({ type: "drop", paths: ["/srv/outside/gun.png"] }));

    expect(await screen.findByText(message)).toBeInTheDocument();
  });

  it("shows the message when a dropped document is refused", async () => {
    wrap(<DocumentList owner={owner} />);
    await screen.findByText(/Drop receipts, bills of sale/);

    act(() => emit({ type: "drop", paths: ["/srv/outside/receipt.pdf"] }));

    expect(await screen.findByText(message)).toBeInTheDocument();
  });
});
