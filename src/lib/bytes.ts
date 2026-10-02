import { track } from "./busy";

/** Reads a browser `File` into a plain number array — the shape a Tauri
 * command's `Vec<u8>` parameter deserializes from over IPC (a typed array
 * would serialize as an object with numeric keys, not a JSON array). The app
 * counts as busy (`lib/busy.ts`) while it reads, as the upload that follows
 * is one more call. */
export async function fileToByteArray(file: File): Promise<number[]> {
  return track(async () => Array.from(new Uint8Array(await file.arrayBuffer())));
}

/** Converts bytes returned from a Tauri command into a data: URL for
 * rendering (e.g. an `<img>` thumbnail) or opening (e.g. a PDF document). */
export function bytesToDataUrl(bytes: number[], mimeType: string): string {
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return `data:${mimeType};base64,${btoa(binary)}`;
}

const SIZE_UNITS = ["bytes", "KB", "MB", "GB", "TB"];

/** A file size as people read one ("212 MB", "1.4 GB"), in the decimal
 * units file managers show. */
export function formatBytes(bytes: number, locale?: string): string {
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < SIZE_UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  const digits = unit > 0 && value < 10 ? 1 : 0;
  const number = new Intl.NumberFormat(locale, { maximumFractionDigits: digits }).format(value);
  return `${number} ${SIZE_UNITS[unit]}`;
}
