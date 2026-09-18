import { createContext, useContext } from "react";

export type ToastTone = "success" | "error";

export type Notify = (message: string, tone?: ToastTone) => void;

export const ToastContext = createContext<Notify>(() => {});

/** Confirms that an action took effect ("Saved changes to Glock 19") or
 * reports why it didn't — the shared feedback pattern for every mutation. */
export function useToast(): Notify {
  return useContext(ToastContext);
}
