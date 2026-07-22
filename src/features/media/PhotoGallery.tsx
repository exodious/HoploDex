import { useEffect, useState } from "react";
import { Button, ConfirmDialog } from "../../components";
import { bytesToDataUrl, fileToByteArray } from "../../lib/bytes";
import { CommandFailure } from "../../services/tauriClient";
import * as firearmsService from "../firearms/firearmsService";
import type { Firearm } from "../firearms/types";
import * as mediaService from "./mediaService";
import type { PhotoSummary } from "./types";

export interface PhotoGalleryProps {
  firearm: Firearm;
  onFirearmUpdated: (firearm: Firearm) => void;
}

const ACCEPTED_MIME_TYPES = ["image/jpeg", "image/png"];

/** Photo attachment and thumbnail-picker for a firearm (US4). Every photo
 * that has none becomes the thumbnail automatically on upload (FR-008);
 * this control also lets the user explicitly pick a different one. */
export function PhotoGallery({ firearm, onFirearmUpdated }: PhotoGalleryProps) {
  const [photos, setPhotos] = useState<PhotoSummary[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<number | null>(null);

  async function refresh() {
    try {
      setPhotos(await mediaService.listPhotos(firearm.id));
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "Failed to load photos.");
    }
  }

  useEffect(() => {
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [firearm.id]);

  async function handleFileSelected(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    if (!ACCEPTED_MIME_TYPES.includes(file.type)) {
      setError("Unsupported photo type. Use JPEG or PNG.");
      return;
    }
    setError(null);
    try {
      const bytes = await fileToByteArray(file);
      await mediaService.addPhoto(firearm.id, bytes, file.name, file.type);
      await refresh();
      // The first photo added auto-becomes the thumbnail (FR-008) — refetch
      // so the caller's cached firearm reflects that.
      onFirearmUpdated(await firearmsService.getFirearm(firearm.id));
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "Failed to add this photo.");
    }
  }

  async function handleSetThumbnail(photoId: number) {
    setError(null);
    try {
      const updated = await mediaService.setThumbnailPhoto(firearm.id, photoId);
      onFirearmUpdated(updated);
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "Failed to set the thumbnail.");
    }
  }

  async function handleDelete(photoId: number) {
    setError(null);
    try {
      await mediaService.deletePhoto(photoId, true);
      await refresh();
      // A deleted thumbnail falls back server-side (next-oldest photo, or
      // null); re-fetch so the caller's cached firearm stays in sync.
      onFirearmUpdated(await firearmsService.getFirearm(firearm.id));
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "Failed to delete this photo.");
    } finally {
      setDeletingId(null);
    }
  }

  return (
    <div>
      <h3>Photos</h3>
      {error && (
        <p className="hd-field__error" role="alert">
          {error}
        </p>
      )}
      {photos.length === 0 && (
        <p>No photos yet — this firearm shows its type's generic thumbnail.</p>
      )}
      <div style={{ display: "flex", gap: 12, flexWrap: "wrap" }}>
        {photos.map((photo) => {
          const isThumbnail = firearm.thumbnailPhotoId === photo.id;
          return (
            <div
              key={photo.id}
              style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 4 }}
            >
              <img
                src={bytesToDataUrl(photo.thumbnailBytes, "image/jpeg")}
                alt={photo.originalFilename}
                width={96}
                height={96}
                style={{ objectFit: "cover", borderRadius: "var(--hd-radius)" }}
              />
              {isThumbnail ? (
                <span>Thumbnail</span>
              ) : (
                <Button variant="secondary" onClick={() => handleSetThumbnail(photo.id)}>
                  Set as thumbnail
                </Button>
              )}
              <Button variant="danger" onClick={() => setDeletingId(photo.id)}>
                Delete
              </Button>
            </div>
          );
        })}
      </div>

      <input
        type="file"
        accept={ACCEPTED_MIME_TYPES.join(",")}
        onChange={handleFileSelected}
        aria-label="Add photo"
      />

      <ConfirmDialog
        open={deletingId != null}
        onOpenChange={(open) => !open && setDeletingId(null)}
        title="Delete this photo?"
        description="This permanently removes the photo. This cannot be undone."
        confirmLabel="Delete"
        onConfirm={() => {
          if (deletingId != null) void handleDelete(deletingId);
        }}
      />
    </div>
  );
}
