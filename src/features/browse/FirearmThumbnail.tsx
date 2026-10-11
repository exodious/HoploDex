import { useEffect, useRef, useState } from "react";
import { bytesToDataUrl } from "../../lib/bytes";
import * as mediaService from "../media/mediaService";
import { useSessionScope } from "../session/sessionScope";
import { TypeDrawing } from "./TypeDrawing";

export interface FirearmThumbnailProps {
  thumbnailPhotoId: number | null;
  genericThumbnailKey: string;
  className?: string;
}

/** Renders a firearm's thumbnail: its designated photo if it has one, or
 * its type's generic drawing otherwise (FR-009, US4 Scenario 3). Photos
 * load only once the frame scrolls near the viewport, so a long
 * collection doesn't issue one IPC call per row up front. Thumbnails never
 * change after upload, so the session scope keeps the ones loaded
 * (`scope.thumbnails`) to avoid re-fetching the same bytes on every browse
 * re-render; they go with the session, and an image that arrives after it
 * ended is neither cached nor shown (specs/008 research.md §4). */
export function FirearmThumbnail({
  thumbnailPhotoId,
  genericThumbnailKey,
  className,
}: FirearmThumbnailProps) {
  const scope = useSessionScope();
  const frameRef = useRef<HTMLDivElement>(null);
  const [nearViewport, setNearViewport] = useState(false);
  const [src, setSrc] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    const frame = frameRef.current;
    if (!frame || thumbnailPhotoId == null) return;
    if (typeof IntersectionObserver === "undefined") {
      setNearViewport(true);
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          setNearViewport(true);
          observer.disconnect();
        }
      },
      { rootMargin: "300px" },
    );
    observer.observe(frame);
    return () => observer.disconnect();
  }, [thumbnailPhotoId]);

  useEffect(() => {
    setFailed(false);
    if (thumbnailPhotoId == null) {
      setSrc(null);
      return;
    }
    const cached = scope.thumbnails.get(thumbnailPhotoId);
    if (cached) {
      setSrc(cached);
      return;
    }
    setSrc(null);
    if (!nearViewport) return;

    let cancelled = false;
    mediaService
      .getPhotoThumbnail(scope, thumbnailPhotoId)
      .then((bytes) => {
        const url = bytesToDataUrl(bytes, "image/jpeg");
        scope.thumbnails.set(thumbnailPhotoId, url);
        if (!cancelled) setSrc(url);
      })
      .catch(() => {
        if (!cancelled) setFailed(true);
      });
    return () => {
      cancelled = true;
    };
  }, [scope, thumbnailPhotoId, nearViewport]);

  const showDrawing = thumbnailPhotoId == null || failed;
  return (
    <div ref={frameRef} className={["hd-thumb", className].filter(Boolean).join(" ")}>
      {showDrawing ? (
        <TypeDrawing typeKey={genericThumbnailKey} />
      ) : (
        src && <img src={src} alt="" className="hd-thumb__photo" />
      )}
    </div>
  );
}
