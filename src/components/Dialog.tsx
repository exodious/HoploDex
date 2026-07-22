import type { ReactNode } from "react";
import * as RadixDialog from "@radix-ui/react-dialog";
import "./components.css";

export interface DialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  children: ReactNode;
}

/** Shared dialog shell (focus trap, Escape-to-close, ARIA labelling via Radix) — no screen builds its own modal. */
export function Dialog({ open, onOpenChange, title, description, children }: DialogProps) {
  return (
    <RadixDialog.Root open={open} onOpenChange={onOpenChange}>
      <RadixDialog.Portal>
        <RadixDialog.Overlay className="hd-dialog__overlay" />
        <RadixDialog.Content className="hd-dialog__content">
          <RadixDialog.Title className="hd-dialog__title">{title}</RadixDialog.Title>
          {description && (
            <RadixDialog.Description className="hd-dialog__description">
              {description}
            </RadixDialog.Description>
          )}
          {children}
        </RadixDialog.Content>
      </RadixDialog.Portal>
    </RadixDialog.Root>
  );
}
