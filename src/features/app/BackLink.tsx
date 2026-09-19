import { Icon } from "../../components";
import type { BackTarget } from "./navigation";

/** "← Where you came from". The label names the page or record returned
 * to. Pass `escapes` on pages where the Escape key does the same. */
export function BackLink({ target, escapes }: { target: BackTarget; escapes?: boolean }) {
  return (
    <button
      type="button"
      className="hd-backlink"
      onClick={target.go}
      aria-keyshortcuts={escapes ? "Escape" : undefined}
    >
      <Icon name="back" size={16} />
      {target.label}
    </button>
  );
}
