import fs from "node:fs";
import path from "node:path";

/**
 * A valid PDF of about a given size, for the preview's speed check (SC-001:
 * a document of about 10 MB shows within 1 s), with no file checked in.
 *
 * Each page draws one image, an RGB bitmap stored without a filter and filled
 * with pseudo-random bytes, so nothing about it compresses: the file is as big
 * as the bitmap bytes it holds, whatever a tool might do to it. The bytes are
 * deterministic (a xorshift generator from a fixed seed), so two runs attach
 * the same document. It has a real cross-reference table and trailer, so every
 * PDF viewer opens it.
 */

const IMAGE_WIDTH = 1000; // pixels; 3 bytes each, so 3000 bytes a row

export interface LargePdfOptions {
  /** The file's size to aim for, in bytes. */
  targetBytes?: number;
  /** How many pages share those bytes. */
  pages?: number;
}

/** The PDF, whose size is within a few kilobytes of `targetBytes`. */
export function largePdf({
  targetBytes = 10 * 1024 * 1024,
  pages = 4,
}: LargePdfOptions = {}): Buffer {
  // A little over 1 KB of structure per page, plus the header and trailer.
  const bitmapBytesPerPage = Math.floor((targetBytes - 2048 - pages * 512) / pages);
  const height = Math.max(1, Math.floor(bitmapBytesPerPage / (IMAGE_WIDTH * 3)));
  const bitmapLength = IMAGE_WIDTH * height * 3;

  const parts: Buffer[] = [];
  const offsets: number[] = []; // offsets[n] is where object n starts
  let length = 0;
  const push = (data: Buffer | string) => {
    const buffer = typeof data === "string" ? Buffer.from(data, "latin1") : data;
    parts.push(buffer);
    length += buffer.length;
  };
  const object = (number: number, body: Buffer | string) => {
    offsets[number] = length;
    push(`${number} 0 obj\n`);
    push(body);
    push("\nendobj\n");
  };

  // Objects: 1 catalog, 2 page tree, then for each page its page, its content
  // stream and its image.
  const pageObject = (i: number) => 3 + i * 3;
  push("%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
  object(1, "<< /Type /Catalog /Pages 2 0 R >>");
  object(
    2,
    `<< /Type /Pages /Count ${pages} /Kids [${Array.from({ length: pages }, (_, i) => `${pageObject(i)} 0 R`).join(" ")}] >>`,
  );

  let state = 0x9e3779b9;
  const noise = (size: number): Buffer => {
    const bytes = Buffer.allocUnsafe(size);
    for (let i = 0; i < size; i++) {
      state ^= state << 13;
      state >>>= 0;
      state ^= state >>> 17;
      state ^= state << 5;
      state >>>= 0;
      bytes[i] = state & 0xff;
    }
    return bytes;
  };

  for (let i = 0; i < pages; i++) {
    const page = pageObject(i);
    const content = `q 612 0 0 792 0 0 cm /Im${i} Do Q`;
    object(
      page,
      `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] ` +
        `/Resources << /XObject << /Im${i} ${page + 2} 0 R >> >> /Contents ${page + 1} 0 R >>`,
    );
    object(page + 1, `<< /Length ${content.length} >>\nstream\n${content}\nendstream`);
    offsets[page + 2] = length;
    push(`${page + 2} 0 obj\n`);
    push(
      `<< /Type /XObject /Subtype /Image /Width ${IMAGE_WIDTH} /Height ${height} ` +
        `/ColorSpace /DeviceRGB /BitsPerComponent 8 /Length ${bitmapLength} >>\nstream\n`,
    );
    push(noise(bitmapLength));
    push("\nendstream\nendobj\n");
  }

  const count = 3 + pages * 3; // objects 0 to count - 1
  const xref = length;
  let table = `xref\n0 ${count}\n0000000000 65535 f \n`;
  for (let n = 1; n < count; n++) table += `${String(offsets[n]).padStart(10, "0")} 00000 n \n`;
  push(table);
  push(`trailer\n<< /Size ${count} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`);
  return Buffer.concat(parts);
}

/** Writes {@link largePdf} into `directory` (the spec's sandbox) and returns
 * its path, for `add_document_from_path`. */
export function writeLargePdf(
  directory: string,
  name = "Large appraisal.pdf",
  options: LargePdfOptions = {},
): string {
  fs.mkdirSync(directory, { recursive: true });
  const file = path.join(directory, name);
  fs.writeFileSync(file, largePdf(options));
  return file;
}
