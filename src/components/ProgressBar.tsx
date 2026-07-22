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
}

interface ProgressPayload {
  processed: number;
  total: number;
}

/** The single shared progress indicator for every long-running bulk
 * operation (import/export), consuming a Tauri progress event
 * (constitution Principle IV: no user-facing operation may run without
 * progress indication). */
export function ProgressBar({ eventName, label }: ProgressBarProps) {
  const [progress, setProgress] = useState<ProgressPayload>({ processed: 0, total: 0 });

  useEffect(() => {
    const unlisten = listen<ProgressPayload>(eventName, (event) => {
      setProgress(event.payload);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [eventName]);

  const percent = progress.total > 0 ? Math.round((progress.processed / progress.total) * 100) : 0;

  return (
    <div
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={progress.total}
      aria-valuenow={progress.processed}
      className="hd-progress"
    >
      <div className="hd-progress__fill" style={{ width: `${percent}%` }} />
      <span className="hd-progress__label">
        {progress.processed} / {progress.total || "?"}
      </span>
    </div>
  );
}
