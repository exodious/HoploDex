import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/*
 * Regression: the 16 and 32 px entries were once packed as PNG under icp4 and
 * icp5. macOS reads those types as raw pixels, so Finder's list and small
 * icon views showed noise. The 1x small slots are ic04 and ic05, holding
 * run-length-coded ARGB, as Apple's iconutil writes them.
 */
const icns = readFileSync(join(process.cwd(), "src-tauri/icons/icon.icns"));

function entries(): Map<string, Buffer> {
  const found = new Map<string, Buffer>();
  for (let at = 8; at < icns.length;) {
    const length = icns.readUInt32BE(at + 4);
    found.set(icns.toString("ascii", at, at + 4), icns.subarray(at + 8, at + length));
    at += length;
  }
  return found;
}

/** Bytes consumed by one run-length-coded channel of `pixels` values. */
function channelLength(data: Buffer, from: number, pixels: number): number {
  let at = from;
  for (let done = 0; done < pixels;) {
    const n = data[at++];
    if (n < 128) {
      at += n + 1;
      done += n + 1;
    } else {
      at += 1;
      done += n - 125;
    }
  }
  return at - from;
}

describe("icon.icns", () => {
  it("has the sizes macOS asks for, none of them under Windows-style icp types", () => {
    expect([...entries().keys()].sort()).toEqual(
      ["ic04", "ic05", "ic07", "ic08", "ic09", "ic10", "ic11", "ic12", "ic13", "ic14"].sort(),
    );
  });

  it.each([
    ["ic04", 16],
    ["ic05", 32],
  ])("holds %s as four run-length-coded ARGB channels of %i px square", (type, size) => {
    const data = entries().get(type)!;
    expect(data.toString("ascii", 0, 4)).toBe("ARGB");
    let at = 4;
    for (let channel = 0; channel < 4; channel++) {
      at += channelLength(data, at, size * size);
    }
    expect(at).toBe(data.length);
  });

  it.each(["ic07", "ic08", "ic09", "ic10", "ic11", "ic12", "ic13", "ic14"])(
    "holds %s as a PNG",
    (type) => {
      expect(entries().get(type)!.subarray(1, 4).toString("ascii")).toBe("PNG");
    },
  );
});
