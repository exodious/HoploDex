import { Component, useEffect, useRef, useState } from "react";
import type { ErrorInfo, ReactNode } from "react";
import { Button, Icon } from "../../components";
import { BrandMark } from "../app/BrandMark";
import * as sessionService from "./sessionService";
import "../app/AppShell.css";
import "../databases/databases.css";

interface FaultBoundaryProps {
  /** Shown in place of `children` once one of them has failed to render. */
  fallback: ReactNode;
  children: ReactNode;
}

/** Catches a failure to render `children`, which would otherwise unmount the
 * whole window and leave it blank, with nothing listening for the window's
 * close button (research.md §17). */
export class FaultBoundary extends Component<FaultBoundaryProps, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  componentDidCatch(error: unknown, info: ErrorInfo) {
    // Only this computer's own console: nothing is reported anywhere
    // (constitution V).
    console.error("HoploDex could not show this screen", error, info.componentStack);
  }

  render() {
    return this.state.failed ? this.props.fallback : this.props.children;
  }
}

interface FaultScreenProps {
  title: string;
  children: ReactNode;
  /** The one way on from here. */
  actionLabel: string;
  onAction: () => Promise<void>;
}

/** A full-window panel, laid out like the closing screen, saying a screen
 * couldn't be shown and offering the way out. */
function FaultScreen({ title, children, actionLabel, onAction }: FaultScreenProps) {
  const [busy, setBusy] = useState(false);
  const action = useRef<HTMLButtonElement>(null);
  useEffect(() => action.current?.focus(), []);

  return (
    <div className="hd-chooser hd-closing">
      <header className="hd-topbar">
        <div className="hd-topbar__inner">
          <div className="hd-brand hd-brand--static">
            <BrandMark />
            <span className="hd-brand__name">HoploDex</span>
          </div>
        </div>
      </header>
      <main className="hd-closing__main">
        <section className="hd-closing__panel hd-fault" role="alert">
          <Icon name="alert" size={28} />
          <h1 className="hd-closing__title">{title}</h1>
          <div className="hd-fault__text">{children}</div>
          <div>
            <Button
              ref={action}
              variant="primary"
              pending={busy}
              onClick={() => {
                setBusy(true);
                void onAction().finally(() => setBusy(false));
              }}
            >
              {actionLabel}
            </Button>
          </div>
        </section>
      </main>
    </div>
  );
}

/** In place of the collection when part of it failed to render: the session
 * is still there, so the database can be closed normally and opened again. */
export function CollectionFault({ name, onClose }: { name: string; onClose: () => Promise<void> }) {
  return (
    <FaultScreen
      title={`“${name}” couldn't be shown`}
      actionLabel="Close the database"
      onAction={onClose}
    >
      <p>
        Something went wrong while showing this screen. Everything saved is safe in the database,
        but changes that weren&apos;t saved yet are lost.
      </p>
      <p>Close the database and open it again to carry on.</p>
    </FaultScreen>
  );
}

/** In place of everything when the session itself failed to render. The
 * window's close button still quits, closing any open database normally. */
export function ApplicationFault() {
  useEffect(() => sessionService.onQuitRequested(() => void sessionService.quitApplication()), []);
  return (
    <FaultScreen
      title="HoploDex stopped working"
      actionLabel="Quit HoploDex"
      onAction={sessionService.quitApplication}
    >
      <p>
        Something went wrong while showing this screen. Quitting closes any open database normally,
        and everything saved in it is safe.
      </p>
    </FaultScreen>
  );
}
