import type { Notify } from "../../components";

/** Shows what a successful save still wants the user to know (FR-032b: a
 * serial-exempt firearm matching another active one) as one warning toast. */
export function notifySaveWarnings(notify: Notify, warnings: string[]) {
  if (warnings.length > 0) notify(warnings.join(" "), "warning");
}
