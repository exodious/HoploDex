import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import * as RadixDialog from "@radix-ui/react-dialog";
import { Button } from "./Button";
import { placeFocus } from "./placeFocus";
import "./components.css";

export interface ConfirmDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: ReactNode;
  confirmLabel?: string;
  cancelLabel?: string;
  destructive?: boolean;
  /** Holds the confirm button back until the user has made a required
   * choice inside `children`. */
  confirmDisabled?: boolean;
  /** May return a promise; the dialog stays open, showing progress, until
   * it settles. If it rejects the dialog stays open afterwards too, so the
   * caller can show what went wrong (and let the user correct it) inside
   * `children`; a caller that reports failures elsewhere just doesn't
   * reject. */
  onConfirm: () => void | Promise<unknown>;
  /** An optional third choice beside cancel and confirm, in destructive
   * style: "Discard changes" in the save / discard / cancel question
   * (specs/003 contracts/ui-databases.md §6). Handled like `onConfirm`. */
  alternativeLabel?: string;
  onAlternative?: () => void | Promise<unknown>;
  children?: ReactNode;
}

/**
 * The single confirmation pattern for every destructive action in the app
 * (delete firearm, delete policy, delete photo/document, bulk overwrite on
 * import) per constitution Principle III — no screen invents its own. With
 * an alternative action it is also the save / discard / cancel question.
 */
export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  destructive = true,
  confirmDisabled = false,
  onConfirm,
  alternativeLabel,
  onAlternative,
  children,
}: ConfirmDialogProps) {
  const [pending, setPending] = useState<"confirm" | "alternative" | null>(null);
  // No screen wires its opening button up as a Radix `Trigger`, so Radix's
  // own focus-restore never fires; this captures and restores it here.
  const previouslyFocused = useRef<HTMLElement | null>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (open) previouslyFocused.current = document.activeElement as HTMLElement | null;
  }, [open]);

  function run(action: () => void | Promise<unknown>, which: "confirm" | "alternative") {
    const result = action();
    if (result instanceof Promise) {
      setPending(which);
      void result
        .then(
          () => onOpenChange(false),
          () => {}, // stay open; the caller shows the failure
        )
        .finally(() => setPending(null));
    } else {
      onOpenChange(false);
    }
  }

  return (
    <RadixDialog.Root open={open} onOpenChange={(next) => pending === null && onOpenChange(next)}>
      <RadixDialog.Portal>
        <RadixDialog.Overlay className="hd-dialog__overlay" />
        <RadixDialog.Content
          className="hd-dialog__content hd-dialog__content--sm"
          ref={contentRef}
          role="alertdialog"
          onOpenAutoFocus={(event) => {
            // Radix's own choice (the first control), placed like every
            // dialog's, unless the content already focused a field itself.
            const content = contentRef.current;
            const focused = document.activeElement;
            event.preventDefault();
            if (focused && focused !== content && content?.contains(focused)) return;
            placeFocus(
              content?.querySelector<HTMLElement>(
                "input:not([type='hidden']):not(:disabled), textarea, button:not(:disabled)",
              ) ?? content,
            );
          }}
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            previouslyFocused.current?.focus();
          }}
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
              <Button variant="secondary" disabled={pending !== null}>
                {cancelLabel}
              </Button>
            </RadixDialog.Close>
            {alternativeLabel && onAlternative && (
              <Button
                variant="danger"
                pending={pending === "alternative"}
                disabled={pending === "confirm"}
                onClick={() => run(onAlternative, "alternative")}
              >
                {alternativeLabel}
              </Button>
            )}
            <Button
              variant={destructive ? "danger" : "primary"}
              pending={pending === "confirm"}
              disabled={confirmDisabled || pending === "alternative"}
              onClick={() => run(onConfirm, "confirm")}
            >
              {confirmLabel}
            </Button>
          </footer>
        </RadixDialog.Content>
      </RadixDialog.Portal>
    </RadixDialog.Root>
  );
}
