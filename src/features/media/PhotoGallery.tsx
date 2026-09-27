import { useEffect, useRef, useState } from "react";
import { pauseIdleForFileInput } from "../session/useIdleActivity";
import type { ChangeEvent } from "react";
import { Button, ConfirmDialog, Dialog, Icon, useToast } from "../../components";
import { bytesToDataUrl, fileToByteArray } from "../../lib/bytes";
import { formatDate } from "../../lib/dates";
import { CommandFailure } from "../../services/tauriClient";
import type { Firearm } from "../firearms/types";
import * as mediaService from "./mediaService";
import type { PhotoSummary } from "./types";
import { fileName, isPhotoPath } from "./filePaths";
import { useFileDrop } from "./useFileDrop";
import "./media.css";

export interface PhotoGalleryProps {
  firearm: Firearm;
  /** Called after photos change, since the firearm's thumbnail may have. */
  onChanged: () => Promise<void>;
}

const ACCEPTED_MIME_TYPES = ["image/jpeg", "image/png"];

function failureMessage(e: unknown, fallback: string): string {
  return e instanceof CommandFailure ? e.message : fallback;
}

function plural(n: number, one: string, many: string) {
  return `${n} ${n === 1 ? one : many}`;
}

/** Photos for a firearm (US4). The first photo added becomes the
 * thumbnail automatically (FR-008); the viewer lets the user pick another,
 * see the full-resolution original, or delete it. */
export function PhotoGallery({ firearm, onChanged }: PhotoGalleryProps) {
  const notify = useToast();
  const [photos, setPhotos] = useState<PhotoSummary[] | null>(null);
  const [adding, setAdding] = useState(0);
  const [viewing, setViewing] = useState<number | null>(null);
  const [deleting, setDeleting] = useState<PhotoSummary | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  // The system's file chooser gives the window no input while it is open
  // (research.md §15).
  useEffect(() => (inputRef.current ? pauseIdleForFileInput(inputRef.current) : undefined), []);

  async function load() {
    try {
      setPhotos(await mediaService.listPhotos(firearm.id));
    } catch (e) {
      notify(failureMessage(e, "Photos couldn't be loaded."), "error");
      setPhotos([]);
    }
  }

  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [firearm.id]);

  /** Adds each photo in turn, carrying on past one that fails so a bad file
   * doesn't cost the rest of a batch. */
  async function addAll(pending: { name: string; add: () => Promise<unknown> }[]) {
    if (pending.length === 0) return;
    setAdding(pending.length);
    let added = 0;
    let failure: string | null = null;
    for (const { name, add } of pending) {
      try {
        await add();
        added += 1;
      } catch (e) {
        failure ??= failureMessage(e, `${name} couldn't be added.`);
      }
    }
    setAdding(0);
    await load();
    if (added > 0) {
      await onChanged();
      notify(`Added ${plural(added, "photo", "photos")}.`);
    }
    if (failure) notify(failure, "error");
  }

  async function addFiles(files: File[]) {
    const accepted = files.filter((f) => ACCEPTED_MIME_TYPES.includes(f.type));
    const rejected = files.filter((f) => !ACCEPTED_MIME_TYPES.includes(f.type));
    if (rejected.length > 0) {
      notify(
        `${rejected.map((f) => f.name).join(", ")} ${rejected.length === 1 ? "isn't a JPEG or PNG photo, so it wasn't" : "aren't JPEG or PNG photos, so they weren't"} added.`,
        "error",
      );
    }
    await addAll(
      accepted.map((file) => ({
        name: file.name,
        add: async () =>
          mediaService.addPhoto(firearm.id, await fileToByteArray(file), file.name, file.type),
      })),
    );
  }

  async function addPaths(paths: string[]) {
    await addAll(
      paths.map((path) => ({
        name: fileName(path),
        add: () => mediaService.addPhotoFromPath(firearm.id, path),
      })),
    );
  }

  async function setThumbnail(photo: PhotoSummary) {
    try {
      await mediaService.setThumbnailPhoto(firearm.id, photo.id);
      await onChanged();
      notify("Thumbnail updated.");
    } catch (e) {
      notify(failureMessage(e, "The thumbnail couldn't be changed."), "error");
    }
  }

  async function deletePhoto(photo: PhotoSummary) {
    try {
      await mediaService.deletePhoto(photo.id, true);
      setViewing(null);
      await load();
      await onChanged();
      notify("Photo deleted.");
    } catch (e) {
      notify(failureMessage(e, "The photo couldn't be deleted."), "error");
    }
  }

  function handleInput(event: ChangeEvent<HTMLInputElement>) {
    const files = Array.from(event.target.files ?? []);
    event.target.value = "";
    void addFiles(files);
  }

  const dragging = useFileDrop(isPhotoPath, (paths) => void addPaths(paths));
  const pick = () => inputRef.current?.click();

  return (
    <section className="hd-panel hd-dropzone-host" aria-labelledby="photos-title">
      <header className="hd-panel__head">
        <h2 className="hd-panel__title" id="photos-title">
          Photos
          {photos && photos.length > 0 && (
            <span className="hd-panel__count hd-num">{photos.length}</span>
          )}
        </h2>
        <Button size="sm" icon="plus" pending={adding > 0} onClick={pick}>
          {adding > 0 ? `Adding ${adding}…` : "Add photos"}
        </Button>
        <input
          ref={inputRef}
          type="file"
          accept={ACCEPTED_MIME_TYPES.join(",")}
          multiple
          className="hd-sr-only"
          tabIndex={-1}
          aria-label="Add photos"
          onChange={handleInput}
        />
      </header>

      {photos && photos.length === 0 && (
        <button type="button" className="hd-dropzone" onClick={pick}>
          <Icon name="image" size={24} />
          <span>
            Drop photos here, or <span className="hd-link">choose files</span>
          </span>
          <span className="hd-dropzone__hint">
            JPEG or PNG. The first photo becomes the thumbnail; until then the record uses its
            type’s drawing.
          </span>
        </button>
      )}

      {photos && photos.length > 0 && (
        <ul className="hd-photos">
          {photos.map((photo, index) => (
            <li key={photo.id}>
              <button
                type="button"
                className="hd-photo"
                onClick={() => setViewing(index)}
                aria-label={`View ${photo.originalFilename}`}
              >
                <img src={bytesToDataUrl(photo.thumbnailBytes, "image/jpeg")} alt="" />
                {firearm.thumbnailPhotoId === photo.id && (
                  <span className="hd-photo__tag">
                    <Icon name="star" size={12} strokeWidth={2} />
                    <span>Thumbnail</span>
                  </span>
                )}
              </button>
            </li>
          ))}
        </ul>
      )}

      {photos && photos.length > 0 && (
        <button type="button" className="hd-dropzone hd-dropzone--compact" onClick={pick}>
          <span>
            Drop more photos here, or <span className="hd-link">choose files</span>
          </span>
        </button>
      )}

      {dragging && (
        <div className="hd-dropzone-overlay" aria-hidden>
          Drop to add photos
        </div>
      )}

      {photos && viewing != null && photos[viewing] && (
        <PhotoViewer
          photos={photos}
          index={viewing}
          onIndexChange={setViewing}
          onClose={() => setViewing(null)}
          isThumbnail={firearm.thumbnailPhotoId === photos[viewing].id}
          onSetThumbnail={() => setThumbnail(photos[viewing])}
          onDelete={() => setDeleting(photos[viewing])}
        />
      )}

      <ConfirmDialog
        open={deleting != null}
        onOpenChange={(open) => !open && setDeleting(null)}
        title="Delete this photo?"
        description={`${deleting?.originalFilename ?? "The photo"} will be permanently removed from this record.`}
        confirmLabel="Delete photo"
        onConfirm={() => (deleting ? deletePhoto(deleting) : undefined)}
      />
    </section>
  );
}

function PhotoViewer({
  photos,
  index,
  onIndexChange,
  onClose,
  isThumbnail,
  onSetThumbnail,
  onDelete,
}: {
  photos: PhotoSummary[];
  index: number;
  onIndexChange: (index: number) => void;
  onClose: () => void;
  isThumbnail: boolean;
  onSetThumbnail: () => void;
  onDelete: () => void;
}) {
  const photo = photos[index];
  const [originalUrl, setOriginalUrl] = useState<string | null>(null);
  const hasPrev = index > 0;
  const hasNext = index < photos.length - 1;

  useEffect(() => {
    let url: string | null = null;
    let cancelled = false;
    setOriginalUrl(null);
    mediaService
      .getPhotoOriginal(photo.id)
      .then((buffer) => {
        if (cancelled) return;
        url = URL.createObjectURL(new Blob([buffer], { type: photo.mimeType }));
        setOriginalUrl(url);
      })
      .catch(() => {
        // The thumbnail stays on screen; nothing else to do.
      });
    return () => {
      cancelled = true;
      if (url) URL.revokeObjectURL(url);
    };
  }, [photo.id, photo.mimeType]);

  return (
    <Dialog
      open
      onOpenChange={(open) => !open && onClose()}
      title={photo.originalFilename}
      description={`Photo ${index + 1} of ${photos.length} · added ${formatDate(photo.createdAt.slice(0, 10))}`}
      size="lg"
      footer={
        <>
          <div className="hd-viewer__nav">
            <Button
              size="sm"
              icon="chevronLeft"
              aria-label="Previous photo"
              disabled={!hasPrev}
              onClick={() => onIndexChange(index - 1)}
            />
            <Button
              size="sm"
              icon="chevronRight"
              aria-label="Next photo"
              disabled={!hasNext}
              onClick={() => onIndexChange(index + 1)}
            />
          </div>
          <Button variant="ghost" icon="trash" onClick={onDelete}>
            Delete photo
          </Button>
          <Button
            variant={isThumbnail ? "secondary" : "primary"}
            icon="star"
            disabled={isThumbnail}
            onClick={onSetThumbnail}
          >
            {isThumbnail ? "Current thumbnail" : "Use as thumbnail"}
          </Button>
        </>
      }
    >
      <div
        className="hd-viewer"
        onKeyDown={(e) => {
          if (e.key === "ArrowLeft" && hasPrev) onIndexChange(index - 1);
          if (e.key === "ArrowRight" && hasNext) onIndexChange(index + 1);
        }}
      >
        <img
          src={originalUrl ?? bytesToDataUrl(photo.thumbnailBytes, "image/jpeg")}
          alt={`Photo ${index + 1} of ${photos.length}`}
          className={
            originalUrl ? "hd-viewer__image" : "hd-viewer__image hd-viewer__image--loading"
          }
        />
      </div>
    </Dialog>
  );
}
