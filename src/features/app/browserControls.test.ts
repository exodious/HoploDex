import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  allowsContextMenu,
  blocksKey,
  installBrowserControls,
  isEditableText,
  isNavigationButton,
  keyTable,
  platformFrom,
  type KeyChord,
  type Platform,
} from "./browserControls";

/** A chord: "Ctrl+Shift+r" names the modifiers, then the `key`. "Mod" is
 * Ctrl on Windows and Linux and ⌘ on macOS; "Ctrl" is always the control key
 * and "Meta" always the meta key. */
function chord(platform: Platform, spec: string): KeyChord {
  const parts = spec.split("+");
  const key = parts.pop() as string;
  const has = (name: string) => parts.includes(name);
  const mac = platform === "macos";
  return {
    key,
    ctrlKey: has("Ctrl") || (has("Mod") && !mac),
    metaKey: has("Meta") || (has("Mod") && mac),
    altKey: has("Alt"),
    shiftKey: has("Shift"),
  };
}

const blocked = (platform: Platform, spec: string, inEditableText = false) =>
  blocksKey(platform, chord(platform, spec), inEditableText);

describe("platformFrom", () => {
  it("tells the three web views' user agents apart", () => {
    expect(
      platformFrom("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15"),
    ).toBe("macos");
    expect(platformFrom("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Edg/130.0")).toBe("windows");
    expect(platformFrom("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15")).toBe("linux");
  });
});

describe("the key table", () => {
  const everywhere = [
    "F5",
    "Shift+F5",
    "Mod+F5",
    "Mod+r",
    "Mod+Shift+R",
    "BrowserBack",
    "BrowserForward",
    "BrowserRefresh",
    "BrowserStop",
    "BrowserHome",
    "Mod+p",
    "Mod+s",
    "Mod+u",
    "Mod+f",
    "Mod+g",
    "Mod+Shift+g",
    "F3",
    "F7",
    "F12",
    "Mod+Shift+I",
    "Mod+Shift+J",
    "Mod+Shift+C",
  ];
  const platforms: Platform[] = ["windows", "linux", "macos"];

  for (const platform of platforms) {
    it(`blocks the reload, print, find and developer-tools keys on ${platform}`, () => {
      for (const spec of everywhere) expect(blocked(platform, spec), spec).toBe(true);
    });

    it(`lets the app's keys and the editing keys through on ${platform}`, () => {
      const through = [
        "/",
        "Mod+l",
        "Escape",
        "Mod+c",
        "Mod+x",
        "Mod+v",
        "Mod+z",
        "Mod+y",
        "Mod+Shift+z",
        "Mod+a",
        "Shift+ArrowLeft",
        "Shift+ArrowRight",
        "Shift+ArrowUp",
        "Shift+ArrowDown",
        "Shift+Home",
        "Shift+End",
        "Home",
        "End",
        "ArrowLeft",
        "Tab",
        "Enter",
        "Alt+F4",
      ];
      for (const spec of through) expect(blocked(platform, spec, true), spec).toBe(false);
      for (const spec of through) expect(blocked(platform, spec, false), spec).toBe(false);
    });
  }

  it("blocks Alt+←/→ on Windows and Linux only", () => {
    for (const platform of ["windows", "linux"] as const) {
      expect(blocked(platform, "Alt+ArrowLeft"), platform).toBe(true);
      expect(blocked(platform, "Alt+ArrowRight"), platform).toBe(true);
      // In text too: Back isn't wanted there either.
      expect(blocked(platform, "Alt+ArrowLeft", true), platform).toBe(true);
    }
    expect(blocked("macos", "Alt+ArrowLeft")).toBe(false);
    expect(blocked("macos", "Alt+ArrowRight")).toBe(false);
  });

  it("moves by word on macOS: Option+←/→ with and without Shift, in text and elsewhere", () => {
    for (const spec of [
      "Alt+ArrowLeft",
      "Alt+ArrowRight",
      "Alt+Shift+ArrowLeft",
      "Alt+Shift+ArrowRight",
    ]) {
      expect(blocked("macos", spec, true), spec).toBe(false);
      expect(blocked("macos", spec, false), spec).toBe(false);
    }
  });

  it("blocks ⌘[, ⌘] and ⌘←/→ outside editable text on macOS only", () => {
    expect(blocked("macos", "Mod+[")).toBe(true);
    expect(blocked("macos", "Mod+]")).toBe(true);
    expect(blocked("macos", "Mod+ArrowLeft", false)).toBe(true);
    expect(blocked("macos", "Mod+ArrowRight", false)).toBe(true);
    // ⌘←/→ move the caret in text (008 FR-013).
    expect(blocked("macos", "Mod+ArrowLeft", true)).toBe(false);
    expect(blocked("macos", "Mod+ArrowRight", true)).toBe(false);
    for (const platform of ["windows", "linux"] as const) {
      expect(blocked(platform, "Mod+[")).toBe(false);
      expect(blocked(platform, "Mod+]")).toBe(false);
      // Ctrl+←/→ moves by word.
      expect(blocked(platform, "Mod+ArrowLeft", false)).toBe(false);
      expect(blocked(platform, "Mod+ArrowRight", false)).toBe(false);
    }
  });

  it("blocks ⌃⌘D (Look Up) on macOS only", () => {
    expect(blocked("macos", "Ctrl+Meta+d")).toBe(true);
    expect(blocked("macos", "Meta+d")).toBe(false);
    expect(blocked("linux", "Ctrl+d")).toBe(false);
    expect(blocked("windows", "Ctrl+Meta+d")).toBe(false);
  });

  it("is a function of the platform", () => {
    const keys = (platform: Platform) => keyTable(platform).map((r) => r.key);
    expect(keys("macos")).toContain("d");
    expect(keys("linux")).not.toContain("d");
    expect(keys("linux")).toContain("arrowleft");
    expect(keyTable("windows")).toEqual(keyTable("linux"));
  });

  it("leaves chords with other modifiers alone", () => {
    expect(blocked("linux", "Mod+Shift+s")).toBe(false);
    expect(blocked("linux", "Mod+Alt+r")).toBe(false);
    expect(blocked("linux", "Alt+F12")).toBe(false);
  });
});

describe("the context-menu decision", () => {
  const point = { x: 5, y: 5 };

  beforeEach(() => {
    document.body.innerHTML = `
      <main id="blank"></main>
      <div class="row" id="row"><span id="rowName">Colt Python</span></div>
      <article class="card" id="card"><p>Card</p></article>
      <button id="button">Save</button>
      <img id="image" alt="Photo" />
      <div role="dialog" id="dialog"><p id="dialogBody">Are you sure?</p></div>
      <input id="text" type="text" value="hello" />
      <input id="search" type="search" />
      <input id="email" type="email" />
      <input id="password" type="password" />
      <input id="number" type="number" />
      <input id="checkbox" type="checkbox" />
      <input id="file" type="file" />
      <textarea id="area">some words</textarea>
      <div id="editable" contenteditable="true"><b id="editableChild">typed</b></div>
      <div id="editableBare" contenteditable=""><b id="editableBareChild">typed</b></div>
      <div id="locked" contenteditable="false">locked</div>
      <div id="outer" contenteditable="true"><span id="inner" contenteditable="false">fixed</span></div>
      <select id="select"><option>One</option></select>
      <input id="readonly" type="text" value="read only" readonly />
      <input id="disabled" type="text" value="disabled" disabled />
      <p id="prose">Some selectable prose.</p>
    `;
    window.getSelection()?.removeAllRanges();
  });
  afterEach(() => {
    window.getSelection()?.removeAllRanges();
    document.body.innerHTML = "";
  });

  const at = (id: string) => document.getElementById(id);
  const allowed = (id: string) => allowsContextMenu(at(id), point);

  it("allows text fields, text areas, editable regions and selects", () => {
    for (const id of [
      "text",
      "search",
      "email",
      "password",
      "number",
      "area",
      "editable",
      "editableChild",
      "editableBare",
      "editableBareChild",
      "select",
    ]) {
      expect(allowed(id), id).toBe(true);
    }
  });

  it("prevents it on a blank area, a row, a card, a button, an image and a dialog body", () => {
    for (const id of [
      "blank",
      "row",
      "rowName",
      "card",
      "button",
      "image",
      "dialog",
      "dialogBody",
      "checkbox",
      "file",
      "locked",
      "inner",
    ]) {
      expect(allowed(id), id).toBe(false);
    }
  });

  it("prevents it on a read-only or disabled field with nothing selected", () => {
    expect(allowed("readonly")).toBe(false);
    expect(allowed("disabled")).toBe(false);
  });

  it("allows a read-only or disabled field with text selected in it", () => {
    for (const id of ["readonly", "disabled"]) {
      const field = at(id) as HTMLInputElement;
      field.setSelectionRange(0, 4);
      expect(allowed(id), id).toBe(true);
    }
  });

  it("allows a click inside a non-empty selection, and not outside it", () => {
    const prose = at("prose")!;
    const range = document.createRange();
    range.selectNodeContents(prose);
    const selection = window.getSelection()!;
    selection.removeAllRanges();
    selection.addRange(range);
    // jsdom has no layout, and no `getClientRects` on a range: without it the
    // decision falls back to whether the target is in the selection.
    expect(allowsContextMenu(prose, { x: 150, y: 10 })).toBe(true);
    expect(allowsContextMenu(at("button"), { x: 150, y: 10 })).toBe(false);
    // With geometry, the point has to be in the selection's rectangles.
    const rects = [{ left: 0, right: 100, top: 0, bottom: 20 }] as unknown as DOMRectList;
    Object.defineProperty(Range.prototype, "getClientRects", {
      configurable: true,
      value: () => rects,
    });
    try {
      expect(allowsContextMenu(prose, { x: 50, y: 10 })).toBe(true);
      expect(allowsContextMenu(prose, { x: 150, y: 10 })).toBe(false);
      expect(allowsContextMenu(at("button"), { x: 150, y: 10 })).toBe(false);
    } finally {
      delete (Range.prototype as { getClientRects?: unknown }).getClientRects;
    }
  });

  it("prevents it on a collapsed selection", () => {
    const range = document.createRange();
    range.setStart(at("prose")!.firstChild!, 3);
    range.collapse(true);
    window.getSelection()!.addRange(range);
    expect(allowed("prose")).toBe(false);
  });

  it("knows editable text from the rest", () => {
    expect(isEditableText(at("text"))).toBe(true);
    expect(isEditableText(at("editableChild"))).toBe(true);
    expect(isEditableText(at("readonly"))).toBe(false);
    expect(isEditableText(at("disabled"))).toBe(false);
    expect(isEditableText(at("select"))).toBe(false);
    expect(isEditableText(at("button"))).toBe(false);
    expect(isEditableText(null)).toBe(false);
  });
});

describe("mouse buttons", () => {
  it("treats buttons 3 and 4 (back, forward) as navigation", () => {
    expect([0, 1, 2, 3, 4, 5].map(isNavigationButton)).toEqual([
      false,
      false,
      false,
      true,
      true,
      false,
    ]);
  });
});

describe("installed", () => {
  let uninstall: (() => void) | null = null;
  const macUserAgent = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15";
  const linuxUserAgent = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15";

  beforeEach(() => {
    document.body.innerHTML = `<main id="blank"></main><input id="text" value="hi" /><button id="button">Go</button>`;
  });
  afterEach(() => {
    uninstall?.();
    uninstall = null;
    vi.unstubAllEnvs();
    document.body.innerHTML = "";
  });

  function install(userAgent: string) {
    vi.stubEnv("PROD", true);
    uninstall = installBrowserControls(window, userAgent);
    expect(uninstall).not.toBeNull();
  }

  function dispatch(target: Element | Window, event: Event): Event {
    target.dispatchEvent(event);
    return event;
  }
  const key = (init: KeyboardEventInit) =>
    new KeyboardEvent("keydown", { bubbles: true, cancelable: true, ...init });
  const mouse = (type: string, init: MouseEventInit = {}) =>
    new MouseEvent(type, { bubbles: true, cancelable: true, ...init });

  it("is not installed when DEV", () => {
    vi.stubEnv("PROD", false);
    vi.stubEnv("DEV", true);
    const spy = vi.spyOn(window, "addEventListener");
    expect(installBrowserControls(window, linuxUserAgent)).toBeNull();
    expect(spy).not.toHaveBeenCalled();
    const event = dispatch(document.body, key({ key: "F5" }));
    expect(event.defaultPrevented).toBe(false);
    spy.mockRestore();
  });

  it("cancels a blocked key before the page sees it, and leaves the others", () => {
    install(linuxUserAgent);
    const seen = vi.fn();
    document.body.addEventListener("keydown", seen);
    expect(dispatch(document.body, key({ key: "F5" })).defaultPrevented).toBe(true);
    expect(seen).not.toHaveBeenCalled();
    expect(dispatch(document.body, key({ key: "r", ctrlKey: true })).defaultPrevented).toBe(true);
    expect(dispatch(document.body, key({ key: "/" })).defaultPrevented).toBe(false);
    expect(seen).toHaveBeenCalledTimes(1);
  });

  it("lets Ctrl+L reach a later capture listener, as SessionProvider's", () => {
    install(linuxUserAgent);
    const lock = vi.fn();
    window.addEventListener("keydown", lock, true);
    try {
      dispatch(document.getElementById("text")!, key({ key: "l", ctrlKey: true }));
      expect(lock).toHaveBeenCalledTimes(1);
    } finally {
      window.removeEventListener("keydown", lock, true);
    }
  });

  it("cancels Ctrl/⌘+F for the web view but lets the app's search shortcut see it", () => {
    install(linuxUserAgent);
    const search = vi.fn();
    window.addEventListener("keydown", search);
    try {
      const event = dispatch(document.getElementById("button")!, key({ key: "f", ctrlKey: true }));
      expect(event.defaultPrevented).toBe(true);
      expect(search).toHaveBeenCalledTimes(1);
    } finally {
      window.removeEventListener("keydown", search);
    }
  });

  it("reads ⌘←/→ by where focus is", () => {
    install(macUserAgent);
    const left = { key: "ArrowLeft", metaKey: true };
    expect(dispatch(document.getElementById("text")!, key(left)).defaultPrevented).toBe(false);
    expect(dispatch(document.getElementById("button")!, key(left)).defaultPrevented).toBe(true);
  });

  it("cancels a context menu except in editable text", () => {
    install(linuxUserAgent);
    for (const id of ["blank", "button"]) {
      expect(
        dispatch(document.getElementById(id)!, mouse("contextmenu")).defaultPrevented,
        id,
      ).toBe(true);
    }
    expect(dispatch(document.getElementById("text")!, mouse("contextmenu")).defaultPrevented).toBe(
      false,
    );
  });

  it("cancels mouse buttons 3 and 4 on mousedown, mouseup and auxclick", () => {
    install(linuxUserAgent);
    for (const type of ["mousedown", "mouseup", "auxclick"]) {
      for (const button of [3, 4]) {
        expect(
          dispatch(document.body, mouse(type, { button })).defaultPrevented,
          `${type} ${button}`,
        ).toBe(true);
      }
      for (const button of [0, 1, 2]) {
        expect(
          dispatch(document.body, mouse(type, { button })).defaultPrevented,
          `${type} ${button}`,
        ).toBe(false);
      }
    }
  });

  it("prevents a force click on macOS only", () => {
    install(macUserAgent);
    const force = new Event("webkitmouseforcewillbegin", { bubbles: true, cancelable: true });
    expect(dispatch(document.body, force).defaultPrevented).toBe(true);
    uninstall?.();

    install(linuxUserAgent);
    const other = new Event("webkitmouseforcewillbegin", { bubbles: true, cancelable: true });
    expect(dispatch(document.body, other).defaultPrevented).toBe(false);
  });

  it("removes its listeners when uninstalled", () => {
    install(linuxUserAgent);
    uninstall?.();
    uninstall = null;
    expect(dispatch(document.body, key({ key: "F5" })).defaultPrevented).toBe(false);
  });
});
