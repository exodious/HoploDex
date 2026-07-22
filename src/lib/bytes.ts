/** Reads a browser `File` into a plain number array — the shape a Tauri
 * command's `Vec<u8>` parameter deserializes from over IPC (a typed array
 * would serialize as an object with numeric keys, not a JSON array). */
export async function fileToByteArray(file: File): Promise<number[]> {
  const buffer = await file.arrayBuffer();
  return Array.from(new Uint8Array(buffer));
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
