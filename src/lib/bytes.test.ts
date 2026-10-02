import { describe, expect, it } from "vitest";
import { fileToByteArray } from "./bytes";

describe("fileToByteArray", () => {
  it("returns the file's bytes as a plain array", async () => {
    const file = new File([new Uint8Array([1, 2, 255])], "x.bin");
    expect(await fileToByteArray(file)).toEqual([1, 2, 255]);
  });

  it("counts the app as busy while it reads, and not after", async () => {
    const file = new File([new Uint8Array([1])], "x.bin");
    const reading = fileToByteArray(file);
    expect(window.__hoplodexBusy).toBe(1);
    await reading;
    expect(window.__hoplodexBusy).toBe(0);
  });
});
