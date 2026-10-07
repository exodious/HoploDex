// The document viewer: a dialog over the record, with its header, the page area
// for the document's kind, and a footer, laid out like PhotoViewer
// (PhotoGallery.tsx). Contract: specs/007-document-preview/contracts/
// ui-document-preview.md §2 (The viewer), §3 (States in the page area), §7
// (Keyboard) and §8 (Accessibility); research.md §12 (text) and §14 (TIFF).
//
// "Open in another app…" is not here yet: user story 2 adds it to the footer and
// to the states that offer it (tasks.md T093).
import { useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent, UIEvent } from "react";
import {
  Button,
  Dialog,
  Icon,
  Menu,
  MenuRadioGroup,
  MenuRadioItem,
  placeFocus,
} from "../../components";
import type { IconName } from "../../components";
import { formatDate } from "../../lib/dates";
import { CommandFailure } from "../../services/tauriClient";
import { documentKindLabel } from "./documentKind";
import {
  closePreview,
  focusPreview,
  onPdfEnded as listenPdfEnded,
  onPreviewEscape,
  onPreviewFocusChrome,
  openPreview,
} from "./mediaService";
import { PreviewSurface } from "./PreviewSurface";
import type { DocumentSummary, DocumentType, PageSize, PdfEndReason, PreviewInfo } from "./types";
import { PAGE_GAP, useTiffPages } from "./useTiffPages";
import type { TiffZoom } from "./useTiffPages";
import "./media.css";

export interface DocumentPreviewProps {
  /** The record's own documents, in list order: previous and next move only
   * through these (FR-007). */
  documents: DocumentSummary[];
  /** The one shown. */
  index: number;
  documentTypes: DocumentType[];
  onIndexChange: (index: number) => void;
  onClose: () => void;
  /** "Delete document": the list owns the confirmation and what follows. */
  onDelete: (document: DocumentSummary) => void;
  /** The PDF surface was closed by the backend (`preview:pdf-ended`), so the
   * list can reload for `copyCaught` and `noViewer`. */
  onPdfEnded?: (reason: PdfEndReason) => void;
}

/** What the page area shows when there is no document to show (ui contract §3). */
interface Problem {
  code: string;
  message: string;
}

type View =
  | { phase: "loading" }
  | { phase: "ready"; info: PreviewInfo }
  | { phase: "problem"; problem: Problem };

const PDF_ENDED: Record<PdfEndReason, Problem> = {
  noViewer: {
    code: "PDF_PREVIEW_UNAVAILABLE",
    message: "This computer's web view has no built-in PDF viewer.",
  },
  copyCaught: { code: "PDF_COPY_CAUGHT", message: "" },
  failed: { code: "PREVIEW_FAILED", message: "" },
};

function withArticle(kind: string): string {
  return /^(?:[AEIOU]|RTF)/.test(kind) ? `an ${kind}` : `a ${kind}`;
}

/** The sentence for a state, ui contract §3. */
function problemSentence(problem: Problem, name: string, kind: string): string {
  switch (problem.code) {
    case "PREVIEW_UNSUPPORTED":
      return `${name} can't be previewed here. ${kind} documents open in another app.`;
    case "PDF_PREVIEW_UNAVAILABLE":
      return `PDFs can't be previewed on this computer. ${problem.message}`.trim();
    case "PDF_COPY_CAUGHT":
      return `This computer's PDF viewer saved a copy of ${name} to disk. HoploDex deleted it and has turned PDF previews off on this computer until HoploDex is updated. You can still open PDFs in another app.`;
    case "DOCUMENT_CONTENT_MISMATCH":
      return `${name} can't be previewed: its content isn't ${withArticle(kind)} document.`;
    case "PREVIEW_DAMAGED":
      return `${name} can't be previewed: the document is damaged or incomplete.`;
    default:
      return `${name} couldn't be previewed.`;
  }
}

function toProblem(error: unknown): Problem {
  return error instanceof CommandFailure
    ? { code: error.code, message: error.message }
    : { code: "PREVIEW_FAILED", message: "" };
}

/** Keys that belong to a menu, a dialog over the viewer, a field, or the TIFF's
 * page area (where ← and → scroll sideways) are not the viewer's own. */
const OWN_KEYS_NOT_HERE =
  '[role="menu"], [role="alertdialog"], input, textarea, select, .hd-preview__pages';

export function DocumentPreview({
  documents,
  index,
  documentTypes,
  onIndexChange,
  onClose,
  onDelete,
  onPdfEnded,
}: DocumentPreviewProps) {
  const doc = documents[index];
  const name = doc.originalFilename;
  const kind = documentKindLabel(doc, documentTypes);
  const [view, setView] = useState<View>({ phase: "loading" });
  const root = useRef<HTMLDivElement>(null);
  const hasPrev = index > 0;
  const hasNext = index < documents.length - 1;

  // The newest values, for the listeners set up once.
  const latest = useRef({ index, hasPrev, hasNext, view, onIndexChange, onClose, onPdfEnded });
  useEffect(() => {
    latest.current = { index, hasPrev, hasNext, view, onIndexChange, onClose, onPdfEnded };
  });

  // The preview open now, to close when the viewer goes. Moving to another
  // document closes nothing: `open_preview` replaces the one before it, and a
  // PDF followed by a PDF keeps its surface (research.md §4).
  const openPreviewId = useRef<number | null>(null);
  const unmounted = useRef(false);
  useEffect(() => {
    unmounted.current = false;
    return () => {
      unmounted.current = true;
      const id = openPreviewId.current;
      openPreviewId.current = null;
      if (id !== null) closePreview(id).catch(() => {});
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    setView({ phase: "loading" });
    openPreview(doc.id).then(
      (info) => {
        // Closed while it was preparing: the answer is closed as soon as it's known.
        if (unmounted.current) {
          closePreview(info.previewId).catch(() => {});
          return;
        }
        if (cancelled) return;
        openPreviewId.current = info.previewId;
        setView({ phase: "ready", info });
      },
      (error: unknown) => {
        if (!cancelled && !unmounted.current)
          setView({ phase: "problem", problem: toProblem(error) });
      },
    );
    return () => {
      cancelled = true;
    };
  }, [doc.id]);

  const focusFirstControl = useCallback(() => {
    const dialog = root.current?.closest('[role="dialog"]');
    placeFocus(dialog?.querySelector<HTMLElement>("button:not(:disabled)"));
  }, []);

  // What the PDF surface says: Escape and F6 pressed inside it, and its end.
  const pdfId = view.phase === "ready" && view.info.kind === "pdf" ? view.info.previewId : null;
  useEffect(() => {
    if (pdfId === null) return;
    const stops = [
      onPreviewEscape(pdfId, () => latest.current.onClose()),
      onPreviewFocusChrome(pdfId, focusFirstControl),
      listenPdfEnded(pdfId, (reason) => {
        setView({ phase: "problem", problem: PDF_ENDED[reason] });
        latest.current.onPdfEnded?.(reason);
      }),
    ];
    return () => stops.forEach((stop) => stop());
  }, [pdfId, focusFirstControl]);

  // ← and → move between documents, and F6 moves into a PDF, wherever the
  // focus is in HoploDex's own controls (ui contract §7).
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey) return;
      const target = event.target instanceof Element ? event.target : null;
      const now = latest.current;
      if (event.key === "F6") {
        if (target?.closest('[role="alertdialog"]')) return;
        if (now.view.phase === "ready" && now.view.info.kind === "pdf") {
          event.preventDefault();
          focusPreview(now.view.info.previewId).catch(() => {});
        }
        return;
      }
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
      if (target?.closest(OWN_KEYS_NOT_HERE)) return;
      if (event.key === "ArrowLeft" && now.hasPrev) now.onIndexChange(now.index - 1);
      if (event.key === "ArrowRight" && now.hasNext) now.onIndexChange(now.index + 1);
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, []);

  const pageCount =
    view.phase === "ready" && view.info.kind === "tiff" ? view.info.pages.length : 0;
  const description = [
    `Document ${index + 1} of ${documents.length}`,
    kind,
    `added ${formatDate(doc.createdAt.slice(0, 10))}`,
    ...(pageCount > 1 ? [`${pageCount} pages`] : []),
  ].join(" · ");

  let content;
  if (view.phase === "loading") {
    content = (
      <div className="hd-preview__state hd-preview__state--loading" role="status">
        <span className="hd-spinner" aria-hidden />
        <span>{`Preparing ${name}…`}</span>
      </div>
    );
  } else if (view.phase === "problem") {
    const icon: IconName = view.problem.code === "PREVIEW_UNSUPPORTED" ? "file" : "alert";
    content = (
      <div className="hd-preview__state">
        <Icon name={icon} size={28} className="hd-preview__state-icon" />
        <p role="status">{problemSentence(view.problem, name, kind)}</p>
      </div>
    );
  } else if (view.info.kind === "pdf") {
    content = (
      <PreviewSurface key={view.info.previewId} previewId={view.info.previewId} name={name} />
    );
  } else if (view.info.kind === "tiff") {
    content = (
      <TiffView
        key={view.info.previewId}
        previewId={view.info.previewId}
        pages={view.info.pages}
        name={name}
        onFailure={(failure) => setView({ phase: "problem", problem: toProblem(failure) })}
      />
    );
  } else {
    content = <TextView key={view.info.previewId} text={view.info.text} name={name} />;
  }

  return (
    <Dialog
      open
      onOpenChange={(open) => !open && onClose()}
      title={name}
      description={description}
      size="xl"
      footer={
        <>
          <div className="hd-viewer__nav">
            <Button
              size="sm"
              icon="chevronLeft"
              aria-label="Previous document"
              disabled={!hasPrev}
              onClick={() => onIndexChange(index - 1)}
            />
            <Button
              size="sm"
              icon="chevronRight"
              aria-label="Next document"
              disabled={!hasNext}
              onClick={() => onIndexChange(index + 1)}
            />
            {pdfId !== null && (
              // What a toast would say over the PDF surface, which a toast can't
              // be drawn over (ui contract §2). Filled in with user story 2.
              <span className="hd-preview__footer-status" role="status" />
            )}
          </div>
          <Button variant="ghost" icon="trash" onClick={() => onDelete(doc)}>
            Delete document
          </Button>
        </>
      }
    >
      <div className="hd-preview" ref={root}>
        {content}
      </div>
    </Dialog>
  );
}

// --- Text ---------------------------------------------------------------------

/** A text document over this is shown in chunks of it, appended as the user
 * scrolls, so the first screen is within budget (research.md §12). */
const TEXT_CHUNK = 1 << 20;

function chunkText(text: string): string[] {
  const chunks: string[] = [];
  let start = 0;
  while (text.length - start > TEXT_CHUNK) {
    let end = start + TEXT_CHUNK;
    // Never between the halves of a surrogate pair.
    const last = text.charCodeAt(end - 1);
    if (last >= 0xd800 && last <= 0xdbff) end -= 1;
    chunks.push(text.slice(start, end));
    start = end;
  }
  chunks.push(text.slice(start));
  return chunks;
}

/** Text and CSV, as a text child of a `<pre>`: never HTML, never linked, and
 * CSV is not laid out (FR-005, research.md §12). */
function TextView({ text, name }: { text: string; name: string }) {
  const chunks = useMemo(() => chunkText(text), [text]);
  const [shown, setShown] = useState(1);

  function onScroll(event: UIEvent<HTMLElement>) {
    const element = event.currentTarget;
    const left = element.scrollHeight - element.scrollTop - element.clientHeight;
    if (shown < chunks.length && left < element.clientHeight) {
      setShown((count) => Math.min(chunks.length, count + 1));
    }
  }

  return (
    <pre
      className="hd-preview__text"
      role="region"
      aria-label={name}
      tabIndex={0}
      onScroll={onScroll}
    >
      {chunks.slice(0, shown)}
    </pre>
  );
}

// --- TIFF ---------------------------------------------------------------------

const ZOOM_CHOICES = Array.from({ length: 15 }, (_, step) => 50 + step * 25);

function zoomValue(zoom: TiffZoom): string {
  return typeof zoom === "number" ? String(zoom) : zoom;
}

/** The TIFF's toolbar and its scroll area of pages (research.md §14). */
function TiffView({
  previewId,
  pages,
  name,
  onFailure,
}: {
  previewId: number;
  pages: PageSize[];
  name: string;
  onFailure: (failure: CommandFailure) => void;
}) {
  const area = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [scrollTop, setScrollTop] = useState(0);
  // The page a control scrolled to, until the user scrolls on their own: a last
  // page too short to reach the top of the view is still the page shown.
  const [forced, setForced] = useState<number | null>(null);
  // Page changes made by the controls are announced, those made by scrolling not.
  const [announce, setAnnounce] = useState(true);
  const [arrived, setArrived] = useState(false);
  const scrolledTo = useRef(0);
  const lastHeight = useRef(0);
  const percentId = useId();
  const count = pages.length;
  const multi = count > 1;

  const tiff = useTiffPages({
    previewId,
    pages,
    viewportWidth: size.width,
    viewportHeight: size.height,
    scrollTop,
    onFailure,
  });

  // The toolbar waits for page 1, and stays on once it has come.
  if (!arrived && (tiff.bitmaps.has(0) || tiff.failed.has(0))) setArrived(true);

  useLayoutEffect(() => {
    const element = area.current;
    if (!element) return;
    const measure = () =>
      setSize((now) =>
        now.width === element.clientWidth && now.height === element.clientHeight
          ? now
          : { width: element.clientWidth, height: element.clientHeight },
      );
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  function scrollTo(top: number) {
    const element = area.current;
    if (!element) return;
    element.scrollTop = top;
    scrolledTo.current = element.scrollTop;
    setScrollTop(element.scrollTop);
  }

  // A zoom or a resize changes the height of the pages: keep the same place in them.
  useLayoutEffect(() => {
    const before = lastHeight.current;
    lastHeight.current = tiff.contentHeight;
    const element = area.current;
    if (before > 0 && tiff.contentHeight !== before && element && element.scrollTop > 0) {
      scrollTo((element.scrollTop * tiff.contentHeight) / before);
    }
  }, [tiff.contentHeight]);

  const current = Math.min(forced ?? tiff.currentPage, count - 1);

  function goTo(page: number) {
    const target = Math.max(0, Math.min(count - 1, page));
    scrollTo(tiff.layout[target].top);
    setForced(target);
    setAnnounce(true);
  }

  function onScroll(event: UIEvent<HTMLDivElement>) {
    const top = event.currentTarget.scrollTop;
    setScrollTop(top);
    // The scroll a control made is not the user's own.
    if (Math.abs(top - scrolledTo.current) > 1) {
      scrolledTo.current = top;
      setForced(null);
      setAnnounce(false);
    }
  }

  function onKeyDown(event: ReactKeyboardEvent<HTMLDivElement>) {
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    const handled = (action: () => void) => {
      event.preventDefault();
      action();
    };
    switch (event.key) {
      case "PageDown":
        if (multi) handled(() => goTo(current + 1));
        break;
      case "PageUp":
        if (multi) handled(() => goTo(current - 1));
        break;
      case "Home":
        if (multi) handled(() => goTo(0));
        break;
      case "End":
        if (multi) handled(() => goTo(count - 1));
        break;
      case "+":
      case "=":
        handled(tiff.zoomIn);
        break;
      case "-":
        handled(tiff.zoomOut);
        break;
      case "0":
        handled(() => tiff.setZoom(multi ? "fitWidth" : "fitPage"));
        break;
      case "1":
        if (!multi) handled(() => tiff.setZoom(100));
        break;
    }
  }

  const choose = (value: string) =>
    tiff.setZoom(value === "fitWidth" || value === "fitPage" ? value : Number(value));
  const widest = Math.max(0, ...tiff.layout.map((box) => box.width));

  const zoomButtons = (
    <>
      <Button
        size="sm"
        variant="ghost"
        icon="minus"
        aria-label="Zoom out"
        title="Zoom out"
        aria-describedby={percentId}
        disabled={!arrived}
        onClick={tiff.zoomOut}
      />
      <Menu
        modal
        trigger={
          <Button size="sm" variant="ghost" disabled={!arrived} title="Zoom">
            <span id={percentId} className="hd-num">{`${tiff.percent}%`}</span>
            <Icon name="chevronDown" size={14} />
          </Button>
        }
      >
        <MenuRadioGroup value={zoomValue(tiff.zoom)} onValueChange={choose}>
          <MenuRadioItem value="fitWidth">Fit width</MenuRadioItem>
          <MenuRadioItem value="fitPage">Fit page</MenuRadioItem>
          {ZOOM_CHOICES.map((percent) => (
            <MenuRadioItem key={percent} value={String(percent)}>
              {`${percent}%`}
            </MenuRadioItem>
          ))}
        </MenuRadioGroup>
      </Menu>
      <Button
        size="sm"
        variant="ghost"
        icon="plus"
        aria-label="Zoom in"
        title="Zoom in"
        aria-describedby={percentId}
        disabled={!arrived}
        onClick={tiff.zoomIn}
      />
    </>
  );

  return (
    <>
      <div className="hd-preview__toolbar" role="toolbar" aria-label="Page and zoom">
        {multi && (
          <>
            <Button
              size="sm"
              variant="ghost"
              icon="pageFirst"
              aria-label="First page"
              title="First page"
              disabled={!arrived || current === 0}
              onClick={() => goTo(0)}
            />
            <Button
              size="sm"
              variant="ghost"
              icon="chevronLeft"
              aria-label="Previous page"
              title="Previous page"
              disabled={!arrived || current === 0}
              onClick={() => goTo(current - 1)}
            />
            <span className="hd-preview__page-indicator" aria-live={announce ? "polite" : "off"}>
              {`Page ${current + 1} of ${count}`}
            </span>
            <Button
              size="sm"
              variant="ghost"
              icon="chevronRight"
              aria-label="Next page"
              title="Next page"
              disabled={!arrived || current === count - 1}
              onClick={() => goTo(current + 1)}
            />
            <Button
              size="sm"
              variant="ghost"
              icon="pageLast"
              aria-label="Last page"
              title="Last page"
              disabled={!arrived || current === count - 1}
              onClick={() => goTo(count - 1)}
            />
            <span className="hd-preview__toolbar-rule" aria-hidden />
          </>
        )}
        {zoomButtons}
        <span className="hd-preview__toolbar-rule" aria-hidden />
        {multi ? (
          <>
            <Button
              size="sm"
              variant="ghost"
              icon="fitWidth"
              aria-label="Fit width"
              title="Fit width"
              disabled={!arrived}
              onClick={() => tiff.setZoom("fitWidth")}
            />
            <Button
              size="sm"
              variant="ghost"
              icon="fitPage"
              aria-label="Fit page"
              title="Fit page"
              disabled={!arrived}
              onClick={() => tiff.setZoom("fitPage")}
            />
          </>
        ) : (
          <>
            <Button
              size="sm"
              variant="ghost"
              icon="fitPage"
              aria-label="Fit"
              title="Fit"
              disabled={!arrived}
              onClick={() => tiff.setZoom("fitPage")}
            />
            <Button
              size="sm"
              variant="ghost"
              aria-label="Actual size"
              title="Actual size"
              disabled={!arrived}
              onClick={() => tiff.setZoom(100)}
            >
              1:1
            </Button>
          </>
        )}
      </div>

      <div
        ref={area}
        className="hd-preview__pages"
        role="region"
        aria-label={`${name}, pages`}
        tabIndex={0}
        onScroll={onScroll}
        onKeyDown={onKeyDown}
      >
        <div className="hd-preview__pagelist" style={{ minWidth: widest, rowGap: PAGE_GAP }}>
          {tiff.layout.map((box, page) => {
            const style = { width: box.width, height: box.height };
            const url = tiff.bitmaps.get(page);
            if (url) {
              return (
                <figure key={page} className="hd-preview__page" style={style}>
                  <img
                    src={url}
                    alt={multi ? `${name}, page ${page + 1} of ${count}` : name}
                    draggable={false}
                  />
                </figure>
              );
            }
            if (tiff.failed.has(page)) {
              return (
                <figure
                  key={page}
                  className="hd-preview__page hd-preview__page--note"
                  style={style}
                >
                  <p role="status">{`Page ${page + 1} can't be shown.`}</p>
                </figure>
              );
            }
            return (
              <figure
                key={page}
                className="hd-preview__page hd-preview__page--note"
                style={style}
                aria-busy="true"
              >
                {page === 0 && (
                  <div className="hd-preview__preparing" aria-live="polite">
                    <span className="hd-spinner" aria-hidden />
                    <span>Preparing page 1…</span>
                  </div>
                )}
              </figure>
            );
          })}
        </div>
      </div>
    </>
  );
}
