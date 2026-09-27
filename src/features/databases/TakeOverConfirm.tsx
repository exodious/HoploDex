import { useRef, useState } from "react";
import { ConfirmDialog, PassphraseField } from "../../components";
import type { PassphraseFieldHandle } from "../../components";

export interface TakeOverTarget {
  name: string;
  machineName: string;
}

export interface TakeOverConfirmProps {
  /** The database to take over, or `null` while not asking. */
  target: TakeOverTarget | null;
  onCancel: () => void;
  /** Called with the passphrase typed here, which is read once and cleared
   * (FR-007). */
  onConfirm: (passphrase: string) => void;
}

/** The destructive confirmation before opening a database marked open on
 * another computer (FR-032, US2-10, contracts/ui-databases.md §1). Typing
 * the passphrase again is the confirmation: the one that found the marker
 * was cleared when it was read, and none is kept in between. */
export function TakeOverConfirm({ target, onCancel, onConfirm }: TakeOverConfirmProps) {
  const field = useRef<PassphraseFieldHandle>(null);
  const [error, setError] = useState<string | undefined>();

  function confirm() {
    const passphrase = field.current?.read() ?? "";
    field.current?.reset();
    if (passphrase === "") {
      setError("Enter the passphrase to take it over.");
      // Keeps the dialog open.
      return Promise.reject(new Error("no passphrase"));
    }
    onConfirm(passphrase);
  }

  return (
    <ConfirmDialog
      open={target !== null}
      onOpenChange={(open) => {
        if (open) return;
        setError(undefined);
        onCancel();
      }}
      title={`Take over “${target?.name ?? ""}”?`}
      description={
        target &&
        `Only do this if ${target.machineName} no longer has “${target.name}” open, or if it crashed. If it still has it open, or its latest changes haven't synced here yet, those changes can be lost.`
      }
      confirmLabel="Take over"
      onConfirm={confirm}
    >
      {target && (
        <PassphraseField
          ref={field}
          label={`Passphrase for “${target.name}”`}
          hint="Enter it again to confirm the take-over."
          autoComplete="current-password"
          autoFocus
          error={error}
          onInput={() => setError(undefined)}
        />
      )}
    </ConfirmDialog>
  );
}
