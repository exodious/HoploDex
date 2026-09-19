export type ThemePreference = "auto" | "light" | "dark";

const THEME_KEY = "hoplodex.theme";

export function loadTheme(): ThemePreference {
  try {
    const stored = localStorage.getItem(THEME_KEY);
    if (stored === "light" || stored === "dark") return stored;
  } catch {
    // storage unavailable — follow the OS
  }
  return "auto";
}

/** Sets (or, for auto, clears) `data-theme` on <html>, which the palettes in
 * tokens.css key on, and remembers the choice. */
export function applyTheme(preference: ThemePreference) {
  const root = document.documentElement;
  if (preference === "auto") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", preference);
  try {
    if (preference === "auto") localStorage.removeItem(THEME_KEY);
    else localStorage.setItem(THEME_KEY, preference);
  } catch {
    // storage unavailable — the choice just won't persist
  }
}
