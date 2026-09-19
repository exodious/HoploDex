import { useState } from "react";
import { Button, ConfirmDialog, Dialog, useToast } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import { useCollection } from "../app/collectionStore";
import { FirearmLinkList } from "./PolicyCard";
import * as insuranceService from "./insuranceService";
import { InsurancePolicyForm } from "./InsurancePolicyForm";
import type { InsurancePolicy, InsurancePolicyInput } from "./types";

/** What the add/edit and delete dialogs need from a page: which policy each
 * is open for, and the callbacks that open them. */
export function usePolicyEditors(onDeleted?: () => void) {
  const { firearms, refresh } = useCollection();
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

  async function handleDelete(policy: InsurancePolicy) {
    try {
      await insuranceService.deleteInsurancePolicy(policy.id, true);
      notify(`Deleted ${policy.name}.`);
      onDeleted?.();
      await refresh();
    } catch (e) {
      notify(
        e instanceof CommandFailure ? e.message : `${policy.name} couldn't be deleted.`,
        "error",
      );
    }
  }

  const assignedTo = (policy: InsurancePolicy) =>
    firearms.filter((f) => f.insurancePolicyId === policy.id);

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

      {deleting && assignedTo(deleting).length > 0 ? (
        <Dialog
          open
          onOpenChange={(open) => !open && setDeleting(null)}
          title={`${deleting.name} still covers firearms`}
          description="Assign these firearms to another policy, or mark them not insured, before deleting it. Deleting it first would silently leave them uninsured."
          footer={
            <Button variant="primary" onClick={() => setDeleting(null)}>
              OK
            </Button>
          }
        >
          <FirearmLinkList firearms={assignedTo(deleting)} onNavigate={() => setDeleting(null)} />
        </Dialog>
      ) : (
        <ConfirmDialog
          open={deleting != null}
          onOpenChange={(open) => !open && setDeleting(null)}
          title={`Delete ${deleting?.name ?? "this policy"}?`}
          description="The policy and its details will be permanently removed. No firearms are assigned to it."
          confirmLabel="Delete policy"
          onConfirm={() => (deleting ? handleDelete(deleting) : undefined)}
        />
      )}
    </>
  );

  return { add: () => setEditing("new"), edit: setEditing, remove: setDeleting, dialogs };
}
