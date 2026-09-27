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
