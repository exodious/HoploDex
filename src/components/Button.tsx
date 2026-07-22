import { forwardRef } from "react";
import type { ButtonHTMLAttributes } from "react";
import "./components.css";

export type ButtonVariant = "primary" | "secondary" | "danger";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
}

/** Shared button styling/behavior — no screen should style its own <button>. */
export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ variant = "secondary", className, type = "button", ...props }, ref) => {
    const classes = ["hd-button", `hd-button--${variant}`, className].filter(Boolean).join(" ");
    return <button ref={ref} type={type} className={classes} {...props} />;
  },
);

Button.displayName = "Button";
