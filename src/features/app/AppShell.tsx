import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Button, Dialog, useToast } from "../../components";
import { CollectionPage } from "../browse/CollectionPage";
import { DatabaseMenu } from "../databases/DatabaseMenu";
import { DatabaseNotes } from "../databases/DatabaseNotes";
import type { BrowseState } from "../browse/types";
import { FirearmForm } from "../firearms/FirearmForm";
import { FirearmRecordPage } from "../firearms/FirearmRecordPage";
import * as firearmsService from "../firearms/firearmsService";
import type { FirearmInput } from "../firearms/types";
import { ExportDialog } from "../import-export/ExportDialog";
import { ImportDialog } from "../import-export/ImportDialog";
import { InsurancePage } from "../insurance/InsurancePage";
import { PolicyPage } from "../insurance/PolicyPage";
import { BrandMark } from "./BrandMark";
import { firearmName, useCollection } from "./collectionStore";
import { NavigationContext } from "./navigation";
import { ThemeToggle } from "./ThemeToggle";
import { peekResumedDraft } from "../session/usePendingDraft";
import type { Draft } from "../databases/types";
import type { BackTarget, Navigation, Route, ShellDialog } from "./navigation";
import "./AppShell.css";

const VIEW_PREFERENCE_KEY = "hoplodex.browseView";

function initialBrowseState(): BrowseState {
  let view: BrowseState["view"] = "list";
  try {
    if (localStorage.getItem(VIEW_PREFERENCE_KEY) === "tile") view = "tile";
  } catch {
    // storage unavailable — default to list
  }
  return { query: "", groupBy: undefined, includeDisposed: false, view };
}

/** A page left behind by following a link, with how far it was scrolled. */
interface Visit {
  route: Route;
  scrollY: number;
}

/** The app frame: top bar, the current page, and the dialogs reachable
 * from anywhere (add firearm, import, export). Browse state lives here so
 * a search, grouping, or scroll position survives opening a record and
 * coming back. Following a link (a policy from a firearm, a firearm from a
 * policy) pushes onto a trail, so "back" retraces the path taken. */
/** Where the form of resumed pending changes is: the firearm's record, or
 * the policy's page, whose own dialogs then open with them. */
function resumedRoute(resumed: Draft | null): Route {
  if (resumed?.kind === "firearm" && resumed.targetId != null)
    return { page: "firearm", id: resumed.targetId, from: "collection" };
  if (resumed?.kind === "policy")
    return resumed.targetId != null
      ? { page: "policy", id: resumed.targetId }
      : { page: "insurance" };
  return { page: "collection" };
}

export function AppShell() {
  const { firearmsById, firearms, policiesById, refresh } = useCollection();
  const notify = useToast();
  // Resumed pending changes open where their form is (FR-039).
  const [route, setRoute] = useState<Route>(() => resumedRoute(peekResumedDraft()));
  const [trail, setTrail] = useState<Visit[]>([]);
  const [browse, setBrowse] = useState<BrowseState>(initialBrowseState);
  const [dialog, setDialog] = useState<ShellDialog | null>(() => {
    const resumed = peekResumedDraft();
    return resumed?.kind === "firearm" && resumed.mode === "add" ? "addFirearm" : null;
  });
  const scrollMemory = useRef<Partial<Record<Route["page"], number>>>({});
  // Set when going back, to restore the scroll position of the page returned to.
  const restore = useRef<{ route: Route; scrollY: number } | null>(null);

  const navigate = useCallback(
    (next: Route) => {
      scrollMemory.current[route.page] = window.scrollY;
      setTrail([]);
      setRoute(next);
    },
    [route.page],
  );

  const open = useCallback(
    (next: Route) => {
      setTrail((visits) => [...visits, { route, scrollY: window.scrollY }]);
      setRoute(next);
    },
    [route],
  );

  const back = useMemo<BackTarget | null>(() => {
    const labelFor = (target: Route): string => {
      switch (target.page) {
        case "collection":
          return "Collection";
        case "insurance":
          return "Insurance";
        case "policy":
          return policiesById.get(target.id)?.name ?? "Policy";
        case "firearm": {
          const firearm = firearmsById.get(target.id);
          return firearm ? firearmName(firearm) : "Firearm";
        }
      }
    };
    const previous = trail[trail.length - 1];
    if (previous) {
      return {
        label: labelFor(previous.route),
        go: () => {
          restore.current = { route: previous.route, scrollY: previous.scrollY };
          setTrail((visits) => visits.slice(0, -1));
          setRoute(previous.route);
        },
      };
    }
    // A record reached without a trail returns to the list it belongs to.
    if (route.page === "firearm") {
      const list: Route = { page: route.from };
      return { label: labelFor(list), go: () => navigate(list) };
    }
    return null;
  }, [trail, route, firearmsById, policiesById, navigate]);

  useLayoutEffect(() => {
    if (restore.current?.route === route) {
      window.scrollTo(0, restore.current.scrollY);
      return;
    }
    // Records always open at the top; list pages return to where they were.
    const isRecord = route.page === "firearm" || route.page === "policy";
    window.scrollTo(0, isRecord ? 0 : (scrollMemory.current[route.page] ?? 0));
  }, [route]);

  useEffect(() => {
    try {
      localStorage.setItem(VIEW_PREFERENCE_KEY, browse.view);
    } catch {
      // storage unavailable — the preference just won't persist
    }
  }, [browse.view]);

  // Ctrl/⌘+N adds a firearm from anywhere, unless a dialog is already open.
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "n") {
        if (document.querySelector('[role="dialog"], [role="alertdialog"]')) return;
        event.preventDefault();
        setDialog("addFirearm");
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  // Escape goes back wherever a back link shows (FR-040), unless it's closing a
  // dialog or popover or the user is typing in a field.
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape" || event.defaultPrevented || !back) return;
      if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return;
      const target = event.target instanceof Element ? event.target : null;
      if (target?.closest("input, textarea, select, [contenteditable]")) return;
      if (
        document.querySelector(
          '[role="dialog"], [role="alertdialog"], [data-radix-popper-content-wrapper]',
        )
      )
        return;
      back.go();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [back]);

  const navigation = useMemo<Navigation>(
    () => ({ route, navigate, open, back, openDialog: setDialog }),
    [route, navigate, open, back],
  );

  const activeCount = firearms.filter((f) => f.status === "active").length;
  // Only firearms are counted: lapsing policies are called out on the
  // Insurance page itself, and mixing them in made the number keep
  // counting a policy after the firearm that prompted it was gone.
  const attentionCount = firearms.filter(
    (f) => f.status === "active" && f.insuranceWarning !== "none",
  ).length;

  async function handleCreate(input: FirearmInput, confirmedWarnings?: boolean) {
    const created = await firearmsService.createFirearm(input, confirmedWarnings);
    setDialog(null);
    await refresh();
    notify(`Added ${firearmName(created)} to the collection.`);
    open({ page: "firearm", id: created.id, from: "collection" });
  }

  const section =
    route.page === "firearm" ? route.from : route.page === "policy" ? "insurance" : route.page;

  return (
    <NavigationContext.Provider value={navigation}>
      <div className="hd-app">
        <header className="hd-topbar">
          <div className="hd-topbar__inner">
            <button
              type="button"
              className="hd-brand"
              onClick={() => navigate({ page: "collection" })}
              aria-label="HoploDex — go to the collection"
            >
              <BrandMark />
              <span className="hd-brand__name">HoploDex</span>
            </button>

            <nav className="hd-tabs" aria-label="Sections">
              <button
                type="button"
                className="hd-tab"
                aria-current={section === "collection" ? "page" : undefined}
                onClick={() => navigate({ page: "collection" })}
              >
                Collection
                <span className="hd-tab__count hd-num">{activeCount}</span>
              </button>
              <button
                type="button"
                className="hd-tab"
                aria-current={section === "insurance" ? "page" : undefined}
                onClick={() => navigate({ page: "insurance" })}
              >
                Insurance
                {attentionCount > 0 && (
                  <span className="hd-tab__alert hd-num">
                    <span className="hd-sr-only">, </span>
                    {attentionCount}
                    <span className="hd-sr-only"> need attention</span>
                  </span>
                )}
              </button>
            </nav>

            <div className="hd-topbar__tools">
              <DatabaseMenu />
              <Button variant="ghost" size="sm" icon="download" onClick={() => setDialog("import")}>
                Import
              </Button>
              <Button variant="ghost" size="sm" icon="upload" onClick={() => setDialog("export")}>
                Export
              </Button>
              <ThemeToggle />
            </div>
          </div>
        </header>

        <main className="hd-main">
          {route.page === "collection" && (
            <>
              <DatabaseNotes />
              <CollectionPage browse={browse} onBrowseChange={setBrowse} />
            </>
          )}
          {route.page === "insurance" && <InsurancePage />}
          {route.page === "policy" && <PolicyPage key={route.id} id={route.id} />}
          {route.page === "firearm" && <FirearmRecordPage key={route.id} id={route.id} />}
        </main>

        <Dialog
          open={dialog === "addFirearm"}
          onOpenChange={(open) => !open && setDialog(null)}
          title="Add firearm"
          description="Only the identifying details are required. Everything else can be filled in later."
          size="lg"
          bare
        >
          <FirearmForm onSubmit={handleCreate} onCancel={() => setDialog(null)} />
        </Dialog>

        <ExportDialog
          open={dialog === "export"}
          onOpenChange={(open) => !open && setDialog(null)}
          browse={browse}
        />
        <ImportDialog
          open={dialog === "import"}
          onOpenChange={(open) => !open && setDialog(null)}
        />
      </div>
    </NavigationContext.Provider>
  );
}
