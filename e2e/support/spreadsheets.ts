import fs from "node:fs";

/**
 * Spreadsheet files for the import specs, made without a library: a workbook
 * is a zip archive, written here with every part stored (not compressed).
 */

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n += 1) {
    let c = n;
    for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(bytes: Buffer): number {
  let crc = 0xffffffff;
  for (const byte of bytes) crc = CRC_TABLE[(crc ^ byte) & 0xff]! ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}

/** A zip archive of `parts` (path inside the archive → contents), stored. */
function storedZip(parts: Record<string, string>): Buffer {
  const locals: Buffer[] = [];
  const centrals: Buffer[] = [];
  let offset = 0;
  for (const [name, text] of Object.entries(parts)) {
    const nameBytes = Buffer.from(name);
    const data = Buffer.from(text);
    const crc = crc32(data);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4); // version needed
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(data.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(nameBytes.length, 26);
    locals.push(local, nameBytes, data);

    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4); // version made by
    central.writeUInt16LE(20, 6); // version needed
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(data.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(nameBytes.length, 28);
    central.writeUInt32LE(offset, 42);
    centrals.push(central, nameBytes);
    offset += local.length + nameBytes.length + data.length;
  }
  const centralBytes = Buffer.concat(centrals);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(Object.keys(parts).length, 8);
  end.writeUInt16LE(Object.keys(parts).length, 10);
  end.writeUInt32LE(centralBytes.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, centralBytes, end]);
}

function escapeXml(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

/** A column's letter in a cell reference (A, B, … Z, AA, …). */
function columnLetters(index: number): string {
  let letters = "";
  for (let n = index + 1; n > 0; n = Math.floor((n - 1) / 26)) {
    letters = String.fromCharCode(65 + ((n - 1) % 26)) + letters;
  }
  return letters;
}

/** A workbook file of `sheets`, each a list of rows of text cells, written to
 * `file`. */
function writeWorkbook(file: string, sheets: string[][][]): void {
  const main = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
  const rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
  const decl = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>';
  const numbers = sheets.map((_, i) => i + 1);
  const parts: Record<string, string> = {
    "[Content_Types].xml":
      `${decl}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">` +
      '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>' +
      '<Default Extension="xml" ContentType="application/xml"/>' +
      '<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>' +
      numbers
        .map(
          (n) =>
            `<Override PartName="/xl/worksheets/sheet${n}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>`,
        )
        .join("") +
      "</Types>",
    "_rels/.rels":
      `${decl}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">` +
      `<Relationship Id="rId1" Type="${rel}/officeDocument" Target="xl/workbook.xml"/></Relationships>`,
    "xl/workbook.xml":
      `${decl}<workbook xmlns="${main}" xmlns:r="${rel}"><sheets>` +
      numbers.map((n) => `<sheet name="Sheet${n}" sheetId="${n}" r:id="rId${n}"/>`).join("") +
      "</sheets></workbook>",
    "xl/_rels/workbook.xml.rels":
      `${decl}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">` +
      numbers
        .map(
          (n) =>
            `<Relationship Id="rId${n}" Type="${rel}/worksheet" Target="worksheets/sheet${n}.xml"/>`,
        )
        .join("") +
      "</Relationships>",
  };
  for (const n of numbers) {
    const rows = sheets[n - 1]
      .map(
        (cells, r) =>
          `<row r="${r + 1}">` +
          cells
            .map(
              (text, c) =>
                `<c r="${columnLetters(c)}${r + 1}" t="inlineStr"><is><t>${escapeXml(text)}</t></is></c>`,
            )
            .join("") +
          "</row>",
      )
      .join("");
    parts[`xl/worksheets/sheet${n}.xml`] =
      `${decl}<worksheet xmlns="${main}"><sheetData>${rows}</sheetData></worksheet>`;
  }
  fs.writeFileSync(file, storedZip(parts));
}

/**
 * Writes a workbook of `sheetCount` sheets, each with `x` in A1, to `file`.
 * With more than 16 sheets it is over the import's limit
 * (008 data-model.md "Import limits").
 */
export function writeSheetsWorkbook(file: string, sheetCount: number): void {
  writeWorkbook(
    file,
    Array.from({ length: sheetCount }, () => [["x"]]),
  );
}

/**
 * Writes a workbook of one sheet holding `rows` (the first is the header) to
 * `file`, every cell text, as the import reads a sheet exported from a
 * spreadsheet program with text cells.
 */
export function writeTableWorkbook(file: string, rows: string[][]): void {
  writeWorkbook(file, [rows]);
}
