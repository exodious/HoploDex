import { act, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../../components";
import type { FileDropEvent } from "../../services/tauriClient";
import { SessionScopeContext, createSessionScope } from "../session/sessionScope";
import type { SessionScope } from "../session/sessionScope";
import { PhotoGallery } from "./PhotoGallery";
import type { PhotoSummary } from "./types";

// specs/008-hardening-batch tasks.md T029 (US1-2, US1-5), research.md §5: a
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

describe("PhotoGallery: a batch stops at the scope's end (US1-5)", () => {
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

describe("PhotoGallery: a late load (US1-2)", () => {
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
