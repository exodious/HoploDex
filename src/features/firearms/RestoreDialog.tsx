import { useState } from "react";
import { ChoiceCards, ConfirmDialog, TextField } from "../../components";
import { formatDate } from "../../lib/dates";
import { formatDollars } from "../../lib/money";
import { CommandFailure } from "../../services/tauriClient";
import { firearmName } from "../app/collectionStore";
import { useDirtyForm } from "../session/usePendingDraft";
import { dispositionLabel } from "./types";
import type { Firearm, HistoryChoice, ReverseDispositionInput } from "./types";
import "./forms.css";

export interface RestoreDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  firearm: Firearm;
  /** Rejects with a `CommandFailure` when the restored record would clash
   * with an active one (FR-031/FR-032); the dialog then shows why. */
  onRestore: (input: ReverseDispositionInput) => Promise<void>;
}

/** Reverses a disposition (FR-033): the firearm returns to the active
 * collection, and the user decides whether its disposition details are
 * kept as history or discarded. That choice is never defaulted. */
export function RestoreDialog({ open, onOpenChange, firearm, onRestore }: RestoreDialogProps) {
  // Mounted only while open, so every opening starts from a blank choice.
  return open ? (
    <RestoreDialogBody onOpenChange={onOpenChange} firearm={firearm} onRestore={onRestore} />
  ) : null;
}

const HISTORY_OPTIONS: { value: HistoryChoice; label: string; description: string }[] = [
  {
    value: "keep",
    label: "Keep as history",
    description: "The details stay on the record under “Earlier dispositions”.",
  },
  {
    value: "discard",
    label: "Discard",
    description: "The details are removed permanently.",
  },
];

function RestoreDialogBody({ onOpenChange, firearm, onRestore }: Omit<RestoreDialogProps, "open">) {
  const [history, setHistory] = useState<HistoryChoice | "">("");
  const [failure, setFailure] = useState<CommandFailure | null>(null);
  // Once the nickname has clashed, the user can pick another right here.
  const [renaming, setRenaming] = useState(false);
  const [nickname, setNickname] = useState(firearm.nickname ?? "");
  // specs/002-firearm-identification US3-9: once an ORIGINAL_MARKS_MATCH
  // warning has been shown, the same button resends confirmed.
  const [confirmedWarnings, setConfirmedWarnings] = useState(false);

  const nicknameError = failure?.fieldErrors?.nickname;
  const otherError = failure && !nicknameError ? failure.message : undefined;

  async function confirm() {
    if (!history) return;
    setFailure(null);
    const renamed = renaming && nickname.trim() !== "";
    try {
      await onRestore({
        history,
        ...(renamed ? { nickname: nickname.trim() } : {}),
        ...(confirmedWarnings ? { confirmedWarnings: true } : {}),
      });
    } catch (e) {
      const error =
        e instanceof CommandFailure
          ? e
          : new CommandFailure({
              code: "INTERNAL_ERROR",
              message: "The firearm couldn't be restored.",
            });
      setFailure(error);
      if (error.fieldErrors?.nickname) setRenaming(true);
      if (error.code === "ORIGINAL_MARKS_MATCH") setConfirmedWarnings(true);
      throw error; // keeps the dialog open
    }
  }

  // Closing or quitting asks about unsaved input first (specs/003 FR-010).
  useDirtyForm({
    label: `${firearmName(firearm)} (restore)`,
    isDirty: history !== "" || (renaming && nickname !== (firearm.nickname ?? "")),
    submit: async () => {
      if (!history) return false;
      try {
        await confirm();
      } catch {
        return false;
      }
      onOpenChange(false);
      return true;
    },
  });

  return (
    <ConfirmDialog
      open
      onOpenChange={onOpenChange}
      title="Restore to the collection?"
      description={`${firearmName(firearm)} becomes active again, and its value and coverage count toward your totals.`}
      confirmLabel={confirmedWarnings ? "Restore anyway" : "Restore to collection"}
      destructive={false}
      confirmDisabled={!history}
      onConfirm={confirm}
    >
      <div className="hd-form-section">
        {otherError && (
          <p className="hd-banner hd-banner--error" role="alert">
            {otherError}
          </p>
        )}
        <p className="hd-form-note">
          Recorded disposition: {dispositionLabel(firearm.dispositionType)} to{" "}
          {firearm.dispositionRecipient}, {formatDate(firearm.dispositionDate)}
          {firearm.dispositionPrice != null && <>, {formatDollars(firearm.dispositionPrice)}</>}.
        </p>
        <ChoiceCards<HistoryChoice>
          label="What should happen to these details?"
          required
          value={history}
          onChange={setHistory}
          options={HISTORY_OPTIONS}
          minCardWidth={200}
        />
        {renaming && (
          <TextField
            label="New nickname"
            value={nickname}
            onChange={(e) => setNickname(e.target.value)}
            error={nicknameError}
            hint="Another active firearm has this nickname. Choose a different one, or leave it blank to have none."
          />
        )}
      </div>
    </ConfirmDialog>
  );
}
