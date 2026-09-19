const PHOTO_EXTENSIONS = ["jpg", "jpeg", "png"];

function extension(path: string): string {
  const name = fileName(path);
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

/** The last segment of a path, whichever separator the platform uses. */
export function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** Whether a dropped file belongs in the photo gallery: the same JPEG and
 * PNG the gallery's file picker accepts. Everything else is a document. */
export function isPhotoPath(path: string): boolean {
  return PHOTO_EXTENSIONS.includes(extension(path));
}

export function isDocumentPath(path: string): boolean {
  return !isPhotoPath(path);
}
