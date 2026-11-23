import { loadAppearance, saveAppearance } from "./appearance";
import type { Appearance } from "./appearance";

export type ThemePreference = Appearance["theme"];

export function loadTheme(): ThemePreference {
  return loadAppearance().theme;
}

/** Sets (or, for auto, clears) `data-theme` on <html>, which the palettes in
 * tokens.css key on. */
export function applyTheme(preference: ThemePreference) {
  const root = document.documentElement;
  if (preference === "auto") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", preference);
}

/** Applies the choice and remembers it on this computer. */
export function chooseTheme(preference: ThemePreference) {
  applyTheme(preference);
  saveAppearance({ theme: preference });
}
