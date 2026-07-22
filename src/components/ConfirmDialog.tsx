import * as RadixDialog from "@radix-ui/react-dialog";
import { Button } from "./Button";
import "./components.css";

export interface ConfirmDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: string;
  confirmLabel?: string;
  cancelLabel?: string;
  destructive?: boolean;
  onConfirm: () => void;
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
}: ConfirmDialogProps) {
  return (
    <RadixDialog.Root open={open} onOpenChange={onOpenChange}>
      <RadixDialog.Portal>
        <RadixDialog.Overlay className="hd-dialog__overlay" />
        <RadixDialog.Content className="hd-dialog__content" role="alertdialog">
          <RadixDialog.Title className="hd-dialog__title">{title}</RadixDialog.Title>
          <RadixDialog.Description className="hd-dialog__description">
            {description}
          </RadixDialog.Description>
          <div className="hd-dialog__actions">
            <RadixDialog.Close asChild>
              <Button variant="secondary">{cancelLabel}</Button>
            </RadixDialog.Close>
            <Button
              variant={destructive ? "danger" : "primary"}
              onClick={() => {
                onConfirm();
                onOpenChange(false);
              }}
            >
              {confirmLabel}
            </Button>
          </div>
        </RadixDialog.Content>
      </RadixDialog.Portal>
    </RadixDialog.Root>
  );
}
