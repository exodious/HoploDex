import { useState } from "react";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Button, ConfirmDialog, Menu, MenuItem, ToastProvider, useToast } from "../../components";
import { PreviewSurface } from "./PreviewSurface";

// specs/007-document-preview/contracts/ui-document-preview.md §2 ("Page
// area"), tasks.md T039. The component is `<PreviewSurface previewId name />`:
// the placeholder the PDF surface is placed over. It talks to the backend
// through `set_preview_bounds` ({ previewId, bounds, visible }) and listens for
// `preview:pdf-ready` ({ previewId }); both are pinned here at the IPC
// boundary (`invoke` and `listen` of tauriClient), so the service's own names
// are free. It finds out that something is drawn over it (a ConfirmDialog, a
// Menu, a toast) from the page itself, so a screen has nothing to pass it.

const backend = vi.hoisted(() => {
  const handlers = new Map<string, Set<(payload: unknown) => void>>();
  return {
    invoke: vi.fn(),
    listen: vi.fn((event: string, handler: (payload: unknown) => void) => {
      if (!handlers.has(event)) handlers.set(event, new Set());
      handlers.get(event)!.add(handler);
      return () => handlers.get(event)?.delete(handler);
    }),
    emit(event: string, payload: unknown) {
      handlers.get(event)?.forEach((handler) => handler(payload));
    },
    listeners(event: string) {
      return handlers.get(event)?.size ?? 0;
    },
    reset() {
      handlers.clear();
    },
  };
});
vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  invoke: backend.invoke,
  listen: backend.listen,
}));

const NAME = "Purchase receipt.pdf";
const PREVIEW_ID = 5;

let rect: { x: number; y: number; width: number; height: number };
const resizeObservers: (() => void)[] = [];

function boundsCalls() {
  return backend.invoke.mock.calls
    .filter(([command]) => command === "set_preview_bounds")
    .map(([, args]) => args as { previewId: number; bounds: typeof rect; visible: boolean });
}
const lastBounds = () => boundsCalls().at(-1);

beforeEach(() => {
  backend.reset();
  backend.invoke.mockReset().mockResolvedValue(null);
  rect = { x: 20, y: 30, width: 640, height: 480 };
  resizeObservers.length = 0;
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(
    () =>
      ({
        ...rect,
        left: rect.x,
        top: rect.y,
        right: rect.x + rect.width,
        bottom: rect.y + rect.height,
        toJSON: () => ({}),
      }) as DOMRect,
  );
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(callback: ResizeObserverCallback) {
        resizeObservers.push(() => callback([], this as unknown as ResizeObserver));
      }
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

/** The surface with the three things that can be drawn over it. */
function Harness() {
  const [confirming, setConfirming] = useState(false);
  const notify = useToast();
  return (
    <>
      <PreviewSurface previewId={PREVIEW_ID} name={NAME} />
      <Button onClick={() => setConfirming(true)}>Delete</Button>
      <Menu trigger={<Button>Actions</Button>}>
        <MenuItem onSelect={() => {}}>One</MenuItem>
      </Menu>
      <Button onClick={() => notify("Opened Purchase receipt.pdf in another app.")}>Toast</Button>
      <ConfirmDialog
        open={confirming}
        onOpenChange={setConfirming}
        title="Delete this document?"
        description="It will be removed."
        confirmLabel="Delete document"
        onConfirm={() => {}}
      />
    </>
  );
}

function renderHarness() {
  return render(
    <ToastProvider>
      <Harness />
    </ToastProvider>,
  );
}

describe("PreviewSurface: bounds (contract §2, contracts/tauri-commands.md set_preview_bounds)", () => {
  it("sends its rectangle on mount", async () => {
    render(<PreviewSurface previewId={PREVIEW_ID} name={NAME} />);

    await waitFor(() => expect(lastBounds()).toBeDefined());
    expect(lastBounds()).toEqual({
      previewId: PREVIEW_ID,
      bounds: { x: 20, y: 30, width: 640, height: 480 },
      visible: true,
    });
  });

  it("sends the new rectangle when the page area is resized", async () => {
    render(<PreviewSurface previewId={PREVIEW_ID} name={NAME} />);
    await waitFor(() => expect(lastBounds()).toBeDefined());
    const sent = boundsCalls().length;

    rect = { x: 20, y: 30, width: 500, height: 400 };
    act(() => resizeObservers.forEach((resize) => resize()));

    await waitFor(() => expect(boundsCalls().length).toBeGreaterThan(sent));
    expect(lastBounds()?.bounds).toEqual({ x: 20, y: 30, width: 500, height: 400 });
  });

  it("sends the new rectangle when the window is resized", async () => {
    render(<PreviewSurface previewId={PREVIEW_ID} name={NAME} />);
    await waitFor(() => expect(lastBounds()).toBeDefined());
    const sent = boundsCalls().length;

    rect = { x: 0, y: 48, width: 900, height: 700 };
    act(() => {
      window.dispatchEvent(new Event("resize"));
    });

    await waitFor(() => expect(boundsCalls().length).toBeGreaterThan(sent));
    expect(lastBounds()?.bounds).toEqual({ x: 0, y: 48, width: 900, height: 700 });
  });
});

describe("PreviewSurface: hidden while something is drawn over the viewer (contract §2)", () => {
  it("is hidden while a ConfirmDialog is open, and shown once it is gone", async () => {
    const user = userEvent.setup();
    renderHarness();
    await waitFor(() => expect(lastBounds()?.visible).toBe(true));

    await user.click(screen.getByRole("button", { name: "Delete" }));
    const confirm = await screen.findByRole("alertdialog");
    await waitFor(() => expect(lastBounds()?.visible).toBe(false));

    await user.click(within(confirm).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    await waitFor(() => expect(lastBounds()?.visible).toBe(true));
  });

  it("is hidden while a Menu is open, and shown once it is closed", async () => {
    const user = userEvent.setup();
    renderHarness();
    await waitFor(() => expect(lastBounds()?.visible).toBe(true));

    await user.click(screen.getByRole("button", { name: "Actions" }));
    await screen.findByRole("menu");
    await waitFor(() => expect(lastBounds()?.visible).toBe(false));

    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());
    await waitFor(() => expect(lastBounds()?.visible).toBe(true));
  });

  it("is hidden while a toast is showing, and shown once it is dismissed", async () => {
    const user = userEvent.setup();
    renderHarness();
    await waitFor(() => expect(lastBounds()?.visible).toBe(true));

    await user.click(screen.getByRole("button", { name: "Toast" }));
    await screen.findByText("Opened Purchase receipt.pdf in another app.");
    await waitFor(() => expect(lastBounds()?.visible).toBe(false));

    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(lastBounds()?.visible).toBe(true));
  });
});

describe("PreviewSurface: progress and accessibility (contract §2, §8)", () => {
  it("shows Preparing with a spinner, announced politely, until preview:pdf-ready for its id", async () => {
    render(<PreviewSurface previewId={PREVIEW_ID} name={NAME} />);

    const preparing = screen.getByText(`Preparing ${NAME}…`);
    const live = preparing.closest('[aria-live="polite"]');
    expect(live).not.toBeNull();
    expect(live!.querySelector(".hd-spinner")).not.toBeNull();

    // Another preview's event is not ours.
    act(() => backend.emit("preview:pdf-ready", { previewId: PREVIEW_ID + 1 }));
    expect(screen.getByText(`Preparing ${NAME}…`)).toBeInTheDocument();

    act(() => backend.emit("preview:pdf-ready", { previewId: PREVIEW_ID }));
    await waitFor(() => expect(screen.queryByText(`Preparing ${NAME}…`)).not.toBeInTheDocument());
  });

  it("is a region named for the document, with the F6 hint visually hidden inside it", () => {
    render(<PreviewSurface previewId={PREVIEW_ID} name={NAME} />);

    const region = screen.getByRole("region", { name: `${NAME}, PDF` });
    const hint = within(region).getByText(
      "Press F6 to move into the document, and F6 again to come back.",
    );
    expect(hint).toHaveClass("hd-sr-only");
  });

  it("stops listening when it unmounts", () => {
    const { unmount } = render(<PreviewSurface previewId={PREVIEW_ID} name={NAME} />);
    expect(backend.listeners("preview:pdf-ready")).toBeGreaterThan(0);

    unmount();

    expect(backend.listeners("preview:pdf-ready")).toBe(0);
  });
});
