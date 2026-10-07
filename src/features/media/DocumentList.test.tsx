import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, afterAll, beforeEach, describe, expect, it, vi } from "vitest";
import { formatDate } from "../../lib/dates";
import { DocumentList } from "./DocumentList";
import type { DocumentSummary, DocumentType, PreviewInfo } from "./types";

// specs/007-document-preview/contracts/ui-document-preview.md §1 and §2,
// tasks.md T041 (User Story 1). The list is `<DocumentList owner />` and hosts
// the viewer over the record. The backend is pinned at the IPC boundary
// (`invoke`, `listen`) with the command names of contracts/tauri-commands.md.

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

// The label of the PDF type is made up, to show the list takes it from
// list_document_types and not from a table of its own.
const types: DocumentType[] = [
  {
    label: "Portable document",
    extensions: ["pdf"],
    mimeType: "application/pdf",
    previewKind: "pdf",
  },
  { label: "TIFF", extensions: ["tif", "tiff"], mimeType: "image/tiff", previewKind: "tiff" },
  { label: "Plain text", extensions: ["txt"], mimeType: "text/plain", previewKind: "text" },
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
  extra: Partial<DocumentSummary> = {},
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
    ...extra,
  };
}

const ADDED = formatDate("2026-03-14");
const receipt = () => doc(1, "Purchase receipt.pdf", "application/pdf", "pdf");
const scan = () => doc(2, "Appraisal scan.tif", "image/tiff", "tiff");
const notes = () => doc(3, "Notes.txt", "text/plain", "text");
const billDocx = () => doc(4, "Bill of sale.docx", types[3].mimeType, null);
const oldScan = () => doc(5, "Old scan.jpg", "image/jpeg", null, { openable: false });

let docs: DocumentSummary[];

function calls(command: string): Record<string, unknown>[] {
  return backend.invoke.mock.calls
    .filter(([name]) => name === command)
    .map(([, args]) => args as Record<string, unknown>);
}

function previewOf(document: DocumentSummary): PreviewInfo {
  const base = { previewId: 100 + document.id, documentId: document.id };
  if (document.previewKind === "pdf") return { ...base, kind: "pdf" };
  if (document.previewKind === "tiff") {
    return { ...base, kind: "tiff", pages: [{ width: 612, height: 792 }] };
  }
  return { ...base, kind: "text", text: `Contents of ${document.originalFilename}` };
}

beforeAll(() => {
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
  docs = [receipt(), scan(), notes(), billDocx(), oldScan()];
  backend.invoke.mockReset().mockImplementation(async (command: string, args = {}) => {
    switch (command) {
      case "list_documents":
        return docs.map((d) => ({ ...d }));
      case "list_document_types":
        return types;
      case "open_preview": {
        const document = docs.find((d) => d.id === args.documentId)!;
        if (document.previewKind === null) {
          throw new CommandFailure({ code: "PREVIEW_UNSUPPORTED", message: "Not previewed." });
        }
        return previewOf(document);
      }
      case "delete_document":
        docs = docs.filter((d) => d.id !== args.id);
        return { deleted: true };
      case "render_preview_page":
        return new ArrayBuffer(8);
      case "close_preview":
      case "set_preview_bounds":
      case "focus_preview":
        return null;
    }
    throw new Error(`unexpected command ${command}`);
  });
  URL.createObjectURL = vi.fn(() => "blob:test/1");
  URL.revokeObjectURL = vi.fn();
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

async function renderList() {
  const user = userEvent.setup();
  render(<DocumentList owner={{ kind: "firearm", id: 1 }} />);
  await screen.findByRole("button", { name: "Purchase receipt.pdf" });
  return user;
}

/** The list item for the document named `name`. */
const row = (name: string) => screen.getByRole("button", { name }).closest("li") as HTMLElement;

describe("DocumentList: kind labels and the meta line (contract §1)", () => {
  it("takes the kind label from list_document_types, and a pre-007 row's from its extension", async () => {
    await renderList();

    expect(row("Purchase receipt.pdf")).toHaveTextContent(`Portable document · added ${ADDED}`);
    expect(row("Appraisal scan.tif")).toHaveTextContent(`TIFF · added ${ADDED}`);
    expect(row("Old scan.jpg")).toHaveTextContent(`JPG · added ${ADDED}`);
  });

  it("says a document with no preview opens in another app", async () => {
    await renderList();

    expect(row("Bill of sale.docx")).toHaveTextContent(
      `Word · added ${ADDED} · opens in another app`,
    );
    expect(row("Purchase receipt.pdf")).not.toHaveTextContent("opens in another app");
  });

  it("says a PDF can't be previewed on this computer while PDF preview is off", async () => {
    docs = [{ ...receipt(), previewAvailable: false }, scan()];
    await renderList();

    expect(row("Purchase receipt.pdf")).toHaveTextContent(
      `Portable document · added ${ADDED} · can't be previewed on this computer`,
    );
    expect(row("Appraisal scan.tif")).not.toHaveTextContent("can't be previewed");
  });

  it("says a row that isn't a document type can't be opened, and not that it opens elsewhere", async () => {
    await renderList();

    const meta = row("Old scan.jpg");
    expect(meta).toHaveTextContent("· can't be opened: not a document type");
    expect(meta).not.toHaveTextContent("opens in another app");
  });
});

describe("DocumentList: the Preview button and the name (contract §1)", () => {
  it("shows a ghost Preview button for every previewable kind, and none for the rest", async () => {
    await renderList();

    for (const name of ["Purchase receipt.pdf", "Appraisal scan.tif", "Notes.txt"]) {
      const button = within(row(name)).getByRole("button", { name: "Preview" });
      expect(button).toBeEnabled();
      expect(button).toHaveClass("hd-button--ghost");
    }
    expect(within(row("Bill of sale.docx")).queryByRole("button", { name: "Preview" })).toBeNull();
    expect(within(row("Old scan.jpg")).queryByRole("button", { name: "Preview" })).toBeNull();
  });

  it("disables Preview for a PDF while PDF preview is off, with the reason as its title", async () => {
    docs = [{ ...receipt(), previewAvailable: false }];
    await renderList();

    const button = within(row("Purchase receipt.pdf")).getByRole("button", { name: "Preview" });
    expect(button).toBeDisabled();
    expect(button.getAttribute("title")).toMatch(/this computer/i);
  });

  it("titles the name for what it does, and opens the viewer on it", async () => {
    const user = await renderList();
    const name = screen.getByRole("button", { name: "Purchase receipt.pdf" });
    expect(name).toHaveAttribute("title", "Preview Purchase receipt.pdf");

    await user.click(name);

    const dialog = await screen.findByRole("dialog", { name: "Purchase receipt.pdf" });
    expect(calls("open_preview")).toEqual([{ documentId: 1 }]);
    expect(dialog).toHaveAccessibleDescription(/Document 1 of 5/);
  });

  it("opens the viewer from the Preview button too, on the document it belongs to", async () => {
    const user = await renderList();

    await user.click(within(row("Notes.txt")).getByRole("button", { name: "Preview" }));

    const dialog = await screen.findByRole("dialog", { name: "Notes.txt" });
    expect(dialog).toHaveAccessibleDescription(/Document 3 of 5/);
    expect(await screen.findByText("Contents of Notes.txt")).toBeInTheDocument();
  });

  it("opens the viewer on a document that can't be previewed, on the matching state", async () => {
    const user = await renderList();

    await user.click(screen.getByRole("button", { name: "Bill of sale.docx" }));

    await screen.findByRole("dialog", { name: "Bill of sale.docx" });
    await waitFor(() =>
      expect(screen.getAllByRole("status").map((el) => el.textContent)).toContain(
        "Bill of sale.docx can't be previewed here. Word documents open in another app.",
      ),
    );
  });
});

describe("DocumentList: after the PDF preview ends (contracts/tauri-commands.md events)", () => {
  async function openReceipt() {
    const user = await renderList();
    await user.click(screen.getByRole("button", { name: "Purchase receipt.pdf" }));
    await screen.findByRole("region", { name: "Purchase receipt.pdf, PDF" });
    return user;
  }

  it.each(["copyCaught", "noViewer"] as const)(
    "reloads the list on preview:pdf-ended %s, so PDFs show as not previewable",
    async (reason) => {
      await openReceipt();
      expect(calls("list_documents")).toHaveLength(1);
      docs = docs.map((d) => (d.previewKind === "pdf" ? { ...d, previewAvailable: false } : d));

      act(() => backend.emit("preview:pdf-ended", { previewId: 101, reason }));

      await waitFor(() => expect(calls("list_documents")).toHaveLength(2));
    },
  );

  it("doesn't reload for a PDF that merely failed", async () => {
    await openReceipt();

    act(() => backend.emit("preview:pdf-ended", { previewId: 101, reason: "failed" }));
    await act(async () => {});

    expect(calls("list_documents")).toHaveLength(1);
  });
});

describe("DocumentList: deleting from the viewer (contract §2 Footer)", () => {
  async function deleteFromViewer(name: string) {
    const user = await renderList();
    await user.click(screen.getByRole("button", { name }));
    await screen.findByRole("dialog", { name });
    await user.click(
      within(screen.getByRole("dialog", { name })).getByRole("button", { name: "Delete document" }),
    );
    const confirm = await screen.findByRole("alertdialog", { name: "Delete this document?" });
    await user.click(within(confirm).getByRole("button", { name: "Delete document" }));
    return user;
  }

  it("moves to the next document after deleting one in the middle", async () => {
    await deleteFromViewer("Appraisal scan.tif");

    expect(calls("delete_document")).toEqual([{ id: 2, confirmed: true }]);
    expect(await screen.findByRole("dialog", { name: "Notes.txt" })).toHaveAccessibleDescription(
      /Document 2 of 4/,
    );
  });

  it("moves to the previous document after deleting the last", async () => {
    docs = [receipt(), scan(), notes()];
    await deleteFromViewer("Notes.txt");

    expect(calls("delete_document")).toEqual([{ id: 3, confirmed: true }]);
    expect(await screen.findByRole("dialog", { name: "Appraisal scan.tif" })).toBeInTheDocument();
  });

  it("closes the viewer after deleting the only document", async () => {
    docs = [receipt()];
    await deleteFromViewer("Purchase receipt.pdf");

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(calls("delete_document")).toEqual([{ id: 1, confirmed: true }]);
  });
});
