import { useEffect, useRef, useState } from "react";
import { bytesToDataUrl } from "../../lib/bytes";
import * as mediaService from "../media/mediaService";
import { useSessionScope } from "../session/sessionScope";
import type { SessionScope } from "../session/sessionScope";
import { TypeDrawing } from "./TypeDrawing";

/** Each scope's thumbnail fetches still under way, by photo id: the tiles
 * that show one photo share one fetch rather than each asking for its bytes
 * before the first answer is cached (code scanning alert
 * untrusted-images/cwe-770/src-tauri/src/commands/photos.rs#get_photo_thumbnail). */
const loading = new WeakMap<SessionScope, Map<number, Promise<string>>>();

function loadThumbnail(scope: SessionScope, photoId: number): Promise<string> {
  let pending = loading.get(scope);
  if (!pending) {
    pending = new Map();
    loading.set(scope, pending);
  }
  const inScope = pending;
  let load = inScope.get(photoId);
  if (!load) {
    load = mediaService
      .getPhotoThumbnail(scope, photoId)
      .then((buffer) => {
        const url = bytesToDataUrl(new Uint8Array(buffer), "image/jpeg");
        scope.thumbnails.set(photoId, url);
        return url;
      })
      .finally(() => inScope.delete(photoId));
    inScope.set(photoId, load);
  }
  return load;
}

export interface FirearmThumbnailProps {
  thumbnailPhotoId: number | null;
  genericThumbnailKey: string;
  className?: string;
}

/** Renders a firearm's thumbnail: its designated photo if it has one, or
 * its type's generic drawing otherwise (001 FR-009, US4-3). Photos
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
    loadThumbnail(scope, thumbnailPhotoId)
      .then((url) => {
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
