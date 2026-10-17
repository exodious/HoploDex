/** Whether the person's last input was a key rather than a pointer. Before
 * any input (a screen shown at launch) it counts as a pointer. */
let keyboardLast = false;

if (typeof window !== "undefined") {
  window.addEventListener("keydown", () => (keyboardLast = true), true);
  window.addEventListener("pointerdown", () => (keyboardLast = false), true);
}

/**
 * Moves focus for the person, where the app chooses: a dialog's first
 * control, a screen's main button. The focus ring shows only when they are
 * using the keyboard: WebKitGTK otherwise rings a control focused this way
 * even after a mouse click, so it looks already chosen.
 */
export function placeFocus(element: HTMLElement | null | undefined) {
  element?.focus({ focusVisible: keyboardLast } as FocusOptions);
}

/**
 * What a dialog opened from the focused element gives focus back to when it
 * closes. A dialog chosen from a menu opens while a menu item has focus; the
 * item is gone by the time the dialog closes, so the opener is the menu's
 * button (Radix labels a menu by it) (#114).
 */
export function currentOpener(): HTMLElement | null {
  const active = document.activeElement as HTMLElement | null;
  const trigger = active?.closest('[role="menu"]')?.getAttribute("aria-labelledby");
  return (trigger && document.getElementById(trigger)) || active;
}
