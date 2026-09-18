import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Button, Dialog, useToast } from "../../components";
import { CollectionPage } from "../browse/CollectionPage";
import type { BrowseState } from "../browse/types";
import { FirearmForm } from "../firearms/FirearmForm";
import { FirearmRecordPage } from "../firearms/FirearmRecordPage";
import * as firearmsService from "../firearms/firearmsService";
import type { FirearmInput } from "../firearms/types";
import { ExportDialog } from "../import-export/ExportDialog";
import { ImportDialog } from "../import-export/ImportDialog";
import { InsurancePage } from "../insurance/InsurancePage";
import { policyExpiry } from "../insurance/coverage";
import { firearmName, useCollection } from "./collectionStore";
import { NavigationContext } from "./navigation";
import type { Navigation, Route, ShellDialog } from "./navigation";
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

/** The app frame: top bar, the current page, and the dialogs reachable
 * from anywhere (add firearm, import, export). Browse state lives here so
 * a search, grouping, or scroll position survives opening a record and
 * coming back. */
export function AppShell() {
  const { firearms, policies, refresh } = useCollection();
  const notify = useToast();
  const [route, setRoute] = useState<Route>({ page: "collection" });
  const [browse, setBrowse] = useState<BrowseState>(initialBrowseState);
  const [dialog, setDialog] = useState<ShellDialog | null>(null);
  const scrollMemory = useRef<Partial<Record<Route["page"], number>>>({});

  const navigate = useCallback(
    (next: Route) => {
      scrollMemory.current[route.page] = window.scrollY;
      setRoute(next);
    },
    [route.page],
  );

  useLayoutEffect(() => {
    // Records always open at the top; list pages return to where they were.
    window.scrollTo(0, route.page === "firearm" ? 0 : (scrollMemory.current[route.page] ?? 0));
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

  const navigation = useMemo<Navigation>(
    () => ({ route, navigate, openDialog: setDialog }),
    [route, navigate],
  );

  const activeCount = firearms.filter((f) => f.status === "active").length;
  const attentionCount =
    firearms.filter((f) => f.status === "active" && f.insuranceWarning !== "none").length +
    policies.filter((p) => {
      const expiry = policyExpiry(p.effectiveEndDate);
      return expiry.expired || expiry.expiringSoon;
    }).length;

  async function handleCreate(input: FirearmInput) {
    const created = await firearmsService.createFirearm(input);
    setDialog(null);
    await refresh();
    notify(`Added ${firearmName(created)} to the collection.`);
    navigate({ page: "firearm", id: created.id, from: "collection" });
  }

  const section = route.page === "firearm" ? route.from : route.page;

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
              <Button variant="ghost" size="sm" icon="download" onClick={() => setDialog("import")}>
                Import
              </Button>
              <Button variant="ghost" size="sm" icon="upload" onClick={() => setDialog("export")}>
                Export
              </Button>
            </div>
          </div>
        </header>

        <main className="hd-main">
          {route.page === "collection" && (
            <CollectionPage browse={browse} onBrowseChange={setBrowse} />
          )}
          {route.page === "insurance" && <InsurancePage />}
          {route.page === "firearm" && (
            <FirearmRecordPage key={route.id} id={route.id} from={route.from} />
          )}
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

/** A hoplon — the round shield the name comes from — crossed by the same
 * dash-dot axis as the type drawings. */
function BrandMark() {
  return (
    <svg className="hd-brand__mark" viewBox="0 0 28 28" aria-hidden focusable={false}>
      <circle cx="14" cy="14" r="11.5" />
      <circle cx="14" cy="14" r="7" />
      <path d="M1 14h26" className="hd-brand__axis" />
    </svg>
  );
}
