import { Icon } from "../../components";
import type { BackTarget } from "./navigation";

/** "← Where you came from". The label names the page or record returned
 * to (FR-040). Escape does the same wherever a back link shows (the app shell
 * handles it), so the link carries the key as a keycap. */
export function BackLink({ target }: { target: BackTarget }) {
  return (
    <button type="button" className="hd-backlink" onClick={target.go} aria-keyshortcuts="Escape">
      <Icon name="back" size={16} />
      <span className="hd-backlink__label">{target.label}</span>
      <kbd className="hd-kbd hd-backlink__kbd" aria-hidden>
        Esc
      </kbd>
    </button>
  );
}
