import type { KeyboardEvent } from "react";

/** A radio is operated by the arrow keys and Space; Enter does nothing to it.
 * Browsers would otherwise treat Enter on a radio as implicit submission of
 * its form, so a person choosing an option could confirm a dialog by mistake
 * (the app's Enter rule, issue #44). */
export function keepEnterOut(event: KeyboardEvent<HTMLInputElement>) {
  if (event.key === "Enter") event.preventDefault();
}
