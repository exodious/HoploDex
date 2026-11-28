import { forwardRef, useCallback, useId, useLayoutEffect, useRef } from "react";
import type { ButtonHTMLAttributes } from "react";
import { Icon } from "./Icon";
import type { IconName } from "./Icon";
import { placeFocus } from "./placeFocus";
import "./components.css";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: "md" | "sm";
  icon?: IconName;
  /** Shows a spinner and blocks repeat activation while an action runs —
   * visible feedback within 100ms (constitution Principle IV). */
  pending?: boolean;
  /** Why the button can't act now, such as "No changes to save". It then
   * looks disabled but stays in the tab order, its reason as its tooltip and
   * description, and activating it does nothing (014 FR-016). */
  unavailable?: string;
}

/** Shared button styling/behavior — no screen should style its own <button>.
 * `autoFocus` focuses it on mount with {@link placeFocus}, so the ring shows
 * only to someone using the keyboard. */
export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  (
    {
      variant = "secondary",
      size = "md",
      icon,
      pending = false,
      className,
      type = "button",
      disabled,
      autoFocus,
      unavailable,
      onClick,
      children,
      ...props
    },
    ref,
  ) => {
    const reasonId = useId();
    const own = useRef<HTMLButtonElement | null>(null);
    const setRef = useCallback(
      (node: HTMLButtonElement | null) => {
        own.current = node;
        if (typeof ref === "function") ref(node);
        else if (ref) ref.current = node;
      },
      [ref],
    );
    // Mount only, like the attribute.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    useLayoutEffect(() => (autoFocus ? placeFocus(own.current) : undefined), []);
    const classes = [
      "hd-button",
      `hd-button--${variant}`,
      size === "sm" && "hd-button--sm",
      !children && "hd-button--icon-only",
      className,
    ]
      .filter(Boolean)
      .join(" ");
    const button = (
      <button
        ref={setRef}
        type={type}
        className={classes}
        disabled={disabled || pending}
        aria-busy={pending || undefined}
        {...props}
        aria-disabled={unavailable ? true : props["aria-disabled"]}
        aria-describedby={unavailable ? reasonId : props["aria-describedby"]}
        title={unavailable ?? props.title}
        onClick={(event) => {
          // Also stops a submit button's form, which Enter in a field reaches
          // through a click on it (the Enter rule, #44).
          if (unavailable) event.preventDefault();
          else onClick?.(event);
        }}
      >
        {pending ? (
          <span className="hd-spinner" aria-hidden />
        ) : (
          icon && <Icon name={icon} size={size === "sm" ? 16 : 18} />
        )}
        {children}
      </button>
    );
    if (!unavailable) return button;
    // Outside the button, so the reason describes it rather than joining its name.
    return (
      <>
        {button}
        <span id={reasonId} className="hd-sr-only">
          {unavailable}
        </span>
      </>
    );
  },
);

Button.displayName = "Button";
