import { act, render, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { bytesToDataUrl } from "../../lib/bytes";
import * as tauriClient from "../../services/tauriClient";
import { SessionScopeContext, createSessionScope } from "../session/sessionScope";
import type { SessionScope } from "../session/sessionScope";
import { FirearmThumbnail } from "./FirearmThumbnail";

// 008 tasks.md T028 (008 US1-1, US1-2), 008 research.md §4: the
// thumbnail cache is the session scope's (`scope.thumbnails`), not the
// module's, and an image that arrives after the scope ended is neither cached
// nor shown. The backend is pinned at the IPC boundary; the scope is real.

vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  invoke: vi.fn(),
}));

const BYTES_A = [1, 2, 3];
const BYTES_B = [9, 8, 7, 6];
const urlOf = (bytes: number[]) => bytesToDataUrl(bytes, "image/jpeg");

function withScope(scope: SessionScope, children: ReactNode) {
  return <SessionScopeContext.Provider value={scope}>{children}</SessionScopeContext.Provider>;
}

function thumbnail(scope: SessionScope, photoId = 7) {
  return withScope(
    scope,
    <FirearmThumbnail thumbnailPhotoId={photoId} genericThumbnailKey="pistol" />,
  );
}

const photo = (container: HTMLElement) =>
  container.querySelector<HTMLImageElement>("img.hd-thumb__photo");

function thumbnailCalls() {
  return vi.mocked(tauriClient.invoke).mock.calls.filter(([c]) => c === "get_photo_thumbnail");
}

beforeEach(() => {
  vi.mocked(tauriClient.invoke).mockReset();
});

describe("FirearmThumbnail: the cache belongs to the session (008 US1-1)", () => {
  it("doesn't show scope A's thumbnail for photo 7 under scope B, which loads its own", async () => {
    const a = createSessionScope(1);
    const b = createSessionScope(2);
    vi.mocked(tauriClient.invoke).mockImplementation(async (_command, _args, options) =>
      options?.session === 1 ? BYTES_A : BYTES_B,
    );

    const first = render(thumbnail(a));
    await waitFor(() => expect(photo(first.container)).toHaveAttribute("src", urlOf(BYTES_A)));
    first.unmount();
    a.end();

    const second = render(thumbnail(b));
    await waitFor(() => expect(photo(second.container)).toHaveAttribute("src", urlOf(BYTES_B)));

    expect(thumbnailCalls()).toHaveLength(2);
    expect(thumbnailCalls()[1][2]).toEqual({ session: 2 });
    expect(b.thumbnails.get(7)).toBe(urlOf(BYTES_B));
  });

  it("never shows the other scope's cached image while its own loads, even if both are alive", async () => {
    const a = createSessionScope(1);
    const b = createSessionScope(2);
    vi.mocked(tauriClient.invoke).mockResolvedValue(BYTES_A);
    const first = render(thumbnail(a, 8));
    await waitFor(() => expect(photo(first.container)).not.toBeNull());
    expect(a.thumbnails.get(8)).toBe(urlOf(BYTES_A));

    let finish!: (bytes: number[]) => void;
    vi.mocked(tauriClient.invoke).mockReturnValue(new Promise((resolve) => (finish = resolve)));
    const second = render(thumbnail(b, 8));

    await waitFor(() => expect(thumbnailCalls()).toHaveLength(2));
    expect(photo(second.container)).toBeNull();
    expect(b.thumbnails.has(8)).toBe(false);
    await act(async () => finish(BYTES_B));
    await waitFor(() => expect(photo(second.container)).toHaveAttribute("src", urlOf(BYTES_B)));
  });

  it("reuses a cached thumbnail within one scope without asking again", async () => {
    const scope = createSessionScope(1);
    vi.mocked(tauriClient.invoke).mockResolvedValue(BYTES_A);
    const first = render(thumbnail(scope, 9));
    await waitFor(() => expect(photo(first.container)).not.toBeNull());
    first.unmount();

    const again = render(thumbnail(scope, 9));

    await waitFor(() => expect(photo(again.container)).toHaveAttribute("src", urlOf(BYTES_A)));
    expect(thumbnailCalls()).toHaveLength(1);
  });

  it("empties the scope's thumbnails when it ends", async () => {
    const scope = createSessionScope(1);
    vi.mocked(tauriClient.invoke).mockResolvedValue(BYTES_A);
    const { container } = render(thumbnail(scope, 10));
    await waitFor(() => expect(photo(container)).not.toBeNull());
    expect(scope.thumbnails.size).toBe(1);

    scope.end();

    expect(scope.thumbnails.size).toBe(0);
  });
});

describe("FirearmThumbnail: a late response (008 US1-2)", () => {
  it("drops a deferred response that arrives after the scope ended: not shown, not cached", async () => {
    const scope = createSessionScope(1);
    let finish!: (bytes: number[]) => void;
    vi.mocked(tauriClient.invoke).mockReturnValue(new Promise((resolve) => (finish = resolve)));
    const { container } = render(thumbnail(scope, 11));
    await waitFor(() => expect(thumbnailCalls()).toHaveLength(1));

    scope.end();
    await act(async () => finish(BYTES_A));

    expect(photo(container)).toBeNull();
    expect(scope.thumbnails.size).toBe(0);
  });
});

describe("FirearmThumbnail: no photo (001 US4-3)", () => {
  it("draws the type's generic drawing and fetches nothing when thumbnailPhotoId is null", async () => {
    const scope = createSessionScope(1);
    const { container } = render(
      withScope(scope, <FirearmThumbnail thumbnailPhotoId={null} genericThumbnailKey="rifle" />),
    );
    await act(async () => void (await new Promise((r) => setTimeout(r, 30))));

    expect(container.querySelector('svg[data-drawing="rifle"]')).not.toBeNull();
    expect(photo(container)).toBeNull();
    expect(vi.mocked(tauriClient.invoke)).not.toHaveBeenCalled();
  });
});
