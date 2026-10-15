import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

// There is no Tauri backend in jsdom: events never arrive unless a test
// mocks this module itself to send them.
vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => {}) }));

// jsdom does not implement ResizeObserver; Radix UI primitives (Checkbox,
// Select) use it for layout measurement.
if (typeof globalThis.ResizeObserver === "undefined") {
  globalThis.ResizeObserver = class ResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}

// jsdom does not implement the Pointer Capture / scrollIntoView APIs that
// Radix UI's Select relies on for its popover positioning and interaction.
if (!Element.prototype.hasPointerCapture) {
  Element.prototype.hasPointerCapture = () => false;
}
if (!Element.prototype.setPointerCapture) {
  Element.prototype.setPointerCapture = () => {};
}
if (!Element.prototype.releasePointerCapture) {
  Element.prototype.releasePointerCapture = () => {};
}
if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = () => {};
}

// jsdom has no Web Workers: the passphrase strength hint's zxcvbn runs in
// this thread instead (a test can mock this module itself to see a failure).
vi.mock("../components/strength/spawn", async () => {
  const { InProcessStrengthWorker } = await import("./inProcessStrengthWorker");
  return { spawnStrengthWorker: () => new InProcessStrengthWorker() as unknown as Worker };
});
