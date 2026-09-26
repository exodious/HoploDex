import { useEffect, useMemo, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { Button } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import { BrandMark } from "../app/BrandMark";
import { ThemeToggle } from "../app/ThemeToggle";
import { useSession } from "../session/sessionStore";
import { CreateDatabaseDialog } from "./CreateDatabaseDialog";
import * as databasesService from "./databasesService";
import { databaseNameOf } from "./paths";
import { RecentDatabaseRow } from "./RecentDatabaseRow";
import type { ChooserRow } from "./RecentDatabaseRow";
import type { ChooserState } from "./types";
import "../app/AppShell.css";
import "./databases.css";

const WELCOME =
  "HoploDex keeps your collection in an encrypted database file that only your passphrase opens.";

/** What a failed open says, in the selected row (contracts/ui-databases.md
 * §1 "Open failures"). */
function openFailureText(name: string, error: unknown): string {
  if (error instanceof CommandFailure) {
    if (error.code === "PASSPHRASE_INCORRECT") {
      return `That passphrase didn't open ${name}. Either the passphrase is wrong, or the file isn't a HoploDex database or is damaged.`;
    }
    return error.message;
  }
  return `HoploDex couldn't open ${name}.`;
}

/** The full-window screen shown whenever no database is open (FR-020,
 * FR-021): the known databases, the selected one's passphrase, and the ways
 * to create or find another. It never shows anything from a collection. */
export function DatabaseChooser() {
  const session = useSession();
  const [state, setState] = useState<ChooserState | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  /** Files found with "Open another database file…", not yet in the list. */
  const [picked, setPicked] = useState<ChooserRow[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [opening, setOpening] = useState<string | null>(null);
  const [failure, setFailure] = useState<{ path: string; message: string } | null>(null);
  const [creating, setCreating] = useState(false);
  const [announcement, setAnnouncement] = useState("");

  useEffect(() => {
    let current = true;
    databasesService.getChooserState().then(
      (loaded) => {
        if (!current) return;
        setState(loaded);
        setSelected(loaded.selectedPath);
      },
      () => current && setLoadError("HoploDex couldn't read its list of databases."),
    );
    return () => {
      current = false;
    };
  }, []);

  const rows = useMemo<ChooserRow[]>(() => {
    const known = state?.recent ?? [];
    return [...picked.filter((row) => !known.some((k) => k.path === row.path)), ...known];
  }, [picked, state]);

  function select(path: string) {
    setSelected(path);
    setFailure(null);
  }

  async function openRow(row: ChooserRow, passphrase: string) {
    setOpening(row.path);
    setFailure(null);
    setAnnouncement(`Opening ${row.name}…`);
    try {
      // On success the session replaces this screen with the collection.
      await session.openDatabase(row.path, passphrase);
    } catch (error) {
      setFailure({ path: row.path, message: openFailureText(row.name, error) });
    } finally {
      setOpening(null);
      setAnnouncement("");
    }
  }

  async function openAnotherFile() {
    const chosen = await openFileDialog({
      multiple: false,
      directory: false,
      title: "Open a database file",
      filters: [
        { name: "HoploDex databases", extensions: ["hoplodex"] },
        { name: "All files", extensions: ["*"] },
      ],
    });
    if (typeof chosen !== "string") return;
    if (!rows.some((row) => row.path === chosen)) {
      setPicked((current) => [{ path: chosen, name: databaseNameOf(chosen) }, ...current]);
    }
    select(chosen);
  }

  const firstRun = state !== null && rows.length === 0;
  const actionClass = firstRun ? "hd-chooser__action--large" : undefined;

  return (
    <div className="hd-chooser">
      <header className="hd-topbar">
        <div className="hd-topbar__inner">
          <div className="hd-brand hd-brand--static">
            <BrandMark />
            <span className="hd-brand__name">HoploDex</span>
          </div>
          <div className="hd-topbar__tools">
            <ThemeToggle />
          </div>
        </div>
      </header>

      <main className="hd-chooser__main">
        {loadError && (
          <p className="hd-banner hd-banner--error" role="alert">
            {loadError}
          </p>
        )}
        {state && (
          <>
            <h1 className="hd-chooser__title">
              {firstRun ? "Welcome to HoploDex" : "Open a database"}
            </h1>
            {firstRun ? (
              <p className="hd-chooser__welcome">{WELCOME}</p>
            ) : (
              <ul className="hd-chooser__list" aria-label="Recent databases">
                {rows.map((row) => (
                  <RecentDatabaseRow
                    key={row.path}
                    entry={row}
                    selected={row.path === selected}
                    disabled={opening !== null && opening !== row.path}
                    opening={opening === row.path}
                    error={failure?.path === row.path ? failure.message : undefined}
                    onSelect={() => select(row.path)}
                    onOpen={(passphrase) => void openRow(row, passphrase)}
                  />
                ))}
              </ul>
            )}
            <div
              className={
                firstRun ? "hd-chooser__actions hd-chooser__actions--large" : "hd-chooser__actions"
              }
            >
              <Button
                variant={firstRun ? "primary" : "secondary"}
                icon="plus"
                className={actionClass}
                disabled={opening !== null}
                onClick={() => setCreating(true)}
              >
                Create a new database…
              </Button>
              <Button
                variant="secondary"
                icon="folder"
                className={actionClass}
                disabled={opening !== null}
                onClick={() => void openAnotherFile()}
              >
                Open another database file…
              </Button>
            </div>
            <CreateDatabaseDialog
              open={creating}
              onOpenChange={setCreating}
              suggested={state.suggested}
              onCreate={session.createDatabase}
            />
          </>
        )}
      </main>

      <p className="hd-sr-only" role="status" aria-live="polite">
        {announcement}
      </p>
    </div>
  );
}
