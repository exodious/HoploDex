import { beforeEach, describe, expect, it, vi } from "vitest";
import { createSessionScope } from "../session/sessionScope";
import * as mediaService from "./mediaService";

// 008 research.md §3, 008 contracts/tauri-commands.md "Every
// request names its session": every photo, document and preview command goes
// through the scope (`{ session: id }`); the per-computer document-opening
// setting does not.

const backend = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  invoke: backend.invoke,
}));

const OWNER = { kind: "firearm", id: 7 } as const;
const bounds = { x: 1, y: 2, width: 3, height: 4 };

describe("mediaService", () => {
  const scope = createSessionScope(41);

  beforeEach(() => {
    backend.invoke.mockReset().mockResolvedValue(undefined);
  });

  it.each([
    ["list_photos", { owner: OWNER }, () => mediaService.listPhotos(scope, OWNER)],
    [
      "add_photo",
      { owner: OWNER, fileBytes: [1], originalFilename: "a.jpg", mimeType: "image/jpeg" },
      () => mediaService.addPhoto(scope, OWNER, [1], "a.jpg", "image/jpeg"),
    ],
    [
      "add_photo_from_path",
      { owner: OWNER, path: "/a.jpg" },
      () => mediaService.addPhotoFromPath(scope, OWNER, "/a.jpg"),
    ],
    [
      "set_thumbnail_photo",
      { owner: OWNER, photoId: 3 },
      () => mediaService.setThumbnailPhoto(scope, OWNER, 3),
    ],
    [
      "delete_photo",
      { photoId: 3, confirmed: true },
      () => mediaService.deletePhoto(scope, 3, true),
    ],
    ["get_photo_thumbnail", { photoId: 3 }, () => mediaService.getPhotoThumbnail(scope, 3)],
    ["get_photo_original", { photoId: 3 }, () => mediaService.getPhotoOriginal(scope, 3)],
    ["list_documents", { owner: OWNER }, () => mediaService.listDocuments(scope, OWNER)],
    ["list_document_types", undefined, () => mediaService.listDocumentTypes(scope)],
    [
      "add_document",
      { owner: OWNER, fileBytes: [1], originalFilename: "a.pdf" },
      () => mediaService.addDocument(scope, OWNER, [1], "a.pdf"),
    ],
    [
      "add_document_from_path",
      { owner: OWNER, path: "/a.pdf" },
      () => mediaService.addDocumentFromPath(scope, OWNER, "/a.pdf"),
    ],
    ["open_document", { id: 5 }, () => mediaService.openDocument(scope, 5)],
    [
      "delete_document",
      { id: 5, confirmed: true },
      () => mediaService.deleteDocument(scope, 5, true),
    ],
    ["open_preview", { documentId: 5 }, () => mediaService.openPreview(scope, 5)],
    [
      "set_preview_bounds",
      { previewId: 9, bounds, visible: true },
      () => mediaService.setPreviewBounds(scope, 9, bounds, true),
    ],
    ["focus_preview", { previewId: 9 }, () => mediaService.focusPreview(scope, 9)],
    [
      "render_preview_page",
      { previewId: 9, page: 2, widthPx: 800 },
      () => mediaService.renderPreviewPage(scope, 9, 2, 800),
    ],
    ["close_preview", { previewId: 9 }, () => mediaService.closePreview(scope, 9)],
  ])("sends %s with the session", async (command, args, call) => {
    await call();
    expect(backend.invoke).toHaveBeenCalledExactlyOnceWith(command, args, { session: 41 });
  });

  it.each([
    ["get_document_opening", undefined, () => mediaService.getDocumentOpening()],
    [
      "set_document_opening",
      { value: "external" },
      () => mediaService.setDocumentOpening("external"),
    ],
  ])("sends %s, a per-computer setting, with no session", async (command, args, call) => {
    await call();
    // No options at all: only a scope names a session.
    expect(backend.invoke.mock.calls).toEqual([args === undefined ? [command] : [command, args]]);
  });

  it("refuses a scoped command once the scope has ended, without sending it", async () => {
    const ended = createSessionScope(42);
    ended.end();
    await expect(mediaService.listPhotos(ended, OWNER)).rejects.toMatchObject({
      code: "DATABASE_CLOSED",
    });
    expect(backend.invoke).not.toHaveBeenCalled();
  });
});
