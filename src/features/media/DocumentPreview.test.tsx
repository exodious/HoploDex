import { useState } from "react";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, afterAll, beforeEach, describe, expect, it, vi } from "vitest";
import { formatDate } from "../../lib/dates";
import { DocumentPreview } from "./DocumentPreview";
import type { DocumentSummary, DocumentType, PreviewInfo } from "./types";

// specs/007-document-preview/contracts/ui-document-preview.md §2, §3, §7, §8,
// tasks.md T038. The viewer is
//
//   <DocumentPreview documents index documentTypes onIndexChange onClose onDelete
//                    onPdfEnded? />
//
// `documents` is the record's own list in list order and `index` the one shown
// (as PhotoViewer). It opens the preview itself (`open_preview` with
// { documentId }), and calls `onDelete(document)` for "Delete document": the
// list owns the ConfirmDialog and what happens after (DocumentList.test.tsx).
// `onPdfEnded(reason)` lets the list reload after `copyCaught` and `noViewer`.
// The backend is pinned at the IPC boundary (`invoke`, `listen`), by the
// command names and argument objects of contracts/tauri-commands.md. The
// scroll area's size is read from the element (jsdom has none, so it is
// stubbed below).
//
// "Open in another app…" is not asserted present here: tasks.md T079 leaves it
// to US2 (T085), so only its absence (DOCUMENT_CONTENT_MISMATCH) is pinned.

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

import { CommandFailure } from "../../services/tauriClient";

const types: DocumentType[] = [
  { label: "PDF", extensions: ["pdf"], mimeType: "application/pdf", previewKind: "pdf" },
  { label: "TIFF", extensions: ["tif", "tiff"], mimeType: "image/tiff", previewKind: "tiff" },
  { label: "Plain text", extensions: ["txt"], mimeType: "text/plain", previewKind: "text" },
  { label: "CSV", extensions: ["csv"], mimeType: "text/csv", previewKind: "text" },
  {
    label: "Word",
    extensions: ["docx"],
    mimeType: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    previewKind: null,
  },
];

function doc(
  id: number,
  originalFilename: string,
  mimeType: string,
  previewKind: DocumentSummary["previewKind"],
): DocumentSummary {
  return {
    id,
    owner: { kind: "firearm", id: 1 },
    originalFilename,
    mimeType,
    createdAt: "2026-03-14 09:30:00",
    previewKind,
    previewAvailable: true,
    openable: true,
  };
}

const receipt = doc(1, "Purchase receipt.pdf", "application/pdf", "pdf");
const scan = doc(2, "Appraisal scan.tif", "image/tiff", "tiff");
const notes = doc(3, "Notes.txt", "text/plain", "text");
const rounds = doc(4, "Round count.csv", "text/csv", "text");
const bill = doc(5, "Bill of sale.docx", types[4].mimeType, null);
const single = doc(6, "One page.tif", "image/tiff", "tiff");
const documents = [receipt, scan, notes, rounds, bill];
const ADDED = formatDate("2026-03-14");

const PAGE: { width: number; height: number } = { width: 612, height: 792 };
const infos: Record<number, PreviewInfo> = {
  1: { previewId: 11, documentId: 1, kind: "pdf" },
  2: { previewId: 12, documentId: 2, kind: "tiff", pages: [PAGE, PAGE, PAGE] },
  3: { previewId: 13, documentId: 3, kind: "text", text: "Field strip the pistol." },
  4: {
    previewId: 14,
    documentId: 4,
    kind: "text",
    text: "=SUM(A1)\n<b>bold</b>\nhttps://example.com\na,b\n1,2",
  },
  6: { previewId: 16, documentId: 6, kind: "tiff", pages: [PAGE] },
};

type Handler = (args: Record<string, unknown>) => unknown;
let created: string[];
let revoked: string[];

function calls(command: string): Record<string, unknown>[] {
  return backend.invoke.mock.calls
    .filter(([name]) => name === command)
    .map(([, args]) => args as Record<string, unknown>);
}

/** Installs the backend: the open previews of `infos`, answers to everything
 * else, and `overrides` for whatever a test changes. */
function installBackend(overrides: Record<string, Handler> = {}) {
  backend.invoke.mockReset().mockImplementation(async (command: string, args = {}) => {
    const handler = overrides[command];
    if (handler) return handler(args);
    switch (command) {
      case "open_preview":
        return infos[args.documentId as number];
      case "render_preview_page":
        return new ArrayBuffer(8);
      case "close_preview":
      case "set_preview_bounds":
      case "focus_preview":
        return null;
    }
    throw new Error(`unexpected command ${command}`);
  });
}

function failure(code: string, message: string): CommandFailure {
  return new CommandFailure({ code, message });
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

/** The viewer opened from a button, as the list opens it. */
function Host({
  docs = documents,
  start = 0,
  onClose = () => {},
  onDelete = () => {},
}: {
  docs?: DocumentSummary[];
  start?: number;
  onClose?: () => void;
  onDelete?: (doc: DocumentSummary) => void;
}) {
  const [index, setIndex] = useState<number | null>(null);
  return (
    <>
      <button type="button" onClick={() => setIndex(start)}>
        Open viewer
      </button>
      {index !== null && (
        <DocumentPreview
          documents={docs}
          index={index}
          documentTypes={types}
          onIndexChange={setIndex}
          onClose={() => {
            setIndex(null);
            onClose();
          }}
          onDelete={onDelete}
        />
      )}
    </>
  );
}

async function openViewer(props: Parameters<typeof Host>[0] = {}) {
  const user = userEvent.setup();
  const view = render(<Host {...props} />);
  await user.click(screen.getByRole("button", { name: "Open viewer" }));
  return { user, ...view };
}

const viewer = () => screen.getByRole("dialog");
const titleOf = () => within(viewer()).getByRole("heading").textContent;

/** The text of every `role="status"` region. */
const statuses = () => screen.queryAllByRole("status").map((el) => el.textContent ?? "");

beforeAll(() => {
  // jsdom lays nothing out: the scroll area has a size, as in a real window.
  Object.defineProperty(HTMLElement.prototype, "clientWidth", {
    configurable: true,
    get: () => 1000,
  });
  Object.defineProperty(HTMLElement.prototype, "clientHeight", {
    configurable: true,
    get: () => 800,
  });
});
afterAll(() => {
  Reflect.deleteProperty(HTMLElement.prototype, "clientWidth");
  Reflect.deleteProperty(HTMLElement.prototype, "clientHeight");
});

beforeEach(() => {
  backend.reset();
  installBackend();
  created = [];
  revoked = [];
  let next = 0;
  URL.createObjectURL = vi.fn(() => {
    const url = `blob:test/${next++}`;
    created.push(url);
    return url;
  });
  URL.revokeObjectURL = vi.fn((url: string) => {
    revoked.push(url);
  });
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(
    () =>
      ({
        x: 0,
        y: 0,
        left: 0,
        top: 0,
        right: 1000,
        bottom: 800,
        width: 1000,
        height: 800,
        toJSON: () => ({}),
      }) as DOMRect,
  );
});
afterEach(() => vi.restoreAllMocks());

describe("DocumentPreview: the dialog (contract §2, §8)", () => {
  it("is an xl dialog titled with the name, described by kind, date and position", async () => {
    await openViewer();

    const dialog = await screen.findByRole("dialog", { name: "Purchase receipt.pdf" });
    expect(dialog).toHaveClass("hd-dialog__content--xl");
    expect(dialog).toHaveAccessibleDescription(new RegExp(`PDF · added ${ADDED}`));
    expect(dialog).toHaveAccessibleDescription(/Document 1 of 5/);
  });

  it("adds the page count to a multi-page TIFF's description", async () => {
    await openViewer({ start: 1 });

    const dialog = await screen.findByRole("dialog", { name: "Appraisal scan.tif" });
    await waitFor(() => expect(dialog).toHaveAccessibleDescription(/3 pages/));
    expect(dialog).toHaveAccessibleDescription(new RegExp(`TIFF · added ${ADDED}`));
    expect(dialog).toHaveAccessibleDescription(/Document 2 of 5/);
  });

  it("shows Preparing before open_preview answers, and closes on Escape and the close control", async () => {
    const open = deferred<PreviewInfo>();
    installBackend({ open_preview: () => open.promise });
    const onClose = vi.fn();
    const { user } = await openViewer({ onClose });

    expect(await screen.findByText("Preparing Purchase receipt.pdf…")).toBeInTheDocument();
    expect(calls("open_preview")).toEqual([{ documentId: 1 }]);

    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    // The preview it was waiting for is closed as soon as it is known.
    open.resolve(infos[1]);
    await waitFor(() => expect(calls("close_preview")).toEqual([{ previewId: 11 }]));
  });

  it("closes through the close control while it is still preparing", async () => {
    const open = deferred<PreviewInfo>();
    installBackend({ open_preview: () => open.promise });
    const onClose = vi.fn();
    const { user } = await openViewer({ onClose });
    await screen.findByText("Preparing Purchase receipt.pdf…");

    await user.click(within(viewer()).getByRole("button", { name: "Close" }));

    expect(onClose).toHaveBeenCalledTimes(1);
    open.resolve(infos[1]);
    await waitFor(() => expect(calls("close_preview")).toEqual([{ previewId: 11 }]));
  });
});

describe("DocumentPreview: closing and focus (US1-6, US1-7)", () => {
  async function closeBy(
    how: "escape" | "control" | "event",
    user: ReturnType<typeof userEvent.setup>,
  ) {
    if (how === "escape") await user.keyboard("{Escape}");
    else if (how === "control") await user.click(screen.getByRole("button", { name: "Close" }));
    else act(() => backend.emit("preview:escape", { previewId: 11 }));
  }

  it.each(["escape", "control", "event"] as const)(
    "closes on %s, calls close_preview and returns focus to the opener",
    async (how) => {
      const onClose = vi.fn();
      const { user } = await openViewer({ onClose });
      await screen.findByRole("region", { name: "Purchase receipt.pdf, PDF" });
      const opener = screen.getByRole("button", { name: "Open viewer", hidden: true });

      await closeBy(how, user);

      await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
      expect(onClose).toHaveBeenCalledTimes(1);
      expect(calls("close_preview")).toEqual([{ previewId: 11 }]);
      await waitFor(() => expect(opener).toHaveFocus());
    },
  );

  it("ignores another preview's preview:escape", async () => {
    await openViewer();
    await screen.findByRole("region", { name: "Purchase receipt.pdf, PDF" });

    act(() => backend.emit("preview:escape", { previewId: 99 }));

    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(calls("close_preview")).toEqual([]);
  });

  it("moves to the next and previous document of the record, with the buttons and with the arrow keys", async () => {
    const { user } = await openViewer();
    await screen.findByRole("region", { name: "Purchase receipt.pdf, PDF" });
    const previous = () => within(viewer()).getByRole("button", { name: "Previous document" });
    const next = () => within(viewer()).getByRole("button", { name: "Next document" });
    expect(previous()).toBeDisabled();
    expect(next()).toBeEnabled();

    await user.click(next());
    await waitFor(() => expect(titleOf()).toBe("Appraisal scan.tif"));
    expect(calls("open_preview")).toEqual([{ documentId: 1 }, { documentId: 2 }]);

    within(viewer()).getByRole("button", { name: "Delete document" }).focus();
    await user.keyboard("{ArrowRight}");
    await waitFor(() => expect(titleOf()).toBe("Notes.txt"));
    await user.keyboard("{ArrowLeft}");
    await waitFor(() => expect(titleOf()).toBe("Appraisal scan.tif"));
    await user.click(previous());
    await waitFor(() => expect(titleOf()).toBe("Purchase receipt.pdf"));

    // The first document has nothing before it.
    within(viewer()).getByRole("button", { name: "Delete document" }).focus();
    await user.keyboard("{ArrowLeft}");
    expect(titleOf()).toBe("Purchase receipt.pdf");
  });

  it("stops at the last document", async () => {
    await openViewer({ start: 4 });
    await screen.findByRole("dialog", { name: "Bill of sale.docx" });

    expect(within(viewer()).getByRole("button", { name: "Next document" })).toBeDisabled();
    expect(within(viewer()).getByRole("button", { name: "Previous document" })).toBeEnabled();
  });

  it("leaves the next preview to open_preview: moving on closes nothing itself", async () => {
    // research.md §4: a PDF followed by a PDF keeps the surface, so only the
    // backend's open_preview replaces a preview; close_preview is for the end.
    const { user } = await openViewer();
    await screen.findByRole("region", { name: "Purchase receipt.pdf, PDF" });

    await user.click(within(viewer()).getByRole("button", { name: "Next document" }));
    await waitFor(() => expect(titleOf()).toBe("Appraisal scan.tif"));

    expect(calls("close_preview")).toEqual([]);
  });

  it("calls onDelete with the document shown", async () => {
    const onDelete = vi.fn();
    const { user } = await openViewer({ onDelete, start: 2 });
    await screen.findByText("Field strip the pistol.");

    await user.click(within(viewer()).getByRole("button", { name: "Delete document" }));

    expect(onDelete).toHaveBeenCalledWith(notes);
  });
});

describe("DocumentPreview: a PDF (contract §2, §7)", () => {
  it("shows the surface's placeholder, and F6 gives the surface the focus", async () => {
    const { user } = await openViewer();
    await screen.findByRole("region", { name: "Purchase receipt.pdf, PDF" });
    within(viewer()).getByRole("button", { name: "Next document" }).focus();

    await user.keyboard("{F6}");

    expect(calls("focus_preview")).toEqual([{ previewId: 11 }]);
  });

  it("puts the focus on the first control when preview:focus-chrome arrives", async () => {
    await openViewer();
    await screen.findByRole("region", { name: "Purchase receipt.pdf, PDF" });

    act(() => backend.emit("preview:focus-chrome", { previewId: 11 }));

    const first = within(viewer())
      .getAllByRole("button")
      .find((button) => !(button as HTMLButtonElement).disabled);
    await waitFor(() => expect(first).toHaveFocus());
  });

  it("has no toolbar and keeps a status line in the footer for what a toast would say", async () => {
    await openViewer();
    await screen.findByRole("region", { name: "Purchase receipt.pdf, PDF" });

    expect(within(viewer()).queryByRole("button", { name: "Zoom in" })).not.toBeInTheDocument();
    const footer = viewer().querySelector("footer");
    expect(footer?.querySelector('[role="status"]')).not.toBeNull();
  });
});

describe("DocumentPreview: a multi-page TIFF (contract §2, §3, §7, §8)", () => {
  const TOOLBAR = [
    "First page",
    "Previous page",
    "Next page",
    "Last page",
    "Zoom out",
    "Zoom in",
    "Fit width",
    "Fit page",
  ];

  it("has its toolbar, disabled until page 1 arrives, with a placeholder saying so", async () => {
    const first = deferred<ArrayBuffer>();
    installBackend({
      render_preview_page: (args) => (args.page === 0 ? first.promise : new Promise(() => {})),
    });
    await openViewer({ start: 1 });
    await screen.findByRole("dialog", { name: "Appraisal scan.tif" });

    const preparing = await screen.findByText("Preparing page 1…");
    expect(preparing.closest('[aria-live="polite"]')).not.toBeNull();
    for (const name of TOOLBAR) {
      expect(within(viewer()).getByRole("button", { name })).toBeDisabled();
    }
    expect(within(viewer()).getByRole("button", { name: /\d+%/ })).toBeDisabled();

    await act(async () => first.resolve(new ArrayBuffer(8)));

    await waitFor(() =>
      expect(within(viewer()).getByRole("button", { name: "Zoom in" })).toBeEnabled(),
    );
    expect(within(viewer()).getByRole("button", { name: "Next page" })).toBeEnabled();
    expect(within(viewer()).getByRole("button", { name: "Previous page" })).toBeDisabled();
    expect(screen.queryByText("Preparing page 1…")).not.toBeInTheDocument();
  });

  it("shows Page n of count, announced politely, and moves with the buttons", async () => {
    const { user } = await openViewer({ start: 1 });
    await screen.findByAltText("Appraisal scan.tif, page 1 of 3");
    expect(screen.getByText("Page 1 of 3")).toHaveAttribute("aria-live", "polite");

    await user.click(within(viewer()).getByRole("button", { name: "Next page" }));
    expect(screen.getByText("Page 2 of 3")).toBeInTheDocument();
    await user.click(within(viewer()).getByRole("button", { name: "Last page" }));
    expect(screen.getByText("Page 3 of 3")).toBeInTheDocument();
    await user.click(within(viewer()).getByRole("button", { name: "Previous page" }));
    expect(screen.getByText("Page 2 of 3")).toBeInTheDocument();
    await user.click(within(viewer()).getByRole("button", { name: "First page" }));
    expect(screen.getByText("Page 1 of 3")).toBeInTheDocument();
  });

  it("offers 50 to 400% and the two fits in the percent menu", async () => {
    const { user } = await openViewer({ start: 1 });
    await screen.findByAltText("Appraisal scan.tif, page 1 of 3");

    await user.click(within(viewer()).getByRole("button", { name: /\d+%/ }));

    const menu = await screen.findByRole("menu");
    const items = within(menu).getAllByRole("menuitemradio");
    const labels = items.map((item) => item.textContent);
    expect(labels).toContain("50%");
    expect(labels).toContain("100%");
    expect(labels).toContain("400%");
    expect(labels).toContain("Fit width");
    expect(labels).toContain("Fit page");

    await user.click(within(menu).getByRole("menuitemradio", { name: "200%" }));
    await waitFor(() =>
      expect(within(viewer()).getByRole("button", { name: /\d+%/ })).toHaveTextContent("200%"),
    );
  });

  it("scrolls and zooms from the page area's keys, which is focusable and labelled", async () => {
    const { user } = await openViewer({ start: 1 });
    await screen.findByAltText("Appraisal scan.tif, page 1 of 3");
    const area = screen.getByLabelText("Appraisal scan.tif, pages");
    expect(area).toHaveAttribute("tabindex", "0");
    const percent = () => within(viewer()).getByRole("button", { name: /\d+%/ });
    await user.click(percent());
    await user.click(await screen.findByRole("menuitemradio", { name: "100%" }));
    area.focus();

    await user.keyboard("{PageDown}");
    expect(screen.getByText("Page 2 of 3")).toBeInTheDocument();
    await user.keyboard("{End}");
    expect(screen.getByText("Page 3 of 3")).toBeInTheDocument();
    await user.keyboard("{PageUp}");
    expect(screen.getByText("Page 2 of 3")).toBeInTheDocument();
    await user.keyboard("{Home}");
    expect(screen.getByText("Page 1 of 3")).toBeInTheDocument();

    await user.keyboard("+");
    expect(percent()).toHaveTextContent("125%");
    await user.keyboard("-");
    expect(percent()).toHaveTextContent("100%");

    // ← and → scroll sideways here; they don't change the document.
    await user.keyboard("{ArrowRight}");
    expect(titleOf()).toBe("Appraisal scan.tif");

    await user.keyboard("0");
    await user.click(percent());
    expect(await screen.findByRole("menuitemradio", { name: "Fit width" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });

  it("gives each page image its alt text", async () => {
    await openViewer({ start: 1 });

    expect(await screen.findByAltText("Appraisal scan.tif, page 1 of 3")).toBeInTheDocument();
    expect(await screen.findByAltText("Appraisal scan.tif, page 2 of 3")).toBeInTheDocument();
  });

  it("shows a page that can't be rendered in its own slot and the others as usual", async () => {
    installBackend({
      render_preview_page: (args) => {
        if (args.page === 0) throw failure("PREVIEW_PAGE_FAILED", "Page 1 failed.");
        return new ArrayBuffer(8);
      },
    });
    await openViewer({ start: 1 });

    expect(await screen.findByText("Page 1 can't be shown.")).toBeInTheDocument();
    expect(await screen.findByAltText("Appraisal scan.tif, page 2 of 3")).toBeInTheDocument();
  });

  it("revokes every blob URL and closes the preview when it unmounts", async () => {
    const { unmount } = await openViewer({ start: 1 });
    await screen.findByAltText("Appraisal scan.tif, page 2 of 3");
    expect(created.length).toBeGreaterThan(0);

    unmount();

    expect([...revoked].sort()).toEqual([...created].sort());
    expect(calls("close_preview")).toEqual([{ previewId: 12 }]);
  });
});

describe("DocumentPreview: a single-page TIFF (contract §2)", () => {
  it("has zoom out, percent, zoom in, fit and actual size, and an image with just the name", async () => {
    const { user } = await openViewer({ docs: [single], start: 0 });

    expect(await screen.findByAltText("One page.tif")).toBeInTheDocument();
    for (const name of ["Zoom out", "Zoom in", "Fit", "Actual size"]) {
      expect(within(viewer()).getByRole("button", { name })).toBeInTheDocument();
    }
    expect(within(viewer()).getByRole("button", { name: /\d+%/ })).toBeInTheDocument();
    expect(within(viewer()).queryByRole("button", { name: "First page" })).not.toBeInTheDocument();
    expect(screen.queryByText(/^Page \d+ of \d+$/)).not.toBeInTheDocument();

    await user.click(within(viewer()).getByRole("button", { name: "Actual size" }));
    expect(within(viewer()).getByRole("button", { name: /\d+%/ })).toHaveTextContent("100%");
  });

  it("zooms with + and −, fits with 0 and shows actual size with 1", async () => {
    const { user } = await openViewer({ docs: [single], start: 0 });
    await screen.findByAltText("One page.tif");
    const percent = () => within(viewer()).getByRole("button", { name: /\d+%/ });
    screen.getByLabelText("One page.tif, pages").focus();

    await user.keyboard("1");
    expect(percent()).toHaveTextContent("100%");
    await user.keyboard("+");
    expect(percent()).toHaveTextContent("125%");
    await user.keyboard("-");
    expect(percent()).toHaveTextContent("100%");
    await user.keyboard("0");
    expect(percent()).not.toHaveTextContent("100%");
  });
});

describe("DocumentPreview: text (contract §2)", () => {
  it("shows text and CSV as a text child of a pre, with nothing made of it", async () => {
    await openViewer({ start: 3 });

    await screen.findByRole("dialog", { name: "Round count.csv" });
    const pre = await waitFor(() => {
      const found = viewer().querySelector("pre.hd-preview__text");
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    expect(pre.textContent).toBe("=SUM(A1)\n<b>bold</b>\nhttps://example.com\na,b\n1,2");
    expect(pre.children).toHaveLength(0);
    expect(viewer().querySelector("a, table, b")).toBeNull();
    expect(pre).toHaveAttribute("aria-label", "Round count.csv");
    expect(pre).toHaveAttribute("tabindex", "0");
  });
});

describe("DocumentPreview: states (contract §3)", () => {
  const cases: [string, () => void, string | RegExp][] = [
    [
      "PREVIEW_UNSUPPORTED",
      () =>
        installBackend({
          open_preview: () => {
            throw failure("PREVIEW_UNSUPPORTED", "x");
          },
        }),
      "Bill of sale.docx can't be previewed here. Word documents open in another app.",
    ],
    [
      "PDF_PREVIEW_UNAVAILABLE",
      () =>
        installBackend({
          open_preview: () => {
            throw failure(
              "PDF_PREVIEW_UNAVAILABLE",
              "This computer's PDF viewer couldn't be set up safely.",
            );
          },
        }),
      "PDFs can't be previewed on this computer. This computer's PDF viewer couldn't be set up safely.",
    ],
    [
      "PREVIEW_FAILED",
      () =>
        installBackend({
          open_preview: () => {
            throw failure("PREVIEW_FAILED", "x");
          },
        }),
      "Bill of sale.docx couldn't be previewed.",
    ],
    [
      "DOCUMENT_CONTENT_MISMATCH",
      () =>
        installBackend({
          open_preview: () => {
            throw failure("DOCUMENT_CONTENT_MISMATCH", "x");
          },
        }),
      "Bill of sale.docx can't be previewed: its content isn't a Word document.",
    ],
    [
      "PREVIEW_DAMAGED",
      () =>
        installBackend({
          open_preview: () => {
            throw failure("PREVIEW_DAMAGED", "x");
          },
        }),
      "Bill of sale.docx can't be previewed: the document is damaged or incomplete.",
    ],
  ];

  it.each(cases)(
    "%s says so in a status region, and previous and next stay usable",
    async (_code, install, sentence) => {
      install();
      await openViewer({ start: 4 });

      await waitFor(() => expect(statuses()).toContain(sentence));
      expect(within(viewer()).getByRole("button", { name: "Previous document" })).toBeEnabled();
      expect(within(viewer()).queryByText(/^Preparing/)).not.toBeInTheDocument();
    },
  );

  it("DOCUMENT_CONTENT_MISMATCH offers no Open in another app… (FR-017)", async () => {
    installBackend({
      open_preview: () => {
        throw failure("DOCUMENT_CONTENT_MISMATCH", "x");
      },
    });
    await openViewer({ start: 4 });

    await waitFor(() =>
      expect(statuses().join(" ")).toContain("its content isn't a Word document"),
    );
    expect(
      within(viewer()).queryByRole("button", { name: "Open in another app…" }),
    ).not.toBeInTheDocument();
  });

  it.each([
    ["noViewer", /^PDFs can't be previewed on this computer\./],
    [
      "copyCaught",
      "This computer's PDF viewer saved a copy of Purchase receipt.pdf to disk. HoploDex deleted it and has turned PDF previews off on this computer until HoploDex is updated. You can still open PDFs in another app.",
    ],
    ["failed", "Purchase receipt.pdf couldn't be previewed."],
  ] as const)(
    "preview:pdf-ended %s replaces the surface with its sentence",
    async (reason, sentence) => {
      await openViewer();
      await screen.findByRole("region", { name: "Purchase receipt.pdf, PDF" });

      act(() => backend.emit("preview:pdf-ended", { previewId: 11, reason }));

      await waitFor(() =>
        expect(
          statuses().some((text) =>
            typeof sentence === "string" ? text === sentence : sentence.test(text),
          ),
        ).toBe(true),
      );
      expect(
        screen.queryByRole("region", { name: "Purchase receipt.pdf, PDF" }),
      ).not.toBeInTheDocument();
      expect(within(viewer()).getByRole("button", { name: "Next document" })).toBeEnabled();
    },
  );

  it("ignores another preview's preview:pdf-ended", async () => {
    await openViewer();
    await screen.findByRole("region", { name: "Purchase receipt.pdf, PDF" });

    act(() => backend.emit("preview:pdf-ended", { previewId: 99, reason: "failed" }));

    expect(screen.getByRole("region", { name: "Purchase receipt.pdf, PDF" })).toBeInTheDocument();
  });
});
