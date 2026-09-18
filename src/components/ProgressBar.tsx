import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import "./components.css";

export interface ProgressBarProps {
  /** Tauri event name, e.g. `"import_collection:progress"`
   * (contracts/tauri-commands.md: `"{command}:progress"`). */
  eventName: string;
  /** Accessible name for the progressbar role (WCAG 4.1.2 Name, Role,
   * Value) — e.g. "Export progress". */
  label: string;
  /** What's being counted, e.g. "firearms" or "rows". */
  unit: string;
}

interface ProgressPayload {
  processed: number;
  total: number;
}

/** The single shared progress indicator for every long-running bulk
 * operation (import/export), consuming a Tauri progress event
 * (constitution Principle IV: no user-facing operation may run without
 * progress indication). Indeterminate until the first event arrives. */
export function ProgressBar({ eventName, label, unit }: ProgressBarProps) {
  const [progress, setProgress] = useState<ProgressPayload | null>(null);

  useEffect(() => {
    const unlisten = listen<ProgressPayload>(eventName, (event) => {
      setProgress(event.payload);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [eventName]);

  const determinate = progress != null && progress.total > 0;
  const percent = determinate ? Math.round((progress.processed / progress.total) * 100) : 0;

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
      <p className="hd-progress__label hd-num">
        {determinate ? `${progress.processed} of ${progress.total} ${unit}` : "Starting…"}
      </p>
    </div>
  );
}
