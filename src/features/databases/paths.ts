// Path text as the chooser and create dialog show it. Paths come from the
// backend or a native picker and are only displayed or passed back, so
// splitting on either separator is enough; nothing here touches a file.

const SEPARATORS = /[\\/]/;

/** `folder` joined with `child`, using the separator `folder` already uses. */
export function joinPath(folder: string, child: string): string {
  const separator = folder.includes("\\") && !folder.includes("/") ? "\\" : "/";
  const trimmed = folder.length > 1 ? folder.replace(/[\\/]+$/, "") : folder;
  return trimmed.endsWith(separator) ? `${trimmed}${child}` : `${trimmed}${separator}${child}`;
}

/** The folder holding `path`. */
export function folderOf(path: string): string {
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  if (cut < 0) return "";
  return cut === 0 ? path.slice(0, 1) : path.slice(0, cut);
}

/** A database's name: its file name without the extension. */
export function databaseNameOf(path: string): string {
  const file = path.split(SEPARATORS).pop() ?? path;
  const dot = file.lastIndexOf(".");
  return dot > 0 ? file.slice(0, dot) : file;
}

/** `text` shortened to `max` characters by replacing its middle with "…",
 * so both the start and the end of a long folder stay readable. */
export function middleTruncate(text: string, max = 56): string {
  const chars = [...text];
  if (chars.length <= max) return text;
  const keep = max - 1;
  const head = Math.ceil(keep / 2);
  return `${chars.slice(0, head).join("")}…${chars.slice(chars.length - (keep - head)).join("")}`;
}
