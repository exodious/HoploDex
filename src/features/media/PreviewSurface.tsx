// The viewer's PDF page area: the placeholder region the PDF surface (a child
// web view the backend places over the window) is laid over. Contract:
// specs/007-document-preview/contracts/ui-document-preview.md §2 ("Page
// area"), research.md §3 and §15.
import { useCallback, useEffect, useRef, useState } from "react";
import { onPdfReady, setPreviewBounds } from "./mediaService";

/** Anything drawn above the viewer hides the surface, which is a separate
 * web view and would otherwise cover it: the delete ConfirmDialog, a Menu, a
 * toast. The page is asked, so no screen has to say so. */
const COVERS = '[role="alertdialog"], [role="menu"], .hd-toast';

export interface PreviewSurfaceProps {
  previewId: number;
  /** The document's name, for the label and the progress line. */
  name: string;
}

export function PreviewSurface({ previewId, name }: PreviewSurfaceProps) {
  const region = useRef<HTMLDivElement>(null);
  const [ready, setReady] = useState(false);
  const sent = useRef("");

  const sendBounds = useCallback(() => {
    const element = region.current;
    if (!element) return;
    const { x, y, width, height } = element.getBoundingClientRect();
    // The backend refuses a size under 1 px; a collapsed area has nothing to show.
    if (!(width >= 1) || !(height >= 1)) return;
    const visible = document.querySelector(COVERS) === null;
    const key = `${previewId}|${x}|${y}|${width}|${height}|${visible}`;
    if (key === sent.current) return;
    sent.current = key;
    setPreviewBounds(previewId, { x, y, width, height }, visible).catch(() => {
      // PREVIEW_CLOSED: the viewer is going away, or the lock closed it.
    });
  }, [previewId]);

  useEffect(() => {
    setReady(false);
    return onPdfReady(previewId, () => setReady(true));
  }, [previewId]);

  useEffect(() => {
    sent.current = "";
    sendBounds();
    const observer = new ResizeObserver(sendBounds);
    if (region.current) observer.observe(region.current);
    const covers = new MutationObserver(sendBounds);
    covers.observe(document.body, { childList: true, subtree: true });
    window.addEventListener("resize", sendBounds);
    // The dialog this sits in animates in, moving the area without resizing it.
    document.addEventListener("animationend", sendBounds, true);
    document.addEventListener("transitionend", sendBounds, true);
    return () => {
      observer.disconnect();
      covers.disconnect();
      window.removeEventListener("resize", sendBounds);
      document.removeEventListener("animationend", sendBounds, true);
      document.removeEventListener("transitionend", sendBounds, true);
    };
  }, [sendBounds]);

  return (
    <div
      ref={region}
      className="hd-preview__surface"
      role="region"
      aria-label={`${name}, PDF`}
    >
      <span className="hd-sr-only">
        Press F6 to move into the document, and F6 again to come back.
      </span>
      <div className="hd-preview__preparing" aria-live="polite">
        {!ready && (
          <>
            <span className="hd-spinner" aria-hidden />
            <span>{`Preparing ${name}…`}</span>
          </>
        )}
      </div>
    </div>
  );
}
