import { useEffect, useRef, useState } from "react";
import { Button, placeFocus } from "../../components";
import {
  getTroubleshootingLog,
  setTroubleshootingLog,
  showTroubleshootingLog,
} from "./troubleshootingLog";
import type { LogState, TroubleshootingLog } from "./troubleshootingLog";

const LEAVES_OUT =
  "It is designed to leave out your records, file names, passphrases and how many items you have. Read it before you share it.";

const SHOW_FAILED = "The log file couldn't be shown.";

function stateText(log: TroubleshootingLog): string {
  if (log.state === "off") return "Off. Turn it on to gather details for a problem you can repeat.";
  if (log.failed) return "On, but the file can't be written.";
  return log.state === "detailed" ? "On, with extra detail until you quit." : "On.";
}

/** About's troubleshooting log section (013 contracts/ui.md): turn it on and off,
 * raise the level until quit, see the path, show the file. */
export function TroubleshootingLogSection() {
  const [log, setLog] = useState<TroubleshootingLog | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const section = useRef<HTMLDivElement>(null);
  const moveFocus = useRef(false);

  useEffect(() => {
    let current = true;
    getTroubleshootingLog().then(
      (loaded) => {
        if (current) setLog(loaded ?? null);
      },
      () => {},
    );
    return () => {
      current = false;
    };
  }, []);

  // The pressed button may have gone with the old state.
  useEffect(() => {
    if (!moveFocus.current) return;
    moveFocus.current = false;
    if (!section.current?.contains(document.activeElement)) {
      placeFocus(section.current?.querySelector<HTMLElement>("button:not(:disabled)") ?? null);
    }
  }, [log]);

  function change(state: LogState) {
    setProblem(null);
    moveFocus.current = true;
    setTroubleshootingLog(state).then(setLog, () => {
      moveFocus.current = false;
    });
  }

  function show() {
    setProblem(null);
    showTroubleshootingLog().catch(() => setProblem(SHOW_FAILED));
  }

  if (!log) return null;
  const on = log.state !== "off";
  return (
    <div className="hd-about__log" ref={section}>
      <h3 className="hd-about__heading">Troubleshooting log</h3>
      <p>{stateText(log)}</p>
      {on && log.path && <p className="hd-about__log-path">{log.path}</p>}
      <p>{LEAVES_OUT}</p>
      {problem && (
        <p className="hd-banner hd-banner--error" role="alert">
          {problem}
        </p>
      )}
      <div className="hd-about__log-actions">
        {!on && <Button onClick={() => change("on")}>Turn on</Button>}
        {log.state === "on" && (
          <Button onClick={() => change("detailed")}>Detailed until you quit</Button>
        )}
        {log.state === "detailed" && <Button onClick={() => change("on")}>Normal detail</Button>}
        {on && (
          <Button disabled={!log.exists} onClick={show}>
            Show log file
          </Button>
        )}
        {on && <Button onClick={() => change("off")}>Turn off</Button>}
      </div>
    </div>
  );
}
