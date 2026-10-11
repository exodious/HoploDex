import { describe, expect, it } from "vitest";
import {
  MAX_LONG_TEXT_CHARS,
  MAX_SHORT_TEXT_CHARS,
  countCharacters,
  textLengthError,
} from "./textLimits";

// specs/008-hardening-batch research.md §10: the frontend mirrors the
// backend's two maximums and its check (`check_text_length`), so a field
// shows its error at once. The backend stays the authority.

describe("the two maximums", () => {
  it("are 200 for short text and 4,000 for long text", () => {
    expect(MAX_SHORT_TEXT_CHARS).toBe(200);
    expect(MAX_LONG_TEXT_CHARS).toBe(4000);
  });
});

describe("countCharacters", () => {
  it("counts Unicode scalar values, not UTF-16 units or bytes", () => {
    expect(countCharacters("abc")).toBe(3);
    expect(countCharacters("😀")).toBe(1);
    expect(countCharacters("😀".repeat(10))).toBe(10);
    expect(countCharacters("é")).toBe(1);
    expect(countCharacters("")).toBe(0);
  });

  it("counts the text as it is, whitespace included", () => {
    expect(countCharacters("  a b  ")).toBe(7);
  });
});

describe("textLengthError", () => {
  it("accepts text up to the maximum", () => {
    expect(textLengthError("Nickname", "a".repeat(200), MAX_SHORT_TEXT_CHARS)).toBeUndefined();
    expect(textLengthError("Notes", "a".repeat(4000), MAX_LONG_TEXT_CHARS)).toBeUndefined();
    expect(textLengthError("Notes", "", MAX_LONG_TEXT_CHARS)).toBeUndefined();
  });

  it("refuses one character more, with the backend's message", () => {
    expect(textLengthError("Nickname", "a".repeat(201), MAX_SHORT_TEXT_CHARS)).toBe(
      "Nickname can be at most 200 characters.",
    );
    expect(textLengthError("Notes", "a".repeat(4001), MAX_LONG_TEXT_CHARS)).toBe(
      "Notes can be at most 4,000 characters.",
    );
  });

  it("counts after trimming", () => {
    expect(
      textLengthError("Nickname", `  ${"a".repeat(200)} \n`, MAX_SHORT_TEXT_CHARS),
    ).toBeUndefined();
    expect(textLengthError("Nickname", `  ${"a".repeat(201)}`, MAX_SHORT_TEXT_CHARS)).toBe(
      "Nickname can be at most 200 characters.",
    );
  });

  it("counts a four-byte character as one", () => {
    expect(textLengthError("Nickname", "😀".repeat(200), MAX_SHORT_TEXT_CHARS)).toBeUndefined();
    expect(textLengthError("Nickname", "😀".repeat(201), MAX_SHORT_TEXT_CHARS)).toBe(
      "Nickname can be at most 200 characters.",
    );
  });
});
