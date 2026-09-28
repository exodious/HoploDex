import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import "./components.css";

export interface ProgressBarProps {
  /** Tauri event name, e.g. `"import_collection:progress"`
   * (contracts/tauri-commands.md: `"{command}:progress"`). Leave it out and
   * pass `value` for progress the caller already follows. */
  eventName?: string;
  /** The progress to show when there is no `eventName`; `null` or a zero
   * `total` is indeterminate. */
  value?: ProgressPayload | null;
  /** Accessible name for the progressbar role (WCAG 4.1.2 Name, Role,
   * Value) — e.g. "Export progress". */
  label: string;
  /** What's being counted, e.g. "firearms" or "rows". */
  unit?: string;
  /** Writes an amount, e.g. as bytes; the default writes the number and
   * `unit`. */
  formatAmount?: (amount: number) => string;
  /** Replaces the "x of y" text; `""` shows none, for a step the caller
   * already names. */
  caption?: string;
}

export interface ProgressPayload {
  processed: number;
  total: number;
}

/** The single shared progress indicator for every long-running bulk
 * operation (import/export), consuming a Tauri progress event
 * (constitution Principle IV: no user-facing operation may run without
 * progress indication). Indeterminate until the first event arrives. */
export function ProgressBar({
  eventName,
  value = null,
  label,
  unit = "",
  formatAmount,
  caption,
}: ProgressBarProps) {
  const [heard, setHeard] = useState<ProgressPayload | null>(null);

  useEffect(() => {
    if (!eventName) return;
    const unlisten = listen<ProgressPayload>(eventName, (event) => {
      setHeard(event.payload);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [eventName]);

  const progress = eventName ? heard : value;
  const determinate = progress != null && progress.total > 0;
  const percent = determinate ? Math.round((progress.processed / progress.total) * 100) : 0;
  const amount = (n: number) => (formatAmount ? formatAmount(n) : `${n}`);
  const counted = determinate
    ? formatAmount
      ? `${amount(progress.processed)} of ${amount(progress.total)}`
      : `${progress.processed} of ${progress.total} ${unit}`
    : "Starting…";

  return (
    <div className="hd-progress">
      <div
        role="progressbar"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={determinate ? progress.total : undefined}
        aria-valuenow={determinate ? progress.processed : undefined}
        className={["hd-progress__track", !determinate && "hd-progress__track--indeterminate"]
          .filter(Boolean)
          .join(" ")}
      >
        <div
          className="hd-progress__fill"
          style={determinate ? { width: `${percent}%` } : undefined}
        />
      </div>
      {caption !== "" && <p className="hd-progress__label hd-num">{caption ?? counted}</p>}
    </div>
  );
}
