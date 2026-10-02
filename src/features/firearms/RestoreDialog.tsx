import { useState } from "react";
import { ChoiceCards, ConfirmDialog, TextField } from "../../components";
import { formatDate } from "../../lib/dates";
import { formatDollars } from "../../lib/money";
import { CommandFailure } from "../../services/tauriClient";
import { resumedValues, useDirtyForm, useResumedDraftTaken } from "../session/usePendingDraft";
import type { DraftTarget } from "../session/usePendingDraft";
import { useRecordSubject } from "./recordSubject";
import type { RecordSubjectProps } from "./recordSubject";
import { dispositionLabel } from "./types";
import type { HistoryChoice, ReverseDispositionInput } from "./types";
import "./forms.css";

export type RestoreDialogProps = RecordSubjectProps & {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Rejects with a `CommandFailure` when the restored record would clash
   * with an active one (FR-031/FR-032); the dialog then shows why. */
  onRestore: (input: ReverseDispositionInput) => Promise<void>;
};

/** Reverses a disposition (FR-033): the firearm or accessory returns to the
 * active collection, and the user decides whether its disposition details are
 * kept as history or discarded. That choice is never defaulted. */
export function RestoreDialog({ open, onOpenChange, onRestore, ...record }: RestoreDialogProps) {
  // Mounted only while open, so every opening starts from a blank choice.
  return open ? (
    <RestoreDialogBody
      onOpenChange={onOpenChange}
      onRestore={onRestore}
      {...(record as RecordSubjectProps)}
    />
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

/** The version of this form's kept drafts (research.md §16). Raise it when
 * {@link RestoreValues} changes shape, so older drafts are only discarded. */
export const FORM_VERSION = 1;

/** The dialog's input, as a kept draft holds it. */
interface RestoreValues {
  history: HistoryChoice | "";
  renaming: boolean;
  nickname: string;
}

function RestoreDialogBody({
  onOpenChange,
  onRestore,
  ...record
}: Omit<RestoreDialogProps, "open">) {
  const subject = useRecordSubject(record as RecordSubjectProps);
  const target: DraftTarget = {
    formVersion: FORM_VERSION,
    kind: subject.kind,
    mode: "restore",
    targetId: subject.id,
  };
  // Pending changes the user resumed start as unsaved input (FR-039).
  const [resumed] = useState(() =>
    resumedValues<RestoreValues>(target, {
      history: "",
      renaming: false,
      nickname: subject.nickname ?? "",
    }),
  );
  useResumedDraftTaken(target);
  const [history, setHistory] = useState<HistoryChoice | "">(resumed.history);
  const [failure, setFailure] = useState<CommandFailure | null>(null);
  // Once the nickname has clashed, the user can pick another right here. Only
  // a firearm has a nickname, so an accessory never clashes (006 FR-006).
  const [renaming, setRenaming] = useState(resumed.renaming);
  const [nickname, setNickname] = useState(resumed.nickname);
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
              message: `The ${subject.noun} couldn't be restored.`,
            });
      setFailure(error);
      if (error.fieldErrors?.nickname) setRenaming(true);
      if (error.code === "ORIGINAL_MARKS_MATCH") setConfirmedWarnings(true);
      throw error; // keeps the dialog open
    }
  }

  // Closing or quitting asks about unsaved input first (specs/003 FR-010),
  // and a lock keeps it (FR-039).
  const values: RestoreValues = { history, renaming, nickname };
  useDirtyForm({
    label: `${subject.name} (restore)`,
    isDirty: history !== "" || (renaming && nickname !== (subject.nickname ?? "")),
    draft: { ...target, values },
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
      description={`${subject.name} becomes active again, and its value and coverage count toward your totals.`}
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
          Recorded disposition: {dispositionLabel(subject.dispositionType)} to{" "}
          {subject.dispositionRecipient}, {formatDate(subject.dispositionDate)}
          {subject.dispositionPrice != null && <>, {formatDollars(subject.dispositionPrice)}</>}.
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
