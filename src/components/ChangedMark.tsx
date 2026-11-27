import { Button } from "./Button";
import { placeFocus } from "./placeFocus";
import type { FieldChange } from "./fieldChanges";
import "./components.css";

export interface ChangedMarkProps {
  /** The field's label, for the revert's accessible name. */
  label: string;
  change: FieldChange;
  /** The tag's id, which the control's `aria-describedby` lists first. */
  id: string;
  /** Where focus goes after the revert: the field's input (014 FR-013). */
  focusTarget: () => HTMLElement | null;
}

/** A changed field's "Changed" tag and its revert (014 FR-006, FR-010;
 * 014 contracts/ui.md §1). The same in every form in scope (014 FR-029). */
export function ChangedMark({ label, change, id, focusTarget }: ChangedMarkProps) {
  return (
    <span className="hd-changed">
      <span id={id} className="hd-badge hd-badge--info hd-changed__tag">
        Changed
      </span>
      <Button
        size="sm"
        variant="ghost"
        icon="revert"
        className="hd-changed__revert"
        aria-label={`Revert ${label} to “${change.saved}”`}
        title={`Revert to “${change.saved}”`}
        onClick={() => {
          change.onRevert();
          // After the reverted value renders; this mark is gone by then.
          setTimeout(() => placeFocus(focusTarget()), 0);
        }}
      />
    </span>
  );
}
