import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PageSize } from "./types";
import { useTiffPages } from "./useTiffPages";
import type { TiffZoom } from "./useTiffPages";

// specs/007-document-preview/research.md §14 (TIFF pages, zoom and memory),
// tasks.md T040. The hook is given the scroll area's measurements (it never
// touches layout itself), so everything here is plain numbers:
//
//   useTiffPages({ previewId, pages, viewportWidth, viewportHeight, scrollTop,
//                  initialZoom? }) => {
//     zoom: TiffZoom, setZoom(zoom), zoomIn(), zoomOut(), percent: number,
//     layout: { top, width, height }[]     // CSS px, scroll-area coordinates
//     contentHeight: number,
//     bitmaps: ReadonlyMap<number, string> // page index -> `blob:` URL
//     failed: ReadonlySet<number>          // pages that came back PREVIEW_PAGE_FAILED
//     currentPage: number                  // 0-based, most visible area
//   }
//   type TiffZoom = "fitWidth" | "fitPage" | number   // a number is a percent
//
// A page is `points * 96 / 72` CSS px at 100%, with a 16 px gap between pages.
// Pages come from `render_preview_page` ({ previewId, page, widthPx }).

const backend = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  invoke: backend.invoke,
}));

import { CommandFailure } from "../../services/tauriClient";

const LETTER: PageSize = { width: 612, height: 792 }; // 816 x 1056 CSS px at 100%
const cache = new Map<number, PageSize[]>();
/** The same array for the same count, as the viewer passes the one `open_preview` returned. */
const pagesOf = (n: number): PageSize[] => {
  if (!cache.has(n))
    cache.set(
      n,
      Array.from({ length: n }, () => LETTER),
    );
  return cache.get(n)!;
};

let created: string[];
let revoked: string[];
let nextBlob: number;
let failPages: Set<number>;

function renderCalls(): { previewId: number; page: number; widthPx: number }[] {
  return backend.invoke.mock.calls
    .filter(([command]) => command === "render_preview_page")
    .map(([, args]) => args);
}

function setDevicePixelRatio(value: number) {
  Object.defineProperty(window, "devicePixelRatio", { configurable: true, value });
}

/** Lets the mocked renders settle and React apply what they returned. */
async function flush() {
  await act(async () => {
    for (let i = 0; i < 5; i++) await Promise.resolve();
  });
}

interface Props {
  previewId: number;
  pages: PageSize[];
  viewportWidth: number;
  viewportHeight: number;
  scrollTop: number;
  initialZoom?: TiffZoom;
}

function mount(pages: PageSize[], overrides: Partial<Props> = {}) {
  const initial: Props = {
    previewId: 12,
    pages,
    viewportWidth: 1000,
    viewportHeight: 800,
    scrollTop: 0,
    initialZoom: 100,
    ...overrides,
  };
  const hook = renderHook((props: Props) => useTiffPages(props), { initialProps: initial });
  /** Scrolls the area, the way a scroll event would give the hook a new scrollTop. */
  const scrollTo = (scrollTop: number) => hook.rerender({ ...initial, scrollTop });
  return { ...hook, scrollTo };
}

beforeEach(() => {
  created = [];
  revoked = [];
  nextBlob = 0;
  failPages = new Set();
  setDevicePixelRatio(1);
  backend.invoke.mockReset().mockImplementation(async (command: string, args: { page: number }) => {
    if (command !== "render_preview_page") throw new Error(`unexpected ${command}`);
    if (failPages.has(args.page)) {
      throw new CommandFailure({ code: "PREVIEW_PAGE_FAILED", message: "Page failed." });
    }
    return new ArrayBuffer(8);
  });
  URL.createObjectURL = vi.fn(() => {
    const url = `blob:test/${nextBlob++}`;
    created.push(url);
    return url;
  });
  URL.revokeObjectURL = vi.fn((url: string) => {
    revoked.push(url);
  });
});

afterEach(() => {
  vi.useRealTimers();
  setDevicePixelRatio(1);
});

describe("useTiffPages: what is requested (research.md §14)", () => {
  it("starts in fit width for a multi-page TIFF and fit page for a single page", () => {
    const multi = mount(pagesOf(3), { initialZoom: undefined });
    expect(multi.result.current.zoom).toBe("fitWidth");
    const single = mount(pagesOf(1), { initialZoom: undefined });
    expect(single.result.current.zoom).toBe("fitPage");
  });

  it("lays every page out from the sizes it was given, with 16 px gaps", () => {
    const { result } = mount([LETTER, { width: 306, height: 396 }, LETTER]);

    const [first, second, third] = result.current.layout;
    expect([first.width, first.height]).toEqual([816, 1056]);
    expect([second.width, second.height]).toEqual([408, 528]);
    expect(second.top - (first.top + first.height)).toBe(16);
    expect(third.top - (second.top + second.height)).toBe(16);
    expect(result.current.contentHeight).toBeGreaterThanOrEqual(third.top + third.height);
  });

  it("fits the width and the page to the scroll area", () => {
    const fitWidth = mount(pagesOf(3), { initialZoom: "fitWidth" });
    expect(fitWidth.result.current.layout[0].width).toBeLessThanOrEqual(1000);
    expect(fitWidth.result.current.layout[0].width).toBeGreaterThanOrEqual(900);

    const fitPage = mount(pagesOf(3), { initialZoom: "fitPage" });
    expect(fitPage.result.current.layout[0].height).toBeLessThanOrEqual(800);
    expect(fitPage.result.current.layout[0].height).toBeGreaterThanOrEqual(736);
  });

  it("requests only the pages within one screen of the view", async () => {
    const { result, scrollTo } = mount(pagesOf(10));
    await flush();

    // 800 px high: the view, one screen above it and one below it reach page 1
    // (tops at 0 and 1072) and stop short of page 2 (2144).
    expect(new Set(renderCalls().map((call) => call.page))).toEqual(new Set([0, 1]));
    expect(renderCalls().every((call) => call.previewId === 12)).toBe(true);
    expect([...result.current.bitmaps.keys()].sort()).toEqual([0, 1]);

    scrollTo(5000);
    await flush();

    const requested = new Set(renderCalls().map((call) => call.page));
    expect(requested.has(4)).toBe(true);
    expect(requested.has(5)).toBe(true);
    expect(requested.has(9)).toBe(false);
  });

  it("asks for a bitmap at CSS width times devicePixelRatio", async () => {
    setDevicePixelRatio(2);
    mount(pagesOf(3));
    await flush();

    expect(renderCalls().map((call) => call.widthPx)).toEqual([1632, 1632]); // 816 x 2
  });

  it("caps the bitmap at 4096 px wide, whatever the zoom and the ratio", async () => {
    setDevicePixelRatio(2);
    mount(pagesOf(2), { initialZoom: 400 }); // 3264 CSS px x 2 = 6528
    await flush();

    expect(renderCalls().length).toBeGreaterThan(0);
    expect(renderCalls().every((call) => call.widthPx === 4096)).toBe(true);
  });

  it("asks for a page once, and keeps asking for none that came back", async () => {
    const { scrollTo } = mount(pagesOf(10));
    await flush();
    const first = renderCalls().filter((call) => call.page === 0).length;

    for (const scrollTop of [300, 0, 400, 0]) {
      scrollTo(scrollTop);
      await flush();
    }

    expect(first).toBe(1);
    expect(renderCalls().filter((call) => call.page === 0)).toHaveLength(1);
  });

  it("marks a page PREVIEW_PAGE_FAILED and leaves the others, without asking again", async () => {
    failPages.add(1);
    const { result, scrollTo } = mount(pagesOf(10));
    await flush();

    expect([...result.current.failed]).toEqual([1]);
    expect(result.current.bitmaps.has(0)).toBe(true);
    expect(result.current.bitmaps.has(1)).toBe(false);

    scrollTo(100);
    await flush();
    expect(renderCalls().filter((call) => call.page === 1)).toHaveLength(1);
  });
});

describe("useTiffPages: memory (research.md §14)", () => {
  it("keeps at most 8 bitmaps, evicting the least recently shown and revoking its blob URL", async () => {
    const { result, scrollTo } = mount(pagesOf(20));
    await flush();
    const firstShown = result.current.bitmaps.get(0);
    expect(firstShown).toBeDefined();

    let most = 0;
    for (let scrollTop = 0; scrollTop <= 20 * 1072; scrollTop += 800) {
      scrollTo(scrollTop);
      await flush();
      most = Math.max(most, result.current.bitmaps.size);
    }

    const held = [...result.current.bitmaps.values()];
    expect(created.length).toBeGreaterThan(8);
    expect(most).toBeLessThanOrEqual(8);
    // Every URL made is either still shown or revoked, and none is both.
    expect(revoked.length).toBe(created.length - held.length);
    expect(held.some((url) => revoked.includes(url))).toBe(false);
    // The first page seen is the first one out.
    expect(revoked[0]).toBe(firstShown);
  });

  it("revokes every blob URL when it unmounts, and any a late render makes", async () => {
    const { result, unmount } = mount(pagesOf(10));
    await flush();
    expect(result.current.bitmaps.size).toBeGreaterThan(0);

    unmount();
    await flush();

    expect(created.length).toBeGreaterThan(0);
    expect([...revoked].sort()).toEqual([...created].sort());
  });
});

describe("useTiffPages: zoom (research.md §14)", () => {
  it("scales the pages at once and re-renders 150 ms after the last zoom input", async () => {
    vi.useFakeTimers();
    const { result } = mount(pagesOf(3));
    await flush();
    const before = renderCalls().length;
    const shown = result.current.bitmaps.get(0);

    act(() => result.current.setZoom(200));

    expect(result.current.percent).toBe(200);
    expect(result.current.layout[0].width).toBe(1632);
    expect(result.current.bitmaps.get(0)).toBe(shown); // scaled in CSS until the new one arrives
    expect(renderCalls()).toHaveLength(before);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(149);
    });
    expect(renderCalls()).toHaveLength(before);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    await flush();
    const rendered = renderCalls().slice(before);
    expect(rendered.length).toBeGreaterThan(0);
    expect(rendered.every((call) => call.widthPx === 1632)).toBe(true);
    // The bitmap it replaced is revoked once the new one is in.
    expect(result.current.bitmaps.get(0)).not.toBe(shown);
    expect(revoked).toContain(shown);
  });

  it("waits out further zoom input before re-rendering", async () => {
    vi.useFakeTimers();
    const { result } = mount(pagesOf(3));
    await flush();
    const before = renderCalls().length;

    act(() => result.current.setZoom(200));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    act(() => result.current.setZoom(250));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(149);
    });
    expect(renderCalls()).toHaveLength(before);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    await flush();
    const rendered = renderCalls().slice(before);
    expect(rendered.length).toBeGreaterThan(0);
    expect(rendered.every((call) => call.widthPx === 2040)).toBe(true); // 816 x 2.5
  });

  it("zooms in and out in 25% steps between 50% and 400%", () => {
    const { result } = mount(pagesOf(2));

    act(() => result.current.zoomIn());
    expect(result.current.percent).toBe(125);
    act(() => result.current.zoomOut());
    act(() => result.current.zoomOut());
    expect(result.current.percent).toBe(75);

    act(() => result.current.setZoom(50));
    act(() => result.current.zoomOut());
    expect(result.current.percent).toBe(50);

    act(() => result.current.setZoom(400));
    act(() => result.current.zoomIn());
    expect(result.current.percent).toBe(400);
  });
});

describe("useTiffPages: the current page (research.md §14)", () => {
  it.each([
    [0, 0],
    [100, 0], // page 0 fills the view; page 1 starts below it
    [900, 1], // 156 px of page 0 against 628 px of page 1
    [2000, 2], // 128 px of page 1 against 656 px of page 2
  ])("at scrollTop %i the current page is %i", (scrollTop, page) => {
    const { result } = mount(pagesOf(5), { scrollTop });
    expect(result.current.currentPage).toBe(page);
  });
});
