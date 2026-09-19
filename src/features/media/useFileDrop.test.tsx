import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { FileDropEvent } from "../../services/tauriClient";
import { isPhotoPath } from "./filePaths";
import { useFileDrop } from "./useFileDrop";

let emit: (event: FileDropEvent) => void = () => {};
const unlisten = vi.fn();

vi.mock("../../services/tauriClient", () => ({
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
