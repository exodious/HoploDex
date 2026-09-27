import { useState } from "react";
import { Button, ConfirmDialog, Dialog } from "../../components";
import { formatDateTime } from "../../lib/dates";
import { FORM_VERSION as COVERAGE_FORM } from "../insurance/CoverageDialog";
import { FORM_VERSION as POLICY_FORM } from "../insurance/InsurancePolicyForm";
import { FORM_VERSION as DISPOSE_FORM } from "../firearms/DisposeDialog";
import { FORM_VERSION as FIREARM_FORM } from "../firearms/FirearmForm";
import { FORM_VERSION as RESTORE_FORM } from "../firearms/RestoreDialog";
import type { PendingSummary } from "../databases/types";

/** The version of each form's drafts this frontend can open (research.md
 * §16). */
const FORM_VERSIONS: Record<string, number> = {
  "firearm/add": FIREARM_FORM,
  "firearm/edit": FIREARM_FORM,
  "firearm/dispose": DISPOSE_FORM,
  "firearm/restore": RESTORE_FORM,
  "firearm/coverage": COVERAGE_FORM,
  "policy/add": POLICY_FORM,
  "policy/edit": POLICY_FORM,
};

/** Whether the kept changes can be opened again: their record still exists,
 * and their form is one this version of HoploDex knows. */
function canResume(pending: PendingSummary): boolean {
  return (
    pending.resumable && FORM_VERSIONS[`${pending.kind}/${pending.mode}`] === pending.formVersion
  );
}

export interface PendingChangesDialogProps {
  pending: PendingSummary;
  /** Resolves once the form has its changes back. */
  onResume: () => Promise<void>;
  onDiscard: () => Promise<void>;
}

/** Unsaved changes kept at a lock, offered before the collection can be
 * used (FR-039, contracts/ui-databases.md §13). It can't be dismissed: the
 * changes are resumed or discarded, and discarding asks first. */
export function PendingChangesDialog({ pending, onResume, onDiscard }: PendingChangesDialogProps) {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const resumable = canResume(pending);

  async function resume() {
    setBusy(true);
    setProblem(null);
    try {
      await onResume();
    } catch {
      setProblem("The changes couldn't be opened. Try again, or discard them.");
      setBusy(false);
    }
  }

  return (
    <>
      <Dialog
        open={!confirming}
        onOpenChange={() => {}}
        dismissible={false}
        title={`Unsaved changes to ${pending.label}`}
        footer={
          <>
            <Button variant="danger" disabled={busy} onClick={() => setConfirming(true)}>
              Discard changes
            </Button>
            {resumable && (
              <Button variant="primary" autoFocus pending={busy} onClick={() => void resume()}>
                Resume editing
              </Button>
            )}
          </>
        }
      >
        <p role="status" aria-live="polite">
          The database locked on {formatDateTime(pending.savedAt)} while you were editing{" "}
          {pending.label}. Your changes were kept.
          {!pending.resumable && (
            <> {pending.label} no longer exists, so these changes can only be discarded.</>
          )}
          {pending.resumable && !resumable && (
            <> They were kept by another version of HoploDex, so they can only be discarded.</>
          )}
        </p>
        {problem && (
          <p className="hd-field__error" role="alert">
            {problem}
          </p>
        )}
      </Dialog>
      <ConfirmDialog
        open={confirming}
        onOpenChange={setConfirming}
        title={`Discard the changes to ${pending.label}?`}
        description="They are removed from the database. This can't be undone."
        confirmLabel="Discard changes"
        onConfirm={onDiscard}
      />
    </>
  );
}
