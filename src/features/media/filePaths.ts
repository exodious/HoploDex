import type { DocumentType } from "./types";

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
 * PNG the gallery's file picker accepts. */
export function isPhotoPath(path: string): boolean {
  return PHOTO_EXTENSIONS.includes(extension(path));
}

/** Whether a dropped file is one of the document types the backend keeps
 * (`list_document_types`), by extension; the backend's content check stays the
 * authority (ui contract §6). */
export function isDocumentPath(path: string, types: DocumentType[]): boolean {
  const ext = extension(path);
  return ext !== "" && types.some((type) => type.extensions.includes(ext));
}

/** The document file input's `accept`: each type's extensions, dot-prefixed, in the
 * table's order; undefined until the types are known. */
export function documentAccept(types: DocumentType[]): string | undefined {
  return types.length === 0
    ? undefined
    : types.flatMap((type) => type.extensions.map((ext) => `.${ext}`)).join(",");
}
