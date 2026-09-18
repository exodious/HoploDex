import { forwardRef } from "react";
import type { ButtonHTMLAttributes } from "react";
import { Icon } from "./Icon";
import type { IconName } from "./Icon";
import "./components.css";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: "md" | "sm";
  icon?: IconName;
  /** Shows a spinner and blocks repeat activation while an action runs —
   * visible feedback within 100ms (constitution Principle IV). */
  pending?: boolean;
}

/** Shared button styling/behavior — no screen should style its own <button>. */
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
      children,
      ...props
    },
    ref,
  ) => {
    const classes = [
      "hd-button",
      `hd-button--${variant}`,
      size === "sm" && "hd-button--sm",
      !children && "hd-button--icon-only",
      className,
    ]
      .filter(Boolean)
      .join(" ");
    return (
      <button
        ref={ref}
        type={type}
        className={classes}
        disabled={disabled || pending}
        aria-busy={pending || undefined}
        {...props}
      >
        {pending ? (
          <span className="hd-spinner" aria-hidden />
        ) : (
          icon && <Icon name={icon} size={size === "sm" ? 16 : 18} />
        )}
        {children}
      </button>
    );
  },
);

Button.displayName = "Button";
