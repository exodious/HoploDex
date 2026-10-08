import { describe, expect, it } from "vitest";
import { fileName, isDocumentPath, isPhotoPath } from "./filePaths";
import type { DocumentType } from "./types";

// specs/007-document-preview/contracts/ui-document-preview.md §6, tasks.md T084:
// `isDocumentPath(path, types)` is "the extension is in the document types
// `list_document_types` gives", no longer "not a photo".
const types: DocumentType[] = [
  { label: "PDF", extensions: ["pdf"], mimeType: "application/pdf", previewKind: "pdf" },
  { label: "TIFF", extensions: ["tif", "tiff"], mimeType: "image/tiff", previewKind: "tiff" },
  { label: "Plain text", extensions: ["txt"], mimeType: "text/plain", previewKind: "text" },
  { label: "Word", extensions: ["doc", "docx"], mimeType: "application/msword", previewKind: null },
];

describe("filePaths", () => {
  it("names a file from POSIX and Windows paths", () => {
    expect(fileName("/home/sam/Pictures/range day.png")).toBe("range day.png");
    expect(fileName("C:\\Users\\Sam\\Receipt.pdf")).toBe("Receipt.pdf");
  });

  it("sends JPEG and PNG files to the photos, whatever the case, and not to the documents", () => {
    for (const path of ["/a/b.jpg", "/a/b.JPEG", "/a/b.png", "C:\\a\\B.PNG"]) {
      expect(isPhotoPath(path)).toBe(true);
      expect(isDocumentPath(path, types)).toBe(false);
    }
  });

  it("takes a path as a document when its extension is one of the document types, whatever the case", () => {
    for (const path of [
      "/a/receipt.pdf",
      "/a/RECEIPT.PDF",
      "/a/scan.tif",
      "/a/scan.TIFF",
      "/a/notes.txt",
      "/a/bill of sale.docx",
      "/a/old.doc",
      "C:\\Users\\Sam\\Receipt.pdf",
    ]) {
      expect(isPhotoPath(path)).toBe(false);
      expect(isDocumentPath(path, types)).toBe(true);
    }
  });

  it("refuses everything else: an executable, an unknown type, no extension, a bare dot-name", () => {
    for (const path of [
      "/a/setup.exe",
      "/a/scan.gif",
      "/a/appraisal",
      "/a/.pdf",
      "/a/archive.pdf.zip",
      "/a/photo.webp",
    ]) {
      expect(isDocumentPath(path, types)).toBe(false);
    }
  });

  it("follows the types it is given, with no list of its own", () => {
    expect(isDocumentPath("/a/bill.docx", types)).toBe(true);
    expect(isDocumentPath("/a/bill.docx", types.slice(0, 3))).toBe(false);
    expect(isDocumentPath("/a/receipt.pdf", [])).toBe(false);
  });
});
