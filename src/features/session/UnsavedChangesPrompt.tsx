import { ConfirmDialog } from "../../components";

export interface UnsavedChangesPromptProps {
  /** The form's label, or `null` while nothing is being asked. */
  label: string | null;
  /** Runs the form's own save; resolves once it has either saved and the
   * close gone ahead, or failed and left the form showing why. */
  onSave: () => Promise<void>;
  onDiscard: () => void;
  onCancel: () => void;
}

/** Save, discard or cancel when a close, switch or quit finds a form with
 * unsaved input (FR-010, specs/003 contracts/ui-databases.md §6). A lock
 * never asks (FR-033). */
export function UnsavedChangesPrompt({
  label,
  onSave,
  onDiscard,
  onCancel,
}: UnsavedChangesPromptProps) {
  return (
    <ConfirmDialog
      open={label !== null}
      onOpenChange={(open) => !open && onCancel()}
      title={`Save changes to ${label ?? ""}?`}
      description="Your changes haven't been saved yet."
      confirmLabel="Save changes"
      destructive={false}
      onConfirm={onSave}
      alternativeLabel="Discard changes"
      onAlternative={onDiscard}
    />
  );
}
