import type { ReactNode } from "react";
import { ConfirmDialog } from "../../components";

export interface RememberPassphraseConfirmProps {
  open: boolean;
  /** The database whose passphrase would be saved. */
  name: string;
  /** It closed, confirmed or not. */
  onClose: () => void;
  /** Handled like `ConfirmDialog`'s: a rejected promise keeps it open. */
  onConfirm: () => void | Promise<unknown>;
  /** Asked for with the confirmation, such as the passphrase in the
   * settings. */
  children?: ReactNode;
}

/** What saving a passphrase on this computer gives away (FR-017, US5-1,
 * contracts/ui-databases.md §8), confirmed before it is saved. */
function rememberFacts(name: string): string {
  return `Anyone who can use this computer account, or its keyring while it's unlocked, will be able to open ${name} without knowing the passphrase. On a shared account this defeats the passphrase. Locking will no longer need the passphrase on this computer, though it still clears the collection from memory, deletes opened document copies, and releases the database for other computers.`;
}

/** The FR-017 confirmation, shown before a passphrase is saved in this
 * computer's keyring, from the chooser or the database's settings. It is a
 * choice, not a destructive action, so it isn't styled as one. */
export function RememberPassphraseConfirm({
  open,
  name,
  onClose,
  onConfirm,
  children,
}: RememberPassphraseConfirmProps) {
  return (
    <ConfirmDialog
      open={open}
      onOpenChange={(next) => !next && onClose()}
      title={`Remember the passphrase of ${name}?`}
      description={rememberFacts(name)}
      confirmLabel="Remember passphrase"
      destructive={false}
      onConfirm={onConfirm}
    >
      {children}
    </ConfirmDialog>
  );
}
