import { describe, expect, it } from "vitest";
import { begin, busyCount, end, track } from "./busy";

describe("busy counter", () => {
  it("counts begins and ends, and shows the count as window.__hoplodexBusy", () => {
    expect(window.__hoplodexBusy).toBe(0);
    begin();
    begin();
    expect(busyCount()).toBe(2);
    expect(window.__hoplodexBusy).toBe(2);
    end();
    expect(window.__hoplodexBusy).toBe(1);
    end();
    expect(window.__hoplodexBusy).toBe(0);
  });

  it("never goes below zero", () => {
    end();
    expect(busyCount()).toBe(0);
    expect(window.__hoplodexBusy).toBe(0);
  });

  it("track holds the count until the work settles, whichever way it ends", async () => {
    let finish!: (value: string) => void;
    const slow = track(() => new Promise<string>((resolve) => (finish = resolve)));
    expect(window.__hoplodexBusy).toBe(1);
    finish("done");
    await expect(slow).resolves.toBe("done");
    expect(window.__hoplodexBusy).toBe(0);

    await expect(track(() => Promise.reject(new Error("boom")))).rejects.toThrow("boom");
    expect(window.__hoplodexBusy).toBe(0);
  });
});
