import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import type { FileDropEvent } from "../../services/tauriClient";
import { SessionScopeContext, createSessionScope } from "../session/sessionScope";
import type { SessionScope } from "../session/sessionScope";
import { PhotoGallery } from "./PhotoGallery";
import type { PhotoSummary } from "./types";

// 008 tasks.md T029 (008 US1-2, US1-5), 008 research.md §5: a
// batch checks `scope.ended` before each file and stops, and a list load that
// resolves after the scope ended never reaches the list. The backend is
// pinned at the IPC boundary (`invoke`, `listenForFileDrops`); the scope is
// real, so every request must go through it to be refused.

const backend = vi.hoisted(() => {
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
    reset() {
      drops.clear();
    },
  };
});
vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  invoke: backend.invoke,
  listenForFileDrops: backend.listenForFileDrops,
}));

const owner = { kind: "firearm", id: 1 } as const;

function photo(id: number, originalFilename: string): PhotoSummary {
  return {
    id,
    owner,
    originalFilename,
    mimeType: "image/jpeg",
    thumbnailBytes: [1, 2, 3],
    sortOrder: id,
    createdAt: "2026-03-14 09:30:00",
  };
}

function calls(command: string) {
  return backend.invoke.mock.calls.filter(([name]) => name === command);
}

function renderGallery(scope: SessionScope) {
  return render(
    <ToastProvider>
      <SessionScopeContext.Provider value={scope}>
        <PhotoGallery owner={owner} thumbnailPhotoId={null} onChanged={vi.fn()} />
      </SessionScopeContext.Provider>
    </ToastProvider>,
  );
}

const settle = () => act(async () => void (await new Promise((r) => setTimeout(r, 30))));

beforeEach(() => {
  backend.reset();
  backend.invoke.mockReset();
});

describe("PhotoGallery: a batch stops at the scope's end (008 US1-5)", () => {
  it("sends exactly one add of three dropped photos when the scope ends after the first", async () => {
    const scope = createSessionScope(1);
    backend.invoke.mockImplementation(async (command: string, args: { path?: string }) => {
      if (command === "list_photos") return [];
      if (command === "add_photo_from_path") {
        // The session ends as the first add's answer arrives.
        scope.end();
        return photo(5, args.path!.split("/").pop()!);
      }
      throw new Error(`unexpected command ${command}`);
    });
    renderGallery(scope);
    await waitFor(() => expect(calls("list_photos")).toHaveLength(1));

    act(() => backend.drop(["/p/one.jpg", "/p/two.jpg", "/p/three.jpg"]));
    await waitFor(() => expect(calls("add_photo_from_path")).toHaveLength(1));
    await settle();

    expect(calls("add_photo_from_path")).toHaveLength(1);
    expect(calls("add_photo_from_path")[0][1]).toMatchObject({ owner, path: "/p/one.jpg" });
  });

  it("sends every add with the scope's session", async () => {
    const scope = createSessionScope(41);
    backend.invoke.mockImplementation(async (command: string) => {
      if (command === "list_photos") return [];
      return photo(5, "one.jpg");
    });
    renderGallery(scope);
    await waitFor(() => expect(calls("list_photos")).toHaveLength(1));

    act(() => backend.drop(["/p/one.jpg"]));
    await waitFor(() => expect(calls("add_photo_from_path")).toHaveLength(1));

    expect(calls("add_photo_from_path")[0][2]).toEqual({ session: 41 });
  });

  it("adds a whole batch while the scope is alive", async () => {
    const scope = createSessionScope(1);
    backend.invoke.mockImplementation(async (command: string) => {
      if (command === "list_photos") return [];
      return photo(5, "one.jpg");
    });
    renderGallery(scope);
    await waitFor(() => expect(calls("list_photos")).toHaveLength(1));

    act(() => backend.drop(["/p/one.jpg", "/p/two.jpg", "/p/three.jpg"]));

    await waitFor(() => expect(calls("add_photo_from_path")).toHaveLength(3));
  });
});

describe("PhotoGallery: a late load (008 US1-2)", () => {
  it("drops a list that resolves after the scope ended", async () => {
    const scope = createSessionScope(1);
    let finish!: (photos: PhotoSummary[]) => void;
    backend.invoke.mockReturnValue(new Promise((resolve) => (finish = resolve)));
    renderGallery(scope);
    await waitFor(() => expect(calls("list_photos")).toHaveLength(1));

    scope.end();
    await act(async () => finish([photo(7, "Left behind.jpg")]));
    await settle();

    expect(screen.queryByRole("button", { name: "View Left behind.jpg" })).not.toBeInTheDocument();
    expect(screen.queryByText("Left behind.jpg")).not.toBeInTheDocument();
  });
});

describe("PhotoGallery: at most 50 photos (001 FR-007, amended 2026-10-11)", () => {
  const FULL_NOTE = "This firearm has 50 photos, the most it can have. Delete one to add another.";
  const many = (n: number) => Array.from({ length: n }, (_, i) => photo(i + 1, `${i + 1}.jpg`));

  it("disables Add photos and shows the note in place of the drop strip once the firearm has 50", async () => {
    backend.invoke.mockImplementation(async (command: string) => {
      if (command === "list_photos") return many(50);
      throw new Error(`unexpected command ${command}`);
    });
    renderGallery(createSessionScope(1));

    expect(await screen.findByRole("note")).toHaveTextContent(FULL_NOTE);
    const add = screen.getByRole("button", { name: "Add photos" });
    expect(add).toBeDisabled();
    expect(add).toHaveAttribute("title", FULL_NOTE);
    expect(screen.queryByText(/Drop more photos here/)).not.toBeInTheDocument();
  });

  it("keeps Add photos and the drop strip below 50", async () => {
    backend.invoke.mockImplementation(async (command: string) => {
      if (command === "list_photos") return many(49);
      throw new Error(`unexpected command ${command}`);
    });
    renderGallery(createSessionScope(1));

    expect(await screen.findByText(/Drop more photos here/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add photos" })).toBeEnabled();
    expect(screen.queryByRole("note")).not.toBeInTheDocument();
  });

  it("adds up to the limit, then sends no more and counts the rest as not added", async () => {
    let photos = many(48);
    backend.invoke.mockImplementation(async (command: string) => {
      if (command === "list_photos") return photos;
      if (command === "add_photo_from_path") {
        if (photos.length >= 50) {
          throw new CommandFailure({
            code: "ATTACHMENT_LIMIT_REACHED",
            message: "This firearm has 50 photos, the most it can have.",
          });
        }
        const added = photo(photos.length + 1, "new.jpg");
        photos = [...photos, added];
        return added;
      }
      throw new Error(`unexpected command ${command}`);
    });
    renderGallery(createSessionScope(1));
    await waitFor(() => expect(calls("list_photos")).toHaveLength(1));

    act(() => backend.drop(["/p/a.jpg", "/p/b.jpg", "/p/c.jpg", "/p/d.jpg"]));

    expect(await screen.findByText("Added 2 photos.")).toBeInTheDocument();
    expect(
      await screen.findByText(
        "2 photos weren't added. This firearm has 50 photos, the most it can have.",
      ),
    ).toBeInTheDocument();
    expect(calls("add_photo_from_path")).toHaveLength(3);
    expect(screen.getByRole("button", { name: "Add photos" })).toBeDisabled();
  });

  it("names a single photo dropped on a full accessory", async () => {
    backend.invoke.mockImplementation(async (command: string) => {
      if (command === "list_photos") return many(50);
      if (command === "add_photo_from_path") {
        throw new CommandFailure({
          code: "ATTACHMENT_LIMIT_REACHED",
          message: "This accessory has 50 photos, the most it can have.",
        });
      }
      throw new Error(`unexpected command ${command}`);
    });
    render(
      <ToastProvider>
        <SessionScopeContext.Provider value={createSessionScope(1)}>
          <PhotoGallery
            owner={{ kind: "accessory", id: 3 }}
            thumbnailPhotoId={null}
            onChanged={vi.fn()}
          />
        </SessionScopeContext.Provider>
      </ToastProvider>,
    );
    expect(await screen.findByRole("note")).toHaveTextContent(
      "This accessory has 50 photos, the most it can have. Delete one to add another.",
    );

    act(() => backend.drop(["/p/Scope.jpg"]));

    expect(
      await screen.findByText(
        "Scope.jpg wasn't added. This accessory has 50 photos, the most it can have.",
      ),
    ).toBeInTheDocument();
  });
});

describe("PhotoGallery: choosing the thumbnail (001 US4-2)", () => {
  // A parent that holds the thumbnail id the way a record page does: the gallery
  // calls `onChanged` and the page reloads the record with its new thumbnail.
  function Page({ scope }: { scope: SessionScope }) {
    const [thumbnailPhotoId, setThumbnailPhotoId] = useState<number | null>(1);
    return (
      <ToastProvider>
        <SessionScopeContext.Provider value={scope}>
          <PhotoGallery
            owner={owner}
            thumbnailPhotoId={thumbnailPhotoId}
            onChanged={async () => setThumbnailPhotoId(2)}
          />
        </SessionScopeContext.Provider>
      </ToastProvider>
    );
  }

  it("Use as thumbnail in the viewer sets it, then shows Current thumbnail and moves the tag", async () => {
    const user = userEvent.setup();
    backend.invoke.mockImplementation(async (command: string) => {
      if (command === "list_photos") return [photo(1, "First.jpg"), photo(2, "Second.jpg")];
      if (command === "set_thumbnail_photo") return { thumbnailPhotoId: 2 };
      if (command === "get_photo_original") return new ArrayBuffer(0);
      throw new Error(`unexpected command ${command}`);
    });
    render(<Page scope={createSessionScope(1)} />);
    const first = await screen.findByRole("button", { name: "View First.jpg" });
    const second = screen.getByRole("button", { name: "View Second.jpg" });
    expect(within(first).getByText("Thumbnail")).toBeInTheDocument();
    expect(within(second).queryByText("Thumbnail")).not.toBeInTheDocument();

    await user.click(second);
    await user.click(await screen.findByRole("button", { name: "Use as thumbnail" }));

    expect(calls("set_thumbnail_photo")).toHaveLength(1);
    expect(calls("set_thumbnail_photo")[0][1]).toEqual({ owner, photoId: 2 });
    const current = await screen.findByRole("button", { name: "Current thumbnail" });
    expect(current).toBeDisabled();
    expect(within(second).getByText("Thumbnail")).toBeInTheDocument();
    expect(within(first).queryByText("Thumbnail")).not.toBeInTheDocument();
  });
});
