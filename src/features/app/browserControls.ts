/**
 * The browser's own controls, taken out of the page (feature 008, FR-010,
 * FR-012, FR-013; research.md §12): HoploDex is an application, so reload,
 * history navigation, print, save, view source, find and the developer tools
 * do nothing, and its context menu appears only where there is something to
 * edit or copy. The native side (`window_controls/`, research.md §13, §14)
 * trims what a menu offers and covers what a page can't cancel.
 *
 * The listeners are in the capture phase on `window` and installed before
 * React renders, so they run before any other. The app's own keys ("/",
 * Ctrl/⌘+L, Escape) and the editing keys (Ctrl/⌘+C, X, V, Z, Y, A, and the
 * arrow, Home and End keys with Shift) are not in the key table, so they
 * work as before. Only production builds install it (FR-014).
 */

export type Platform = "macos" | "windows" | "linux";

/** The platform, from `navigator.userAgent`. */
export function platformFrom(userAgent: string): Platform {
  if (/Macintosh|Mac OS X/i.test(userAgent)) return "macos";
  if (/Windows/i.test(userAgent)) return "windows";
  return "linux";
}

/** One chord the page cancels. `key` is `KeyboardEvent.key`, lower-cased.
 * `mod` is Ctrl on Windows and Linux and ⌘ on macOS (`"any"`: with or
 * without it); `shift` and `alt` are required (true), refused (false, the
 * default) or ignored ("any"). On macOS `ctrl` is the physical ⌃ key, which
 * a rule uses only for ⌃⌘D. */
export interface KeyRule {
  key: string;
  mod?: boolean | "any";
  shift?: boolean | "any";
  alt?: boolean | "any";
  ctrl?: boolean;
  /** Only when focus isn't in editable text (⌘←/→ move the caret there). */
  outsideEditableText?: boolean;
  /** The page's own handler still gets the key. Ctrl/⌘+F is the search
   * shortcut (`useSearchShortcut`): the web view's find is cancelled, the
   * app's is not. */
  reachesApp?: boolean;
}

/** The keys that reach the page and no one else, whatever their modifiers. */
const BROWSER_KEYS = [
  "browserback",
  "browserforward",
  "browserrefresh",
  "browserstop",
  "browserhome",
];

/** The chords the page cancels, for `platform` (FR-012). */
export function keyTable(platform: Platform): readonly KeyRule[] {
  const rules: KeyRule[] = [
    // Reload.
    { key: "f5", mod: "any", shift: "any" },
    { key: "r", mod: true, shift: "any" },
    // The browser's own keys on a multimedia keyboard.
    ...BROWSER_KEYS.map((key): KeyRule => ({ key, mod: "any", shift: "any", alt: "any" })),
    // Print, save, view source, find (and find next and previous), caret
    // browsing, developer tools.
    { key: "p", mod: true },
    { key: "s", mod: true },
    { key: "u", mod: true },
    { key: "f", mod: true, reachesApp: true },
    { key: "g", mod: true, shift: "any" },
    { key: "f3", mod: "any", shift: "any" },
    { key: "f7" },
    { key: "f12" },
    { key: "i", mod: true, shift: true },
    { key: "j", mod: true, shift: true },
    { key: "c", mod: true, shift: true },
  ];
  if (platform === "macos") {
    // Option+←/→ moves the caret by word on macOS (FR-013), so Alt is left
    // alone there; ⌘[ and ⌘] are Back and Forward, and ⌘←/→ are too outside
    // text; ⌃⌘D is Look Up (FR-011).
    rules.push(
      { key: "[", mod: true },
      { key: "]", mod: true },
      { key: "arrowleft", mod: true, outsideEditableText: true },
      { key: "arrowright", mod: true, outsideEditableText: true },
      { key: "d", mod: true, ctrl: true },
    );
  } else {
    rules.push({ key: "arrowleft", alt: true }, { key: "arrowright", alt: true });
  }
  return rules;
}

/** The parts of a `KeyboardEvent` the table reads. */
export interface KeyChord {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

function allows(rule: boolean | "any" | undefined, pressed: boolean): boolean {
  return rule === "any" || (rule ?? false) === pressed;
}

/** The rule that cancels `event`'s chord on `platform`, if one does.
 * `inEditableText` says whether focus is in text the user can edit. */
export function ruleFor(
  platform: Platform,
  event: KeyChord,
  inEditableText: boolean,
): KeyRule | undefined {
  const mac = platform === "macos";
  const key = event.key.toLowerCase();
  // The meta key is the Windows key elsewhere, and no chord of the table.
  const mod = mac ? event.metaKey : event.ctrlKey;
  // On macOS ⌃ is a modifier of its own; elsewhere it is `mod`.
  const control = mac ? event.ctrlKey : false;
  return keyTable(platform).find(
    (rule) =>
      rule.key === key &&
      allows(rule.mod, mod) &&
      allows(rule.shift, event.shiftKey) &&
      allows(rule.alt, event.altKey) &&
      (rule.ctrl ?? false) === control &&
      !(rule.outsideEditableText && inEditableText) &&
      (mac || !event.metaKey),
  );
}

/** Whether `event`'s chord is cancelled on `platform`. */
export function blocksKey(platform: Platform, event: KeyChord, inEditableText: boolean): boolean {
  return ruleFor(platform, event, inEditableText) !== undefined;
}

const TEXT_INPUT_TYPES = new Set([
  "",
  "text",
  "search",
  "url",
  "tel",
  "email",
  "password",
  "number",
]);

function elementOf(target: EventTarget | null): Element | null {
  if (target instanceof Element) return target;
  return target instanceof Node ? target.parentElement : null;
}

function isTextInput(element: Element): element is HTMLInputElement | HTMLTextAreaElement {
  if (element instanceof HTMLTextAreaElement) return true;
  return element instanceof HTMLInputElement && TEXT_INPUT_TYPES.has(element.type);
}

/** Whether `target` is text the user can edit: a text-like `input` or a
 * `textarea` that is neither read-only nor disabled, or inside a
 * `[contenteditable]` other than `"false"`. */
export function isEditableText(target: EventTarget | null): boolean {
  const element = elementOf(target);
  if (!element) return false;
  const field = element.closest("input, textarea");
  if (field) return isTextInput(field) && !field.readOnly && !field.disabled;
  const editable = element.closest("[contenteditable]");
  return editable !== null && editable.getAttribute("contenteditable") !== "false";
}

/** Whether a field has text selected in it (the document's own selection
 * doesn't report that for an `input` or a `textarea`). */
function fieldHasSelection(element: Element): boolean {
  const field = element.closest("input, textarea");
  if (!field || !isTextInput(field)) return false;
  try {
    return field.selectionStart !== null && field.selectionStart !== field.selectionEnd;
  } catch {
    return false;
  }
}

/** Whether a click at `point` is inside the document's non-empty selection. */
function insideSelection(target: Element, point: { x: number; y: number }, doc: Document): boolean {
  const selection = doc.getSelection();
  if (!selection || selection.isCollapsed || selection.rangeCount === 0) return false;
  for (let i = 0; i < selection.rangeCount; i++) {
    const range = selection.getRangeAt(i);
    if (typeof range.getClientRects !== "function") {
      // No geometry to ask (the selection is all there is): the target's
      // being in it is the nearest answer.
      if (range.intersectsNode(target)) return true;
      continue;
    }
    for (const rect of Array.from(range.getClientRects())) {
      if (
        point.x >= rect.left &&
        point.x <= rect.right &&
        point.y >= rect.top &&
        point.y <= rect.bottom
      ) {
        return true;
      }
    }
  }
  return false;
}

/** Whether a context menu may open for a right-click on `target` at `point`
 * (FR-010): on editable text, a `select`, and on a non-empty selection, which
 * includes a read-only or disabled field with text selected in it. Nowhere
 * else: a blank area, a row, a card, a button, an image, a dialog body. */
export function allowsContextMenu(
  target: EventTarget | null,
  point: { x: number; y: number },
  doc: Document = document,
): boolean {
  const element = elementOf(target);
  if (!element) return false;
  if (isEditableText(element) || element.closest("select")) return true;
  return fieldHasSelection(element) || insideSelection(element, point, doc);
}

/** The mouse's back and forward buttons (`MouseEvent.button` 3 and 4). */
export function isNavigationButton(button: number): boolean {
  return button === 3 || button === 4;
}

/** Installs the listeners on `target` and returns a function that removes
 * them, or `null` when nothing is installed: only production builds have
 * them (FR-014), so development keeps its reload and developer tools. */
export function installBrowserControls(
  target: Window = window,
  userAgent: string = navigator.userAgent,
): (() => void) | null {
  if (!import.meta.env.PROD) return null;
  const platform = platformFrom(userAgent);
  const doc = target.document;

  const onContextMenu = (event: MouseEvent) => {
    if (!allowsContextMenu(event.target, { x: event.clientX, y: event.clientY }, doc)) {
      event.preventDefault();
    }
  };
  const onKeyDown = (event: KeyboardEvent) => {
    const rule = ruleFor(platform, event, isEditableText(event.target));
    if (!rule) return;
    event.preventDefault();
    if (!rule.reachesApp) event.stopPropagation();
  };
  const onMouseButton = (event: MouseEvent) => {
    if (isNavigationButton(event.button)) event.preventDefault();
  };
  // A force click's default action is Look Up (FR-011).
  const onForceClick = (event: Event) => event.preventDefault();

  const listeners: [string, EventListener][] = [
    ["contextmenu", onContextMenu as EventListener],
    ["keydown", onKeyDown as EventListener],
    ["mousedown", onMouseButton as EventListener],
    ["mouseup", onMouseButton as EventListener],
    ["auxclick", onMouseButton as EventListener],
  ];
  if (platform === "macos") {
    listeners.push(["webkitmouseforcewillbegin", onForceClick]);
  }
  for (const [type, listener] of listeners) target.addEventListener(type, listener, true);
  return () => {
    for (const [type, listener] of listeners) target.removeEventListener(type, listener, true);
  };
}
