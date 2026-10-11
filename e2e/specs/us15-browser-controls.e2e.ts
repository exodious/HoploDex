import {
  HAS_REAL_INPUT,
  realClick,
  realKey,
  realMouseButton,
  skipWithoutRealInput,
} from "../support/realInput";
import {
  $,
  addFirearm,
  browser,
  clickButton,
  createDatabase,
  expect,
  goTo,
  openFirearm,
  pressEscape,
  scratchDocuments,
  settle,
  waitForChooser,
} from "../support/ui";

/**
 * End-to-end coverage of specs/008-hardening-batch's User Story 3 (browser
 * controls, #40; spec.md FR-010 to FR-013, contracts/ui.md) against the real
 * built app. The E2E build installs the controls like a release build does
 * (research.md §15), so this runs them: the page's own layer, here
 * (research.md §12); what each OS's menus offer is the window controls
 * check's (research.md §13, §18).
 *
 * The page side runs on every OS. The real keyboard and the mouse's back and
 * forward buttons, which reach the web view before the page does, run on
 * Linux only (XTest, support/realInput.ts).
 */

const folder = `${scratchDocuments()}/BrowserControls`;
const firearm = {
  make: "Controlled",
  model: "One",
  caliber: "9mm",
  type: "Handgun" as const,
  serial: "BC-1",
};
const MOD = process.platform === "darwin" ? "meta" : "ctrl";

/** Dispatches a `contextmenu` at the middle of the first element matching
 * `selector` (or, with `inSelection`, of the document's selected text, which
 * is narrower than the element holding it) and says whether the page
 * cancelled it. */
function contextMenuCancelled(selector: string, inSelection = false): Promise<boolean> {
  return browser.execute(
    (css: string, selected: boolean) => {
      const element = document.querySelector(css);
      if (!element) throw new Error(`nothing matches ${css}`);
      const r = selected
        ? window.getSelection()!.getRangeAt(0).getClientRects()[0]
        : element.getBoundingClientRect();
      const event = new MouseEvent("contextmenu", {
        bubbles: true,
        cancelable: true,
        button: 2,
        clientX: r.left + r.width / 2,
        clientY: r.top + r.height / 2,
      });
      element.dispatchEvent(event);
      return event.defaultPrevented;
    },
    selector,
    inSelection,
  );
}

/** The id of the field behind a visible label, in the open dialog. */
function fieldSelector(label: string): Promise<string> {
  return browser.execute((wanted: string) => {
    const dialog = document.querySelector('[role="dialog"]') ?? document;
    const labelEl = [...dialog.querySelectorAll("label")].find(
      (l) => l.textContent?.trim() === wanted && l.htmlFor,
    );
    if (!labelEl) throw new Error(`no field labelled ${wanted}`);
    return `[id="${labelEl.htmlFor}"]`;
  }, label);
}

/** The value of the field at `selector`. */
function valueOf(selector: string): Promise<string> {
  return browser.execute(
    (css: string) => (document.querySelector(css) as HTMLInputElement | HTMLTextAreaElement).value,
    selector,
  );
}

/** Dispatches a key at the focused element (or the page) and says whether the
 * page cancelled it: the keys the app and the field's editing need must not
 * be. */
function keyCancelled(init: KeyboardEventInit): Promise<boolean> {
  return browser.execute((data: KeyboardEventInit) => {
    const target = document.activeElement ?? document.body;
    const event = new KeyboardEvent("keydown", { bubbles: true, cancelable: true, ...data });
    target.dispatchEvent(event);
    return event.defaultPrevented;
  }, init);
}

/** `expect(actual, message)` isn't available, so a failure says what it was. */
function is<T>(actual: T, expected: T, message: string) {
  if (actual !== expected) {
    throw new Error(`${message}: expected ${String(expected)}, got ${String(actual)}`);
  }
}
function greater(actual: number, floor: number, message: string) {
  if (!(actual > floor)) throw new Error(`${message}: got ${actual}`);
}

describe("User Story 3 (008) - The Window Behaves Like an Application, Not a Browser", () => {
  before(async () => {
    await createDatabase({ folder, name: "Controls" });
    await addFirearm(firearm);
  });

  it("cancels the context menu on a blank area, a row, a card, a button and an image", async () => {
    // The record page the firearm was added on: a title-block card.
    expect(await contextMenuCancelled(".hd-titleblock__cell")).toBe(true);
    await goTo("Collection");
    expect(await contextMenuCancelled("body")).toBe(true);
    expect(await contextMenuCancelled(".hd-row")).toBe(true);
    expect(await contextMenuCancelled('nav[aria-label="Sections"] button')).toBe(true);
    // An image, in whatever the page shows: the firearm's drawing, then a
    // real <img>.
    expect(await contextMenuCancelled(".hd-row__thumb")).toBe(true);
    await browser.execute(() => {
      const img = document.createElement("img");
      img.id = "controls-probe-image";
      img.alt = "";
      img.width = 40;
      img.height = 40;
      img.src = "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7";
      document.body.append(img);
    });
    expect(await contextMenuCancelled("#controls-probe-image")).toBe(true);
    await browser.execute(() => document.getElementById("controls-probe-image")?.remove());
  });

  it("cancels it on a dialog's body, and not on a text field, a text area or a selection", async () => {
    await clickButton("Add firearm");
    await $('[role="dialog"]').waitForExist();

    expect(await contextMenuCancelled('[role="dialog"] .hd-dialog__title')).toBe(true);
    expect(await contextMenuCancelled('[role="dialog"]')).toBe(true);

    expect(await contextMenuCancelled(await fieldSelector("Make"))).toBe(false);
    expect(await contextMenuCancelled(await fieldSelector("Notes"))).toBe(false);

    // A selection in the dialog's text: a click inside it gets a menu (with
    // Copy), a click elsewhere in the dialog does not.
    await browser.execute(() => {
      const title = document.querySelector('[role="dialog"] .hd-dialog__title') as HTMLElement;
      const range = document.createRange();
      range.selectNodeContents(title);
      const selection = window.getSelection()!;
      selection.removeAllRanges();
      selection.addRange(range);
    });
    expect(await contextMenuCancelled('[role="dialog"] .hd-dialog__title', true)).toBe(false);
    await browser.execute(() => window.getSelection()?.removeAllRanges());
    expect(await contextMenuCancelled('[role="dialog"] .hd-dialog__title')).toBe(true);
  });

  it("closes a dialog with Escape", async () => {
    await pressEscape();
    await browser.waitUntil(async () => !(await $('[role="dialog"]').isExisting()), {
      timeout: 5000,
      timeoutMsg: "Escape did not close the dialog",
    });
  });

  it('focuses the search box with "/" and leaves the editing keys alone', async () => {
    await browser.execute(() => (document.activeElement as HTMLElement | null)?.blur());
    expect(await keyCancelled({ key: "/" })).toBe(true); // the app's own handler cancels it
    const focused = await browser.execute(
      () => document.activeElement?.matches('input[type="search"]') ?? false,
    );
    expect(focused).toBe(true);

    // In a field, the keys that edit are not cancelled by the controls, and
    // "/" types.
    const mod = { ctrlKey: MOD === "ctrl", metaKey: MOD === "meta" };
    for (const [key, extra] of [
      ["c", {}],
      ["x", {}],
      ["v", {}],
      ["z", {}],
      ["y", {}],
      ["z", { shiftKey: true }],
      ["a", {}],
    ] as const) {
      is(await keyCancelled({ key, ...mod, ...extra }), false, `${MOD}+${key}`);
    }
    for (const key of ["ArrowLeft", "ArrowRight", "Home", "End"]) {
      is(await keyCancelled({ key, shiftKey: true }), false, `Shift+${key}`);
    }

    // The reload and navigation keys are cancelled, here too.
    for (const init of [
      { key: "F5" },
      { key: "r", ...mod },
      { key: "R", shiftKey: true, ...mod },
      { key: "F12" },
    ]) {
      is(await keyCancelled(init), true, JSON.stringify(init));
    }
    await browser.execute(() => (document.activeElement as HTMLElement | null)?.blur());
  });

  it("keeps the window and the typed input through every reload and navigation key, and works the editing keys with real input", async function () {
    skipWithoutRealInput(this);
    await openFirearm("Controlled One");
    await clickButton("Edit");
    await $('[role="dialog"]').waitForExist();
    await settle();
    const notes = await fieldSelector("Notes");
    await realClick(notes);
    for (const key of ["h", "e", "l", "l", "o"]) await realKey(key);
    expect(await valueOf(notes)).toBe("hello");

    // A marker a reload would erase, and a record of the mouse buttons the
    // page saw (they must arrive, or the check below proves nothing).
    await browser.execute(() => {
      sessionStorage.setItem("hd-controls-marker", "still here");
      const seen: Record<number, { count: number; prevented: boolean }> = {};
      (window as unknown as { __hdMouse: typeof seen }).__hdMouse = seen;
      for (const type of ["mousedown", "mouseup", "auxclick"]) {
        window.addEventListener(type, (e) => {
          const event = e as MouseEvent;
          if (event.button < 3) return;
          const entry = (seen[event.button] ??= { count: 0, prevented: true });
          entry.count += 1;
          entry.prevented &&= event.defaultPrevented;
        });
      }
    });

    const keys = [
      "F5",
      "Control_L+r",
      "Control_L+Shift_L+r",
      "Control_L+F5",
      "Shift_L+F5",
      "Alt_L+Left",
      "Alt_L+Right",
    ];
    for (const keyChord of keys) {
      await realKey(keyChord);
      await settle();
      is(
        await browser.execute(() => sessionStorage.getItem("hd-controls-marker")),
        "still here",
        `${keyChord} reloaded the window`,
      );
      is(await valueOf(notes), "hello", `${keyChord} changed the input`);
      is(await $('[role="dialog"]').isExisting(), true, `${keyChord} closed the form`);
    }

    // The mouse's back (X button 8) and forward (9).
    for (const button of [8, 9]) {
      await realMouseButton(button);
      await settle();
      is(
        await browser.execute(() => sessionStorage.getItem("hd-controls-marker")),
        "still here",
        `button ${button} navigated`,
      );
      is(await valueOf(notes), "hello", `button ${button} changed the input`);
    }
    const seen = await browser.execute(
      () =>
        (window as unknown as { __hdMouse: Record<number, { count: number; prevented: boolean }> })
          .__hdMouse,
    );
    for (const dom of [3, 4]) {
      greater(seen[dom]?.count ?? 0, 0, `the page saw no mouse button ${dom}`);
      is(seen[dom].prevented, true, `mouse button ${dom} was not cancelled`);
    }

    // The editing keys, for real: select all, cut, paste, copy, undo.
    await realKey("Control_L+a");
    await realKey("Control_L+x");
    expect(await valueOf(notes)).toBe("");
    await realKey("Control_L+v");
    expect(await valueOf(notes)).toBe("hello");
    await realKey("Control_L+a");
    await realKey("Control_L+c");
    await realKey("End");
    await realKey("Control_L+v");
    expect(await valueOf(notes)).toBe("hellohello");
    // Undo: the web view on Linux (WebKitGTK under Xvfb) does not undo in any
    // text field, not even a bare textarea, so what this checks is that the
    // controls leave the key alone (the page doesn't cancel it).
    await browser.execute(() => {
      const w = window as unknown as { __hdUndoCancelled?: boolean };
      w.__hdUndoCancelled = undefined;
      window.addEventListener("keydown", (e) => {
        if (e.key.toLowerCase() === "z" && e.ctrlKey) w.__hdUndoCancelled = e.defaultPrevented;
      });
    });
    await realKey("Control_L+z");
    is(
      await browser.execute(
        () => (window as unknown as { __hdUndoCancelled?: boolean }).__hdUndoCancelled,
      ),
      false,
      "Ctrl+Z was cancelled or never reached the page",
    );
  });

  it("locks with Ctrl/⌘+L from the open form", async () => {
    if (HAS_REAL_INPUT) {
      // The form from the last test is open with unsaved input.
      expect(await $('[role="dialog"]').isExisting()).toBe(true);
    } else {
      await openFirearm("Controlled One");
      await clickButton("Edit");
      await $('[role="dialog"]').waitForExist();
    }
    expect(await keyCancelled({ key: "l", ctrlKey: MOD === "ctrl", metaKey: MOD === "meta" })).toBe(
      true,
    ); // SessionProvider's own listener took it
    await waitForChooser();
    expect(await $('nav[aria-label="Sections"]').isExisting()).toBe(false);
  });
});
