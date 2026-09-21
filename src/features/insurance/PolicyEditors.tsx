import { useState } from "react";
import { Dialog, useToast } from "../../components";
import { useCollection } from "../app/collectionStore";
import * as insuranceService from "./insuranceService";
import { InsurancePolicyForm } from "./InsurancePolicyForm";
import { PolicyDeleteDialog } from "./PolicyDeleteDialog";
import type { InsurancePolicy, InsurancePolicyInput } from "./types";

/** What the add/edit and delete dialogs need from a page: which policy each
 * is open for, and the callbacks that open them. */
export function usePolicyEditors(onDeleted?: () => void) {
  const { refresh } = useCollection();
  const notify = useToast();
  const [editing, setEditing] = useState<InsurancePolicy | "new" | null>(null);
  const [deleting, setDeleting] = useState<InsurancePolicy | null>(null);

  async function handleSave(input: InsurancePolicyInput) {
    if (editing === "new") {
      const created = await insuranceService.createInsurancePolicy(input);
      notify(`Added ${created.name}.`);
    } else if (editing) {
      const updated = await insuranceService.updateInsurancePolicy(editing.id, input);
      notify(`Saved changes to ${updated.name}.`);
    }
    setEditing(null);
    await refresh();
  }

  async function handleDeleted(policy: InsurancePolicy) {
    setDeleting(null);
    notify(`Deleted ${policy.name}.`);
    onDeleted?.();
    await refresh();
  }

  const dialogs = (
    <>
      <Dialog
        open={editing != null}
        onOpenChange={(open) => !open && setEditing(null)}
        title={editing === "new" || editing == null ? "Add policy" : `Edit ${editing.name}`}
        size="lg"
        bare
      >
        <InsurancePolicyForm
          initialValues={editing === "new" || editing == null ? undefined : editing}
          onSubmit={handleSave}
          onCancel={() => setEditing(null)}
        />
      </Dialog>

      {deleting && (
        <PolicyDeleteDialog
          open
          onOpenChange={(open) => !open && setDeleting(null)}
          policy={deleting}
          onDeleted={() => handleDeleted(deleting)}
        />
      )}
    </>
  );

  return { add: () => setEditing("new"), edit: setEditing, remove: setDeleting, dialogs };
}
