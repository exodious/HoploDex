import { describe, expect, it } from "vitest";
import { kindInSentence } from "./documentKind";

// specs/007-document-preview/contracts/ui-document-preview.md §3 (tasks.md T159).
describe("kindInSentence", () => {
  it.each([
    ["Plain text", "plain text"],
    ["Spreadsheet", "spreadsheet"],
    ["File", "file"],
  ])("lower-cases %s inside a sentence", (label, expected) => {
    expect(kindInSentence(label)).toBe(expected);
  });

  it.each([
    "PDF",
    "TIFF",
    "CSV",
    "RTF",
    "JPG",
    "OpenDocument text",
    "OpenDocument spreadsheet",
    "Word",
  ])("keeps %s as it is", (label) => {
    expect(kindInSentence(label)).toBe(label);
  });
});
