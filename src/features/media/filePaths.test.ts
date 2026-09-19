import { describe, expect, it } from "vitest";
import { fileName, isDocumentPath, isPhotoPath } from "./filePaths";

describe("filePaths", () => {
  it("names a file from POSIX and Windows paths", () => {
    expect(fileName("/home/sam/Pictures/range day.png")).toBe("range day.png");
    expect(fileName("C:\\Users\\Sam\\Receipt.pdf")).toBe("Receipt.pdf");
  });

  it("sends JPEG and PNG files to the photos, whatever the case", () => {
    for (const path of ["/a/b.jpg", "/a/b.JPEG", "/a/b.png", "C:\\a\\B.PNG"]) {
      expect(isPhotoPath(path)).toBe(true);
      expect(isDocumentPath(path)).toBe(false);
    }
  });

  it("treats everything else as a document", () => {
    for (const path of [
      "/a/receipt.pdf",
      "/a/scan.gif",
      "/a/appraisal",
      "/a/.png",
      "/a/notes.txt",
    ]) {
      expect(isPhotoPath(path)).toBe(false);
      expect(isDocumentPath(path)).toBe(true);
    }
  });
});
