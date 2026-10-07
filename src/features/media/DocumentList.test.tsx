import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, afterAll, beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../../components";
import { formatDate } from "../../lib/dates";
import type { FileDropEvent } from "../../services/tauriClient";
import { DocumentList } from "./DocumentList";
import { chooseDocumentOpening, loadDocumentOpening, useDocumentOpening } from "./documentOpening";
import type { DocumentSummary, DocumentType, PreviewInfo } from "./types";

// specs/007-document-preview/contracts/ui-document-preview.md §1 and §2,
// tasks.md T041 (User Story 1). The list is `<DocumentList owner />` and hosts
// the viewer over the record. The backend is pinned at the IPC boundary
// (`invoke`, `listen`) with the command names of contracts/tauri-commands.md.
// The US2 part (T084) adds the picker's `accept`, drops (`listenForFileDrops`,
// routed by ui contract §6), "Open in another app…" and its results; toasts
// are read through a ToastProvider around the list. The US3 part (T097) adds
// the name following this computer's setting, held in the store of
// documentOpening.ts: `loadDocumentOpening()` reads it with
// `get_document_opening`, `chooseDocumentOpening(value)` sends
// `set_document_opening` `{ value }` and resolves with `changed`, and
// `useDocumentOpening()` is the value a component follows.

const backend = vi.hoisted(() => {
  const handlers = new Map<string, Set<(payload: unknown) => void>>();
  const drops = new Set<(event: FileDropEvent) => void>();
  return {
    invoke: vi.fn(),
    listenForFileDrops: vi.fn((handler: (event: FileDropEvent) => void) => {
      drops.add(handler);
      return () => drops.delete(handler);
    }),
    drop(paths: string[]) {
      drops.forEach((handler) => handler({ type: "drop", paths }));
    },
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
      drops.clear();
    },
  };
});
vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  invoke: backend.invoke,
  listen: backend.listen,
  listenForFileDrops: backend.listenForFileDrops,
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

/** What `get_document_opening` answers, and what `set_document_opening` sets. */
let opening: "preview" | "external";
/** What `set_document_opening` answers with `changed`, for a test to change
 * (false: the user cancelled the native dialog). */
let settingChanges: boolean;

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

beforeEach(async () => {
  backend.reset();
  docs = [receipt(), scan(), notes(), billDocx(), oldScan()];
  openAnswer = () => ({ opened: true });
  opening = "preview";
  settingChanges = true;
  backend.invoke.mockReset().mockImplementation(async (command: string, args = {}) => {
    switch (command) {
      case "get_document_opening":
        return opening;
      case "set_document_opening":
        if (settingChanges) opening = args.value as typeof opening;
        return { changed: settingChanges };
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
      case "open_document":
        return openAnswer(args);
      case "delete_document":
        docs = docs.filter((d) => d.id !== args.id);
        return { deleted: true };
      case "render_preview_page":
        return new ArrayBuffer(8);
      case "add_document":
        return doc(9, args.originalFilename as string, "application/pdf", "pdf");
      case "add_document_from_path":
        return doc(9, (args.path as string).split("/").pop()!, "application/pdf", "pdf");
      case "close_preview":
      case "set_preview_bounds":
      case "focus_preview":
        return null;
    }
    throw new Error(`unexpected command ${command}`);
  });
  // The store is shared by the whole module: every test starts from "preview".
  await loadDocumentOpening();
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
  render(
    <ToastProvider>
      <DocumentList owner={{ kind: "firearm", id: 1 }} />
    </ToastProvider>,
  );
  await screen.findByRole("button", { name: "Purchase receipt.pdf" });
  return user;
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

/** What the backend answers `open_document` with, for a test to change. */
let openAnswer: (args: Record<string, unknown>) => unknown;

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

// --- User Story 2 (T084) ------------------------------------------------------

const REFUSED =
  "wasn't attached. Documents can be PDF, TIFF, text, CSV, RTF, Word, spreadsheet or OpenDocument files.";

/** The toast showing `text`, or fails. */
async function toastWith(text: string | RegExp): Promise<HTMLElement> {
  return (await screen.findByText(text)).closest(".hd-toast") as HTMLElement;
}

const noToasts = () => document.querySelector(".hd-toast");

/** The list with its document types loaded, which routing a drop and the
 * picker's `accept` depend on. */
async function renderListWithTypes() {
  const user = await renderList();
  await waitFor(() => expect(calls("list_document_types").length).toBeGreaterThan(0));
  await act(async () => {});
  return user;
}

describe("DocumentList: the picker (contract §6)", () => {
  it("accepts the extensions of list_document_types, in its order, and no others", async () => {
    await renderList();

    await waitFor(() =>
      expect(screen.getByLabelText("Attach documents")).toHaveAttribute(
        "accept",
        ".pdf,.tif,.tiff,.txt,.docx",
      ),
    );
  });

  it("sends a chosen file's bytes and name to add_document", async () => {
    const user = await renderList();

    await user.upload(
      screen.getByLabelText("Attach documents"),
      new File(["%PDF-1.4"], "Receipt two.pdf", { type: "application/pdf" }),
    );

    await waitFor(() => expect(calls("add_document")).toHaveLength(1));
    expect(calls("add_document")[0]).toMatchObject({
      owner: { kind: "firearm", id: 1 },
      originalFilename: "Receipt two.pdf",
    });
    expect(await toastWith("Attached 1 document.")).toBeInTheDocument();
  });

  it("shows '{name} is a photo. Add it under Photos instead.' when the backend refuses a photo", async () => {
    backend.invoke.mockImplementation(async (command: string, args = {}) => {
      if (command === "add_document") {
        throw new CommandFailure({
          code: "DOCUMENT_TYPE_NOT_ALLOWED",
          message: "That isn't a document type HoploDex keeps.",
        });
      }
      if (command === "list_documents") return docs.map((d) => ({ ...d }));
      if (command === "list_document_types") return types;
      throw new Error(`unexpected command ${command} ${JSON.stringify(args)}`);
    });
    // The input's own filter would keep the photo from being chosen at all.
    const user = userEvent.setup({ applyAccept: false });
    render(
      <ToastProvider>
        <DocumentList owner={{ kind: "firearm", id: 1 }} />
      </ToastProvider>,
    );
    await screen.findByRole("button", { name: "Purchase receipt.pdf" });

    await user.upload(
      screen.getByLabelText("Attach documents"),
      new File(["x"], "Range day.jpg", { type: "image/jpeg" }),
    );

    const toast = await toastWith("Range day.jpg is a photo. Add it under Photos instead.");
    expect(toast).toHaveClass("hd-toast--error");
  });

  it("shows the backend's own message for another refusal, and carries on past it", async () => {
    let added = 0;
    backend.invoke.mockImplementation(async (command: string, args = {}) => {
      if (command === "add_document") {
        if ((args.originalFilename as string).endsWith(".bin")) {
          throw new CommandFailure({
            code: "DOCUMENT_CONTENT_MISMATCH",
            message: "data.bin's content isn't a PDF document.",
          });
        }
        added += 1;
        return doc(9, args.originalFilename as string, "application/pdf", "pdf");
      }
      if (command === "list_documents") return docs.map((d) => ({ ...d }));
      if (command === "list_document_types") return types;
      throw new Error(`unexpected command ${command}`);
    });
    const user = userEvent.setup({ applyAccept: false });
    render(
      <ToastProvider>
        <DocumentList owner={{ kind: "firearm", id: 1 }} />
      </ToastProvider>,
    );
    await screen.findByRole("button", { name: "Purchase receipt.pdf" });

    await user.upload(screen.getByLabelText("Attach documents"), [
      new File(["x"], "data.bin", { type: "" }),
      new File(["%PDF"], "Second.pdf", { type: "application/pdf" }),
    ]);

    expect(await toastWith("data.bin's content isn't a PDF document.")).toHaveClass(
      "hd-toast--error",
    );
    expect(added).toBe(1);
    expect(await toastWith("Attached 1 document.")).toBeInTheDocument();
  });
});

describe("DocumentList: drops (contract §6)", () => {
  it("attaches a dropped document of an allowed type through add_document_from_path", async () => {
    await renderListWithTypes();

    act(() => backend.drop(["/home/sam/Receipts/Bill of sale.DOCX"]));

    await waitFor(() => expect(calls("add_document_from_path")).toHaveLength(1));
    expect(calls("add_document_from_path")).toEqual([
      { owner: { kind: "firearm", id: 1 }, path: "/home/sam/Receipts/Bill of sale.DOCX" },
    ]);
  });

  it("refuses a dropped .exe before any command, with the toast of §6", async () => {
    await renderListWithTypes();
    const before = backend.invoke.mock.calls.length;

    act(() => backend.drop(["/home/sam/Downloads/setup.exe"]));

    const toast = await toastWith(`setup.exe ${REFUSED}`);
    expect(toast).toHaveClass("hd-toast--error");
    await act(async () => {});
    const commandsSince = backend.invoke.mock.calls.slice(before).map(([name]) => name);
    expect(commandsSince.filter((name) => name.startsWith("add_"))).toEqual([]);
  });

  it("names a Windows path's file in the refusal", async () => {
    await renderListWithTypes();

    act(() => backend.drop(["C:\\Users\\Sam\\Downloads\\setup.exe"]));

    expect(await toastWith(`setup.exe ${REFUSED}`)).toBeInTheDocument();
  });

  it("leaves a dropped JPEG to Photos: no command from here, and no refusal", async () => {
    await renderListWithTypes();
    const before = backend.invoke.mock.calls.length;

    act(() => backend.drop(["/home/sam/Pictures/Range day.jpg", "/home/sam/Pictures/Two.PNG"]));
    await act(async () => {});

    expect(backend.invoke.mock.calls.slice(before)).toEqual([]);
    expect(noToasts()).toBeNull();
  });

  it("attaches the documents of a mixed drop, refuses the other, and leaves the photo alone", async () => {
    await renderListWithTypes();

    act(() => backend.drop(["/a/Receipt.pdf", "/a/Range day.jpg", "/a/virus.exe", "/a/Scan.tiff"]));

    await waitFor(() => expect(calls("add_document_from_path")).toHaveLength(2));
    expect(calls("add_document_from_path").map((args) => args.path)).toEqual([
      "/a/Receipt.pdf",
      "/a/Scan.tiff",
    ]);
    expect(await toastWith(`virus.exe ${REFUSED}`)).toBeInTheDocument();
    expect(screen.queryByText(/Range day\.jpg/)).not.toBeInTheDocument();
  });
});

describe("DocumentList: the empty state (contract §6)", () => {
  it("says what can be dropped, and offers choose files", async () => {
    docs = [];
    render(
      <ToastProvider>
        <DocumentList owner={{ kind: "firearm", id: 1 }} />
      </ToastProvider>,
    );

    const zone = await screen.findByRole("button", { name: /^Drop receipts/ });

    expect(zone).toHaveTextContent(
      "Drop receipts, bills of sale, registration forms or service records here (PDF, TIFF, text, Word or spreadsheet), or choose files",
    );
    expect(within(zone).getByText("choose files")).toHaveClass("hd-link");
  });
});

describe("DocumentList: Open in another app… (contract §1)", () => {
  const OPEN = "Open in another app…";
  const openButton = (name: string) => within(row(name)).getByRole("button", { name: OPEN });

  it("is a ghost button on every row, replacing Open", async () => {
    await renderList();

    for (const name of ["Purchase receipt.pdf", "Appraisal scan.tif", "Bill of sale.docx"]) {
      const button = openButton(name);
      expect(button).toBeEnabled();
      expect(button).toHaveClass("hd-button--ghost");
    }
    expect(screen.queryByRole("button", { name: "Open" })).not.toBeInTheDocument();
  });

  it("calls open_document with just the id, and shows pending while the dialog is up", async () => {
    const answer = deferred<{ opened: boolean }>();
    openAnswer = () => answer.promise;
    const user = await renderList();

    await user.click(openButton("Bill of sale.docx"));

    expect(calls("open_document")).toEqual([{ id: 4 }]);
    expect(openButton("Bill of sale.docx")).toBeDisabled();
    expect(openButton("Bill of sale.docx")).toHaveAttribute("aria-busy", "true");
    // Another row's button is not held up by it.
    expect(openButton("Purchase receipt.pdf")).toBeEnabled();

    await act(async () => answer.resolve({ opened: true }));
    await waitFor(() => expect(openButton("Bill of sale.docx")).toBeEnabled());
    expect(openButton("Bill of sale.docx")).not.toHaveAttribute("aria-busy");
  });

  it("toasts 'Opened {name} in another app.' on success", async () => {
    const user = await renderList();

    await user.click(openButton("Bill of sale.docx"));

    const toast = await toastWith("Opened Bill of sale.docx in another app.");
    expect(toast).not.toHaveClass("hd-toast--error");
  });

  it("says nothing when the user cancelled the native dialog ({ opened: false })", async () => {
    openAnswer = () => ({ opened: false });
    const user = await renderList();

    await user.click(openButton("Bill of sale.docx"));
    await waitFor(() => expect(openButton("Bill of sale.docx")).toBeEnabled());
    await act(async () => {});

    expect(calls("open_document")).toEqual([{ id: 4 }]);
    expect(noToasts()).toBeNull();
  });

  it("toasts the NO_APP_FOR_DOCUMENT message as an error", async () => {
    const message =
      "This computer has no app that opens Word documents. HoploDex deleted the copy it made.";
    openAnswer = () => {
      throw new CommandFailure({ code: "NO_APP_FOR_DOCUMENT", message });
    };
    const user = await renderList();

    await user.click(openButton("Bill of sale.docx"));

    expect(await toastWith(message)).toHaveClass("hd-toast--error");
    await waitFor(() => expect(openButton("Bill of sale.docx")).toBeEnabled());
  });

  it.each(["DOCUMENT_TYPE_NOT_ALLOWED", "DOCUMENT_CONTENT_MISMATCH"])(
    "toasts the backend's message for %s as an error",
    async (code) => {
      const message = `Bill of sale.docx can't be opened (${code}).`;
      openAnswer = () => {
        throw new CommandFailure({ code, message });
      };
      const user = await renderList();

      await user.click(openButton("Bill of sale.docx"));

      expect(await toastWith(message)).toHaveClass("hd-toast--error");
    },
  );

  it("is disabled, with its reason as the title, for a row that isn't a document type", async () => {
    const user = await renderList();

    const button = openButton("Old scan.jpg");
    expect(button).toBeDisabled();
    expect(button.getAttribute("title")).toMatch(/not a document type/i);

    await user.click(button);
    expect(calls("open_document")).toEqual([]);
  });
});

// --- User Story 3 (T097) ------------------------------------------------------

describe("DocumentList: the name follows the setting (contract §1, US3-5, US3-6)", () => {
  /** Renders the list with this computer's setting loaded as `value`. */
  async function renderListWith(value: "preview" | "external") {
    opening = value;
    await loadDocumentOpening();
    return renderList();
  }

  it("with 'external', the name calls open_document, titled 'Open {name} in another app'", async () => {
    const user = await renderListWith("external");
    const name = screen.getByRole("button", { name: "Bill of sale.docx" });
    expect(name).toHaveAttribute("title", "Open Bill of sale.docx in another app");

    await user.click(name);

    expect(calls("open_document")).toEqual([{ id: 4 }]);
    expect(calls("open_preview")).toEqual([]);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(await toastWith("Opened Bill of sale.docx in another app.")).toBeInTheDocument();
  });

  it("with 'external', every row's name is titled for opening, a previewable one too", async () => {
    await renderListWith("external");

    for (const name of ["Purchase receipt.pdf", "Appraisal scan.tif", "Notes.txt"]) {
      expect(screen.getByRole("button", { name })).toHaveAttribute(
        "title",
        `Open ${name} in another app`,
      );
    }
  });

  it("with 'external', the name of a previewable document opens it in another app, and 'Preview' still previews it (US3-5)", async () => {
    const user = await renderListWith("external");

    await user.click(screen.getByRole("button", { name: "Purchase receipt.pdf" }));
    await waitFor(() => expect(calls("open_document")).toEqual([{ id: 1 }]));
    expect(calls("open_preview")).toEqual([]);

    await user.click(within(row("Purchase receipt.pdf")).getByRole("button", { name: "Preview" }));

    const dialog = await screen.findByRole("dialog", { name: "Purchase receipt.pdf" });
    expect(dialog).toHaveAccessibleDescription(/Document 1 of 5/);
    expect(calls("open_preview")).toEqual([{ documentId: 1 }]);
    expect(calls("open_document")).toEqual([{ id: 1 }]);
  });

  it("with 'external', keeps both buttons on a previewable row and only 'Open in another app…' on the rest", async () => {
    await renderListWith("external");

    for (const name of ["Purchase receipt.pdf", "Appraisal scan.tif", "Notes.txt"]) {
      const buttons = within(row(name))
        .getAllByRole("button")
        .map((b) => b.textContent?.trim());
      expect(buttons).toContain("Preview");
      expect(buttons).toContain("Open in another app…");
    }
    const word = within(row("Bill of sale.docx"));
    expect(word.queryByRole("button", { name: "Preview" })).toBeNull();
    expect(word.getByRole("button", { name: "Open in another app…" })).toBeEnabled();
  });

  it("changes back to 'preview': the name previews again, with its title (US3-6)", async () => {
    const user = await renderListWith("external");
    expect(screen.getByRole("button", { name: "Notes.txt" })).toHaveAttribute(
      "title",
      "Open Notes.txt in another app",
    );

    let changed: boolean | undefined;
    await act(async () => {
      changed = await chooseDocumentOpening("preview");
    });

    expect(changed).toBe(true);
    expect(calls("set_document_opening")).toEqual([{ value: "preview" }]);
    expect(screen.getByRole("button", { name: "Notes.txt" })).toHaveAttribute(
      "title",
      "Preview Notes.txt",
    );
    await user.click(screen.getByRole("button", { name: "Notes.txt" }));
    expect(await screen.findByRole("dialog", { name: "Notes.txt" })).toBeInTheDocument();
    expect(calls("open_preview")).toEqual([{ documentId: 3 }]);
    expect(calls("open_document")).toEqual([]);
  });

  it("changes to 'external' while the list is shown: the name opens in another app from then on", async () => {
    const user = await renderList();
    expect(screen.getByRole("button", { name: "Notes.txt" })).toHaveAttribute(
      "title",
      "Preview Notes.txt",
    );

    await act(async () => {
      await chooseDocumentOpening("external");
    });

    expect(calls("set_document_opening")).toEqual([{ value: "external" }]);
    expect(screen.getByRole("button", { name: "Notes.txt" })).toHaveAttribute(
      "title",
      "Open Notes.txt in another app",
    );
    await user.click(screen.getByRole("button", { name: "Notes.txt" }));
    await waitFor(() => expect(calls("open_document")).toEqual([{ id: 3 }]));
    expect(calls("open_preview")).toEqual([]);
  });

  it("keeps 'preview' when the setting's confirmation is cancelled ({ changed: false })", async () => {
    settingChanges = false;
    await renderList();

    let changed: boolean | undefined;
    await act(async () => {
      changed = await chooseDocumentOpening("external");
    });

    expect(changed).toBe(false);
    expect(calls("set_document_opening")).toEqual([{ value: "external" }]);
    expect(screen.getByRole("button", { name: "Notes.txt" })).toHaveAttribute(
      "title",
      "Preview Notes.txt",
    );
  });

  it("reads the setting with get_document_opening, which takes no arguments", async () => {
    opening = "external";
    backend.invoke.mockClear();

    await loadDocumentOpening();

    expect(calls("get_document_opening")).toHaveLength(1);
    const [, args] = backend.invoke.mock.calls.find(([name]) => name === "get_document_opening")!;
    expect(args === undefined || Object.keys(args as object).length === 0).toBe(true);
  });

  it("useDocumentOpening is the loaded value, and follows a change", async () => {
    function Probe() {
      return <output aria-label="opening">{useDocumentOpening()}</output>;
    }
    opening = "external";
    await loadDocumentOpening();
    render(<Probe />);
    expect(screen.getByLabelText("opening")).toHaveTextContent("external");

    await act(async () => {
      await chooseDocumentOpening("preview");
    });
    expect(screen.getByLabelText("opening")).toHaveTextContent("preview");
  });
});
