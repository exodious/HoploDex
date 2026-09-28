import { useEffect, useRef } from "react";
import type { ReactNode } from "react";
import * as RadixDialog from "@radix-ui/react-dialog";
import { Icon } from "./Icon";
import { LOCK_SHORTCUT, useLock } from "./lock";
import { placeFocus } from "./placeFocus";
import "./components.css";

const FIRST_FIELD = [
  "[autofocus]",
  ".hd-dialog__body input:not([type='hidden']):not([tabindex='-1']):not(:disabled)",
  ".hd-dialog__body textarea",
  ".hd-dialog__body button:not(:disabled)",
].join(", ");

export interface DialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: ReactNode;
  /** Pinned below the scrolling body — the dialog's actions. */
  footer?: ReactNode;
  size?: "sm" | "md" | "lg";
  /** For forms: children render their own `.hd-dialog__body` and
   * `.hd-dialog__footer` (inside a `.hd-dialog__form`), keeping the submit
   * button inside the <form> so Enter submits and pending state is local. */
  bare?: boolean;
  /** `false` while something runs that must not be interrupted: no close
   * button, and Escape or a click outside do nothing. */
  dismissible?: boolean;
  children: ReactNode;
}

/** Shared dialog shell (focus trap, Escape-to-close, ARIA labelling via
 * Radix) — no screen builds its own modal. Header and footer stay put
 * while a long body scrolls, so a form's Save is always in reach. While a
 * database is open, the header also has a lock button beside the close
 * button, since the dialog covers the top bar's (contracts/ui-databases.md
 * §4); a lock keeps a form's unsaved input as pending changes (FR-039). */
export function Dialog({
  open,
  onOpenChange,
  title,
  description,
  footer,
  size = "md",
  bare = false,
  dismissible = true,
  children,
}: DialogProps) {
  const contentRef = useRef<HTMLDivElement>(null);
  const lock = useLock();
  // No screen wires its opening button up as a Radix `Trigger` (each opens
  // from its own state instead), so Radix's own focus-restore never fires;
  // this captures and restores it here, once, for every dialog.
  const previouslyFocused = useRef<HTMLElement | null>(null);
  useEffect(() => {
    if (open) previouslyFocused.current = document.activeElement as HTMLElement | null;
  }, [open]);
  return (
    <RadixDialog.Root
      open={open}
      onOpenChange={(next) => (dismissible || next) && onOpenChange(next)}
    >
      <RadixDialog.Portal>
        <RadixDialog.Overlay className="hd-dialog__overlay" />
        <RadixDialog.Content
          ref={contentRef}
          className={`hd-dialog__content hd-dialog__content--${size}`}
          // Radix wires aria-describedby to the Description automatically;
          // opting out explicitly when there is none silences its warning.
          {...(description ? {} : { "aria-describedby": undefined })}
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            previouslyFocused.current?.focus();
          }}
          onOpenAutoFocus={(event) => {
            // Start in the first field rather than on the header's close
            // button, which Radix would otherwise focus first — unless the
            // content already put focus somewhere itself (a form opened on a
            // specific field).
            const focused = document.activeElement;
            if (
              focused &&
              focused !== contentRef.current &&
              contentRef.current?.contains(focused)
            ) {
              event.preventDefault();
              return;
            }
            const first = contentRef.current?.querySelector<HTMLElement>(FIRST_FIELD);
            if (first) {
              event.preventDefault();
              placeFocus(first);
            }
          }}
        >
          <header className="hd-dialog__header">
            <div>
              <RadixDialog.Title className="hd-dialog__title">{title}</RadixDialog.Title>
              {description && (
                <RadixDialog.Description className="hd-dialog__description">
                  {description}
                </RadixDialog.Description>
              )}
            </div>
            {dismissible && (
              <div className="hd-dialog__tools">
                {lock && (
                  <button
                    type="button"
                    className="hd-dialog__tool"
                    aria-label="Lock now"
                    title={`Lock now (${LOCK_SHORTCUT})`}
                    onClick={lock}
                  >
                    <Icon name="lock" />
                  </button>
                )}
                <RadixDialog.Close className="hd-dialog__tool" aria-label="Close">
                  <Icon name="close" />
                </RadixDialog.Close>
              </div>
            )}
          </header>
          {bare ? children : <div className="hd-dialog__body">{children}</div>}
          {footer && <footer className="hd-dialog__footer">{footer}</footer>}
        </RadixDialog.Content>
      </RadixDialog.Portal>
    </RadixDialog.Root>
  );
}
