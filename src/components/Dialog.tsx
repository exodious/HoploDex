import { useRef } from "react";
import type { ReactNode } from "react";
import * as RadixDialog from "@radix-ui/react-dialog";
import { Icon } from "./Icon";
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
  children: ReactNode;
}

/** Shared dialog shell (focus trap, Escape-to-close, ARIA labelling via
 * Radix) — no screen builds its own modal. Header and footer stay put
 * while a long body scrolls, so a form's Save is always in reach. */
export function Dialog({
  open,
  onOpenChange,
  title,
  description,
  footer,
  size = "md",
  bare = false,
  children,
}: DialogProps) {
  const contentRef = useRef<HTMLDivElement>(null);
  return (
    <RadixDialog.Root open={open} onOpenChange={onOpenChange}>
      <RadixDialog.Portal>
        <RadixDialog.Overlay className="hd-dialog__overlay" />
        <RadixDialog.Content
          ref={contentRef}
          className={`hd-dialog__content hd-dialog__content--${size}`}
          // Radix wires aria-describedby to the Description automatically;
          // opting out explicitly when there is none silences its warning.
          {...(description ? {} : { "aria-describedby": undefined })}
          onOpenAutoFocus={(event) => {
            // Start in the first field rather than on the header's close
            // button, which Radix would otherwise focus first.
            const first = contentRef.current?.querySelector<HTMLElement>(FIRST_FIELD);
            if (first) {
              event.preventDefault();
              first.focus();
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
            <RadixDialog.Close className="hd-dialog__close" aria-label="Close">
              <Icon name="close" />
            </RadixDialog.Close>
          </header>
          {bare ? children : <div className="hd-dialog__body">{children}</div>}
          {footer && <footer className="hd-dialog__footer">{footer}</footer>}
        </RadixDialog.Content>
      </RadixDialog.Portal>
    </RadixDialog.Root>
  );
}
