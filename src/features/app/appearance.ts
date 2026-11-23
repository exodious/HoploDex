import { invoke } from "../../services/tauriClient";

/** The page's color mode and the collection's layout, kept in `machine.json`:
 * the main web view keeps no storage of its own (code scanning alert
 * plaintext-lifetime/cwe-922). Rust hands it over before the first script runs
 * (`window.__HOPLODEX_APPEARANCE__`, main.rs), so a chosen theme never flashes. */
export interface Appearance {
  theme: "auto" | "light" | "dark";
  browseView: "list" | "tile";
}

const current: Appearance = {
  theme: "auto",
  browseView: "list",
  ...(window as { __HOPLODEX_APPEARANCE__?: Partial<Appearance> }).__HOPLODEX_APPEARANCE__,
};

export function loadAppearance(): Readonly<Appearance> {
  return current;
}

/** Saves one setting on this computer. A failed save only means it won't
 * outlive this run. */
export function saveAppearance(change: Partial<Appearance>): void {
  const next = { ...current, ...change };
  if (next.theme === current.theme && next.browseView === current.browseView) return;
  Object.assign(current, next);
  invoke("set_appearance", { appearance: next }).catch(() => {});
}
