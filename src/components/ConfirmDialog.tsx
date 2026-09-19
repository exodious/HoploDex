import { useState } from "react";
import type { ReactNode } from "react";
import * as RadixDialog from "@radix-ui/react-dialog";
import { Button } from "./Button";
import "./components.css";

export interface ConfirmDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: ReactNode;
  confirmLabel?: string;
  cancelLabel?: string;
  destructive?: boolean;
  /** May return a promise; the dialog stays open, showing progress, until
   * it settles. */
  onConfirm: () => void | Promise<unknown>;
  children?: ReactNode;
}

/**
 * The single confirmation pattern for every destructive action in the app
 * (delete firearm, delete policy, delete photo/document, bulk overwrite on
 * import) per constitution Principle III — no screen invents its own.
 */
export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  destructive = true,
  onConfirm,
  children,
}: ConfirmDialogProps) {
  const [pending, setPending] = useState(false);

  function handleConfirm() {
    const result = onConfirm();
    if (result instanceof Promise) {
      setPending(true);
      void result.finally(() => {
        setPending(false);
        onOpenChange(false);
      });
    } else {
      onOpenChange(false);
    }
  }

  return (
    <RadixDialog.Root open={open} onOpenChange={(next) => !pending && onOpenChange(next)}>
      <RadixDialog.Portal>
        <RadixDialog.Overlay className="hd-dialog__overlay" />
        <RadixDialog.Content
          className="hd-dialog__content hd-dialog__content--sm"
          role="alertdialog"
        >
          <header className="hd-dialog__header">
            <div>
              <RadixDialog.Title className="hd-dialog__title">{title}</RadixDialog.Title>
              <RadixDialog.Description className="hd-dialog__description">
                {description}
              </RadixDialog.Description>
            </div>
          </header>
          {children && <div className="hd-dialog__body">{children}</div>}
          <footer className="hd-dialog__footer">
            <RadixDialog.Close asChild>
              <Button variant="secondary" disabled={pending}>
                {cancelLabel}
              </Button>
            </RadixDialog.Close>
            <Button
              variant={destructive ? "danger" : "primary"}
              pending={pending}
              onClick={handleConfirm}
            >
              {confirmLabel}
            </Button>
          </footer>
        </RadixDialog.Content>
      </RadixDialog.Portal>
    </RadixDialog.Root>
  );
}
