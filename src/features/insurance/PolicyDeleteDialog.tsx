import { useEffect, useState } from "react";
import { Checkbox, ChoiceCards, ConfirmDialog, Icon, Select } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import { RecordName } from "../mounts/RecordName";
import * as insuranceService from "./insuranceService";
import type { InsurancePolicy, PolicyDeletionImpact, ScheduledFirearmsAction } from "./types";
import "../firearms/forms.css";

export interface PolicyDeleteDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  policy: InsurancePolicy;
  /** Called once the policy is gone, to refresh and report; shouldn't throw. */
  onDeleted: () => Promise<void>;
}

type Choice = "move" | "unschedule";

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** Deleting a policy never silently un-insures a record (FR-034, 006 FR-009):
 * with firearms or accessories scheduled under it, the user first moves them to another policy
 * (each keeps its scheduled amount) or leaves them unscheduled, with a
 * stronger confirmation when the policy is still current. Built on the
 * shared `ConfirmDialog` (constitution III). */
export function PolicyDeleteDialog({
  open,
  onOpenChange,
  policy,
  onDeleted,
}: PolicyDeleteDialogProps) {
  // Mounted only while open, so every opening loads a fresh impact.
  return open ? (
    <PolicyDeleteBody onOpenChange={onOpenChange} policy={policy} onDeleted={onDeleted} />
  ) : null;
}

function PolicyDeleteBody({
  onOpenChange,
  policy,
  onDeleted,
}: Omit<PolicyDeleteDialogProps, "open">) {
  const [impact, setImpact] = useState<PolicyDeletionImpact | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [choice, setChoice] = useState<Choice | "">("");
  const [targetId, setTargetId] = useState("");
  const [understood, setUnderstood] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    insuranceService
      .getPolicyDeletionImpact(policy.id)
      .then((loaded) => !cancelled && setImpact(loaded))
      .catch(
        (e) =>
          !cancelled &&
          setLoadError(
            e instanceof CommandFailure ? e.message : "This policy couldn't be checked.",
          ),
      );
    return () => {
      cancelled = true;
    };
  }, [policy.id]);

  const scheduled = impact?.scheduledRecordCount ?? 0;
  const target = impact?.otherPolicies.find((p) => String(p.id) === targetId);
  // A policy that hasn't expired is still insuring these firearms.
  const strongConfirmation = choice === "unschedule" && impact != null && !impact.isExpired;
  const outcome =
    impact?.unscheduleOutcome === "blanket"
      ? "covered by the blanket policy in force"
      : "uninsured";

  const ready =
    impact != null &&
    (scheduled === 0 ||
      (choice === "move" && targetId !== "") ||
      (choice === "unschedule" && (!strongConfirmation || understood)));

  function resolution(): ScheduledFirearmsAction | undefined {
    if (scheduled === 0) return undefined;
    if (choice === "move") return { action: "move", targetPolicyId: Number(targetId) };
    return strongConfirmation
      ? { action: "unschedule", confirmUnschedule: true }
      : { action: "unschedule" };
  }

  async function confirm() {
    setFailure(null);
    try {
      await insuranceService.deleteInsurancePolicy(policy.id, true, resolution());
    } catch (e) {
      setFailure(e instanceof CommandFailure ? e.message : `${policy.name} couldn't be deleted.`);
      throw e; // keeps the dialog open
    }
    await onDeleted();
  }

  return (
    <ConfirmDialog
      open
      onOpenChange={onOpenChange}
      title={`Delete ${policy.name}?`}
      description={
        scheduled > 0
          ? `${plural(scheduled, "record is", "records are")} scheduled under it. Choose what happens to them; none is left uninsured by accident.`
          : "The policy and its details will be permanently removed."
      }
      confirmLabel="Delete policy"
      confirmDisabled={!ready}
      onConfirm={confirm}
    >
      <div className="hd-form-section">
        {(loadError || failure) && (
          <p className="hd-banner hd-banner--error" role="alert">
            {loadError ?? failure}
          </p>
        )}
        {!impact && !loadError && <p className="hd-form-note">Checking what this policy covers…</p>}

        {impact?.isBlanketInForce && (
          <p className="hd-banner" role="note">
            <Icon name="alert" />
            <span className="hd-banner__text">
              This is the blanket policy in force.{" "}
              {impact.blanketRecordCount > 0
                ? `${plural(impact.blanketRecordCount, "record", "records")} that ${impact.blanketRecordCount === 1 ? "isn't" : "aren't"} scheduled will lose its blanket coverage`
                : "No unscheduled firearms or accessories rely on it, but any you add later will lose its blanket coverage"}
              , and will be uninsured unless another blanket policy is in force.
            </span>
          </p>
        )}

        {impact && scheduled > 0 && (
          <>
            <ul className="hd-past-dispositions">
              {impact.scheduledRecords.map((label) => (
                <li key={`${label.record.kind}:${label.record.id}`}>
                  <RecordName label={label} />
                </li>
              ))}
            </ul>
            {impact.isExpired && (
              <p className="hd-form-note">
                This policy has expired, so these records are already treated as uninsured.
              </p>
            )}
            <ChoiceCards<Choice>
              label="What should happen to them?"
              required
              value={choice}
              onChange={setChoice}
              minCardWidth={200}
              options={[
                ...(impact.otherPolicies.length > 0
                  ? [
                      {
                        value: "move" as const,
                        label: "Move to another policy",
                        description: "Each keeps its scheduled amount.",
                      },
                    ]
                  : []),
                {
                  value: "unschedule" as const,
                  label: "Leave unscheduled",
                  description: `They will be ${outcome}.`,
                },
              ]}
            />
            {choice === "move" && (
              <>
                <Select
                  label="Move to"
                  required
                  value={targetId || undefined}
                  onValueChange={setTargetId}
                  options={impact.otherPolicies.map((p) => ({
                    value: String(p.id),
                    label: p.name,
                    detail: p.isExpired ? "Expired" : undefined,
                  }))}
                />
                <p className="hd-form-note">
                  Please confirm that the new policy actually covers these records.
                </p>
                {target?.isExpired && (
                  <p className="hd-form-note">
                    That policy has expired, so they would still count as uninsured.
                  </p>
                )}
              </>
            )}
            {choice === "unschedule" && (
              <>
                <p className="hd-form-note">
                  {impact.isExpired
                    ? "This changes nothing about their coverage, since the policy has already expired."
                    : "They lose the coverage scheduled on this policy."}
                </p>
                {strongConfirmation && (
                  <Checkbox
                    label={`${plural(scheduled, "record", "records")} will lose their scheduled coverage`}
                    checked={understood}
                    onCheckedChange={setUnderstood}
                  />
                )}
              </>
            )}
          </>
        )}
      </div>
    </ConfirmDialog>
  );
}
