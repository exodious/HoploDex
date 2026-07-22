import { useEffect, useState } from "react";
import { bytesToDataUrl } from "../../lib/bytes";
import * as mediaService from "../media/mediaService";

export interface FirearmThumbnailProps {
  thumbnailPhotoId: number | null;
  genericThumbnailKey: string;
  alt: string;
  size?: number;
}

// Thumbnails never change after upload, and there's only a handful of
// generic per-type images — a simple in-memory cache avoids re-fetching
// the same bytes on every browse re-render (no need for a fuller
// data-fetching library at this scale).
const photoThumbnailCache = new Map<number, string>();
const genericThumbnailCache = new Map<string, string>();

/** Renders a firearm's thumbnail: its designated photo if it has one, or
 * its type's bundled generic thumbnail otherwise (FR-009, US4 Scenario 3). */
export function FirearmThumbnail({
  thumbnailPhotoId,
  genericThumbnailKey,
  alt,
  size = 48,
}: FirearmThumbnailProps) {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      if (thumbnailPhotoId != null) {
        const cached = photoThumbnailCache.get(thumbnailPhotoId);
        if (cached) {
          setSrc(cached);
          return;
        }
        try {
          const bytes = await mediaService.getPhotoThumbnail(thumbnailPhotoId);
          const url = bytesToDataUrl(bytes, "image/jpeg");
          photoThumbnailCache.set(thumbnailPhotoId, url);
          if (!cancelled) setSrc(url);
          return;
        } catch {
          // fall through to the generic thumbnail
        }
      }

      const cached = genericThumbnailCache.get(genericThumbnailKey);
      if (cached) {
        setSrc(cached);
        return;
      }
      try {
        const bytes = await mediaService.getGenericThumbnail(genericThumbnailKey);
        const url = bytesToDataUrl(bytes, "image/png");
        genericThumbnailCache.set(genericThumbnailKey, url);
        if (!cancelled) setSrc(url);
      } catch {
        if (!cancelled) setSrc(null);
      }
    }

    void load();
    return () => {
      cancelled = true;
    };
  }, [thumbnailPhotoId, genericThumbnailKey]);

  if (!src) {
    return <div aria-hidden style={{ width: size, height: size }} />;
  }
  return (
    <img
      src={src}
      alt={alt}
      width={size}
      height={size}
      style={{ objectFit: "cover", borderRadius: "var(--hd-radius)" }}
    />
  );
}
