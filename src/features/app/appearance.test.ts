import { beforeEach, describe, expect, it, vi } from "vitest";

const backend = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock("../../services/tauriClient", () => ({ invoke: backend.invoke }));

// Code scanning alert plaintext-lifetime/cwe-922: the main web view keeps no
// storage, so the color mode and layout come from Rust at start and are saved
// through `set_appearance` into machine.json.
describe("the page's appearance", () => {
  beforeEach(() => {
    vi.resetModules();
    backend.invoke.mockReset().mockResolvedValue(undefined);
  });

  it("starts from what Rust handed over, and from the defaults without it", async () => {
    delete (window as { __HOPLODEX_APPEARANCE__?: unknown }).__HOPLODEX_APPEARANCE__;
    expect((await import("./appearance")).loadAppearance()).toEqual({
      theme: "auto",
      browseView: "list",
    });

    vi.resetModules();
    (window as { __HOPLODEX_APPEARANCE__?: unknown }).__HOPLODEX_APPEARANCE__ = {
      theme: "dark",
      browseView: "tile",
    };
    expect((await import("./appearance")).loadAppearance()).toEqual({
      theme: "dark",
      browseView: "tile",
    });
    delete (window as { __HOPLODEX_APPEARANCE__?: unknown }).__HOPLODEX_APPEARANCE__;
  });

  it("saves a change whole, and nothing when nothing changed", async () => {
    const { loadAppearance, saveAppearance } = await import("./appearance");
    saveAppearance({ browseView: "list" });
    expect(backend.invoke).not.toHaveBeenCalled();

    saveAppearance({ theme: "light" });
    expect(backend.invoke).toHaveBeenCalledWith("set_appearance", {
      appearance: { theme: "light", browseView: "list" },
    });
    expect(loadAppearance().theme).toBe("light");
  });

  it("keeps the change for this run when the save fails", async () => {
    backend.invoke.mockRejectedValue(new Error("no backend"));
    const { loadAppearance, saveAppearance } = await import("./appearance");
    saveAppearance({ browseView: "tile" });
    await Promise.resolve();
    expect(loadAppearance().browseView).toBe("tile");
  });
});
