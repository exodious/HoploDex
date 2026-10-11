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

/**
 * Writes a workbook of `sheetCount` sheets, each with `x` in A1, to `file`.
 * With more than 16 sheets it is over the import's limit
 * (data-model.md "Import limits").
 */
export function writeSheetsWorkbook(file: string, sheetCount: number): void {
  const main = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
  const rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
  const decl = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>';
  const numbers = Array.from({ length: sheetCount }, (_, i) => i + 1);
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
    parts[`xl/worksheets/sheet${n}.xml`] =
      `${decl}<worksheet xmlns="${main}"><sheetData><row r="1">` +
      '<c r="A1" t="inlineStr"><is><t>x</t></is></c></row></sheetData></worksheet>';
  }
  fs.writeFileSync(file, storedZip(parts));
}
