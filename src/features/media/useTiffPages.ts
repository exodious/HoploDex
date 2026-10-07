// Page layout, zoom, current page and the render-ahead window for a TIFF
// shown from the render helper's page sizes. Design: specs/007-document-preview/
// research.md §14 (TIFF pages, zoom and memory). The hook is given the scroll
// area's measurements and never touches layout itself.
import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import { CommandFailure } from "../../services/tauriClient";
import { renderPreviewPage } from "./mediaService";
import type { PageSize } from "./types";

/** "fitWidth", "fitPage", or a percent (50–400). */
export type TiffZoom = "fitWidth" | "fitPage" | number;

/** The gap between two pages, in CSS px. */
export const PAGE_GAP = 16;
/** Room kept clear around a page when it is fitted, in CSS px. */
const FIT_MARGIN = 16;
/** Bitmaps kept in the main web view (research.md §14). */
const MAX_BITMAPS = 8;
/** Widest bitmap asked for; beyond it the page is scaled up in CSS. */
const MAX_WIDTH_PX = 4096;
const ZOOM_STEP = 25;
const ZOOM_MIN = 50;
const ZOOM_MAX = 400;
/** A page is `points * 96 / 72` CSS px at 100%. */
const CSS_PX_PER_POINT = 96 / 72;
/** Zoom input this long ago or older is settled: pages are re-rendered. */
const ZOOM_SETTLE_MS = 150;

export interface PageBox {
  top: number;
  width: number;
  height: number;
}

export interface UseTiffPagesOptions {
  previewId: number;
  pages: PageSize[];
  viewportWidth: number;
  viewportHeight: number;
  scrollTop: number;
  initialZoom?: TiffZoom;
  /** A render that failed for a reason beyond its own page (`PREVIEW_FAILED`:
   * the preview is closed, `PREVIEW_CLOSED`, …). A page's own
   * `PREVIEW_PAGE_FAILED` is `failed`, not reported here. */
  onFailure?: (failure: CommandFailure) => void;
}

export interface TiffPages {
  zoom: TiffZoom;
  setZoom: (zoom: TiffZoom) => void;
  zoomIn: () => void;
  zoomOut: () => void;
  /** The zoom as a percent; for a fit, the percent it works out to. */
  percent: number;
  layout: PageBox[];
  contentHeight: number;
  /** Page index → `blob:` URL of its PNG. Revoked when the page leaves. */
  bitmaps: ReadonlyMap<number, string>;
  failed: ReadonlySet<number>;
  /** 0-based: the page with the most visible area. */
  currentPage: number;
}

interface Held {
  url: string;
  widthPx: number;
}

const clampPercent = (percent: number) => Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, percent));

/** CSS px per point for a zoom, in the scroll area given. */
function scaleFor(
  zoom: TiffZoom,
  pages: PageSize[],
  viewportWidth: number,
  viewportHeight: number,
): number {
  if (typeof zoom === "number") return (clampPercent(zoom) / 100) * CSS_PX_PER_POINT;
  const widest = Math.max(1, ...pages.map((page) => page.width));
  const tallest = Math.max(1, ...pages.map((page) => page.height));
  const availableWidth = viewportWidth - 2 * FIT_MARGIN;
  const availableHeight = viewportHeight - 2 * FIT_MARGIN;
  // Before the area is measured, show pages at 100%.
  if (availableWidth <= 0 || availableHeight <= 0) return CSS_PX_PER_POINT;
  return zoom === "fitWidth"
    ? availableWidth / widest
    : Math.min(availableWidth / widest, availableHeight / tallest);
}

export function useTiffPages({
  previewId,
  pages,
  viewportWidth,
  viewportHeight,
  scrollTop,
  initialZoom,
  onFailure,
}: UseTiffPagesOptions): TiffPages {
  const [zoom, setZoomState] = useState<TiffZoom>(
    () => initialZoom ?? (pages.length > 1 ? "fitWidth" : "fitPage"),
  );
  const scale = scaleFor(zoom, pages, viewportWidth, viewportHeight);
  const percent =
    typeof zoom === "number" ? clampPercent(zoom) : Math.round((scale / CSS_PX_PER_POINT) * 100);

  const layout = useMemo(() => {
    let top = 0;
    return pages.map((page): PageBox => {
      const box = {
        top,
        width: Math.round(page.width * scale),
        height: Math.round(page.height * scale),
      };
      top += box.height + PAGE_GAP;
      return box;
    });
  }, [pages, scale]);
  const contentHeight = layout.length
    ? layout[layout.length - 1].top + layout[layout.length - 1].height
    : 0;

  // Pages within one screen of the view, and the page with the most of it.
  const viewTop = scrollTop;
  const viewBottom = scrollTop + viewportHeight;
  const wanted = useMemo(() => {
    const from = viewTop - viewportHeight;
    const to = viewBottom + viewportHeight;
    const indexes: number[] = [];
    layout.forEach((box, index) => {
      if (box.top + box.height > from && box.top < to) indexes.push(index);
    });
    return indexes;
  }, [layout, viewTop, viewBottom, viewportHeight]);
  const wantedKey = wanted.join(",");

  const currentPage = useMemo(() => {
    let best = 0;
    let bestArea = -1;
    layout.forEach((box, index) => {
      const visible = Math.min(box.top + box.height, viewBottom) - Math.max(box.top, viewTop);
      if (visible > bestArea) {
        best = index;
        bestArea = visible;
      }
    });
    return best;
  }, [layout, viewTop, viewBottom]);

  // Zoom is applied to the layout at once; bitmaps are asked for again at the
  // new size once the zoom has been still for 150 ms (`settledScale`).
  const [settledScale, setSettledScale] = useState(scale);

  const [bitmaps, setBitmaps] = useState<ReadonlyMap<number, string>>(() => new Map());
  const [failed, setFailed] = useState<ReadonlySet<number>>(() => new Set());
  const [version, bump] = useReducer((n: number) => n + 1, 0);

  const held = useRef(new Map<number, Held>());
  const inflight = useRef(new Map<number, number>()); // page → widthPx asked for
  const failedPages = useRef(new Set<number>());
  const lastShown = useRef(new Map<number, number>());
  const tick = useRef(0);
  const generation = useRef(0);
  const latest = useRef({ pages, settledScale, wanted, onFailure });
  useEffect(() => {
    latest.current = { pages, settledScale, wanted, onFailure };
  });

  const publish = useCallback(() => {
    setBitmaps(new Map([...held.current].map(([page, bitmap]) => [page, bitmap.url])));
  }, []);

  // A new preview (or a new page list) starts empty, and a closing one frees
  // every bitmap it made; a render still in flight then finds its generation
  // gone and makes none (research.md §20).
  useEffect(() => {
    const mine = held.current;
    const pending = inflight.current;
    const shown = lastShown.current;
    const broken = failedPages.current;
    generation.current += 1;
    return () => {
      generation.current += 1;
      mine.forEach((bitmap) => URL.revokeObjectURL(bitmap.url));
      mine.clear();
      pending.clear();
      shown.clear();
      broken.clear();
      setBitmaps(new Map());
      setFailed(new Set());
    };
  }, [previewId, pages]);

  useEffect(() => {
    if (settledScale === scale) return;
    const settledNow = held.current.size === 0 && inflight.current.size === 0;
    const timer = window.setTimeout(() => setSettledScale(scale), settledNow ? 0 : ZOOM_SETTLE_MS);
    return () => window.clearTimeout(timer);
  }, [scale, settledScale]);

  const dpr = typeof window === "undefined" ? 1 : window.devicePixelRatio || 1;

  useEffect(() => {
    const widthFor = (page: number, at: number) =>
      Math.min(MAX_WIDTH_PX, Math.max(1, Math.round(latest.current.pages[page].width * at * dpr)));

    for (const page of wanted) lastShown.current.set(page, ++tick.current);
    // While a zoom is still moving, nothing is asked for.
    if (settledScale !== scale) return;

    const mine = generation.current;
    for (const page of wanted) {
      if (failedPages.current.has(page)) continue;
      const widthPx = widthFor(page, settledScale);
      if (held.current.get(page)?.widthPx === widthPx) continue;
      if (inflight.current.get(page) === widthPx) continue;
      inflight.current.set(page, widthPx);

      renderPreviewPage(previewId, page, widthPx).then(
        (bytes) => {
          if (generation.current !== mine) return;
          if (inflight.current.get(page) === widthPx) inflight.current.delete(page);
          const now = latest.current;
          // The view moved on, or the zoom did, while it rendered.
          if (!now.wanted.includes(page) || widthFor(page, now.settledScale) !== widthPx) {
            bump();
            return;
          }
          const url = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
          const replaced = held.current.get(page);
          held.current.set(page, { url, widthPx });
          if (replaced) URL.revokeObjectURL(replaced.url);
          // Least recently shown first out; among pages shown at the same
          // time, the one furthest from this page.
          while (held.current.size > MAX_BITMAPS) {
            let victim = -1;
            for (const candidate of held.current.keys()) {
              if (candidate === page) continue;
              if (victim === -1) {
                victim = candidate;
                continue;
              }
              const older =
                (lastShown.current.get(candidate) ?? 0) - (lastShown.current.get(victim) ?? 0);
              if (
                older < 0 ||
                (older === 0 && Math.abs(candidate - page) > Math.abs(victim - page))
              ) {
                victim = candidate;
              }
            }
            if (victim === -1) break;
            URL.revokeObjectURL(held.current.get(victim)!.url);
            held.current.delete(victim);
          }
          publish();
        },
        (error: unknown) => {
          if (generation.current !== mine) return;
          if (inflight.current.get(page) === widthPx) inflight.current.delete(page);
          failedPages.current.add(page);
          setFailed(new Set(failedPages.current));
          if (error instanceof CommandFailure && error.code !== "PREVIEW_PAGE_FAILED") {
            latest.current.onFailure?.(error);
          }
        },
      );
    }
    // `wanted` is keyed by its content: a new array with the same pages is not a change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [previewId, wantedKey, scale, settledScale, dpr, version, pages, publish]);

  const setZoom = useCallback((next: TiffZoom) => {
    setZoomState(typeof next === "number" ? clampPercent(next) : next);
  }, []);
  const zoomIn = useCallback(() => {
    setZoomState(Math.min(ZOOM_MAX, (Math.floor(percent / ZOOM_STEP) + 1) * ZOOM_STEP));
  }, [percent]);
  const zoomOut = useCallback(() => {
    setZoomState(Math.max(ZOOM_MIN, (Math.ceil(percent / ZOOM_STEP) - 1) * ZOOM_STEP));
  }, [percent]);

  return {
    zoom,
    setZoom,
    zoomIn,
    zoomOut,
    percent,
    layout,
    contentHeight,
    bitmaps,
    failed,
    currentPage,
  };
}
