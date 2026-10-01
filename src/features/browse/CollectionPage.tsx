import { useEffect, useMemo, useRef, useState } from "react";
import { Button, Checkbox, Icon, SegmentedControl } from "../../components";
import { daysUntil } from "../../lib/dates";
import { formatDollars } from "../../lib/money";
import { CommandFailure } from "../../services/tauriClient";
import { useCollection } from "../app/collectionStore";
import { useNavigation } from "../app/navigation";
import { BrowseList } from "./BrowseList";
import { BrowseTiles } from "./BrowseTiles";
import * as browseService from "./browseService";
import { GroupByMenu } from "./GroupByMenu";
import { SearchBar } from "./SearchBar";
import { SEARCH_DEBOUNCE_MS, useDebounced, useSearchShortcut } from "./searchHooks";
import { GROUP_BY_OPTIONS, GROUP_BY_SECTIONS } from "./types";
import type { BrowseState, FirearmGroup, VisibleGroup } from "./types";
import "./collection.css";

/** Rows rendered per step; more mount as the end of the list nears, so a
 * 10,000-firearm collection stays responsive (constitution Principle IV). */
const RENDER_STEP = 150;

export interface CollectionPageProps {
  browse: BrowseState;
  onBrowseChange: (next: BrowseState) => void;
}

/** Browse, search, and group the collection (User Story 2). */
export function CollectionPage({ browse, onBrowseChange }: CollectionPageProps) {
  const { firearms, summary, loaded, revision } = useCollection();
  const { open, openDialog } = useNavigation();
  const [groups, setGroups] = useState<FirearmGroup[] | null>(null);
  const [fetching, setFetching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [renderLimit, setRenderLimit] = useState(RENDER_STEP);
  const searchRef = useRef<HTMLInputElement>(null);
  const sentinelRef = useRef<HTMLDivElement>(null);

  const query = useDebounced(browse.query.trim(), SEARCH_DEBOUNCE_MS);

  function update(patch: Partial<BrowseState>) {
    onBrowseChange({ ...browse, ...patch });
  }

  useEffect(() => {
    let cancelled = false;
    setFetching(true);
    browseService
      .listFirearms({
        query: query || undefined,
        groupBy: browse.groupBy,
        includeDisposed: browse.includeDisposed,
      })
      .then((result) => {
        if (cancelled) return;
        setGroups(result.groups);
        setError(null);
      })
      .catch((e) => {
        if (!cancelled) {
          setError(e instanceof CommandFailure ? e.message : "The collection couldn't be loaded.");
        }
      })
      .finally(() => {
        if (!cancelled) setFetching(false);
      });
    return () => {
      cancelled = true;
    };
  }, [query, browse.groupBy, browse.includeDisposed, revision]);

  useEffect(() => {
    setRenderLimit(RENDER_STEP);
  }, [query, browse.groupBy, browse.includeDisposed]);

  useSearchShortcut(searchRef);

  const total = groups?.reduce((n, g) => n + g.firearms.length, 0) ?? 0;
  const visibleGroups = useMemo(
    () => truncateGroups(groups ?? [], renderLimit),
    [groups, renderLimit],
  );

  // Mount the next batch of rows as the end of the list scrolls into view.
  useEffect(() => {
    const sentinel = sentinelRef.current;
    if (!sentinel || renderLimit >= total || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          setRenderLimit((limit) => limit + RENDER_STEP);
        }
      },
      { rootMargin: "600px" },
    );
    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [renderLimit, total]);

  const activeCount = firearms.filter((f) => f.status === "active").length;
  const disposedCount = firearms.length - activeCount;
  const grouped = browse.groupBy !== undefined;
  const openRecord = (id: number) => open({ page: "firearm", id, from: "collection" });

  if (loaded && firearms.length === 0) {
    return (
      <>
        <PageHeader activeCount={0} valueDollars={0} disposedCount={0} />
        <div className="hd-empty">
          <h2 className="hd-empty__title">Start the record</h2>
          <p className="hd-empty__text">
            Add each firearm with its make, model, and serial number, then fill in value, insurance,
            photos, and receipts as you go. Everything stays on this computer, in an encrypted
            database.
          </p>
          <div className="hd-empty__actions">
            <Button variant="primary" icon="plus" onClick={() => openDialog("addFirearm")}>
              Add firearm
            </Button>
            <Button variant="secondary" icon="download" onClick={() => openDialog("import")}>
              Import a spreadsheet
            </Button>
          </div>
        </div>
      </>
    );
  }

  return (
    <>
      <PageHeader
        activeCount={activeCount}
        valueDollars={summary?.collectionTotal ?? 0}
        disposedCount={disposedCount}
      />

      <AttentionBanner />

      <div className="hd-toolbar" role="search">
        <SearchBar
          ref={searchRef}
          value={browse.query}
          onChange={(value) => update({ query: value })}
          busy={fetching}
        />
        <GroupByMenu
          value={browse.groupBy}
          onChange={(groupBy) => update({ groupBy })}
          options={GROUP_BY_OPTIONS}
          sections={GROUP_BY_SECTIONS}
        />
        <div className="hd-toolbar__end">
          <Checkbox
            label="Show disposed"
            checked={browse.includeDisposed}
            onCheckedChange={(checked) => update({ includeDisposed: checked })}
          />
          <SegmentedControl<BrowseState["view"]>
            label="View"
            hideLabel
            size="sm"
            value={browse.view}
            onChange={(view) => update({ view })}
            options={[
              { value: "list", label: "List", icon: "list" },
              { value: "tile", label: "Tiles", icon: "tiles" },
            ]}
          />
        </div>
      </div>

      {error && (
        <p className="hd-banner hd-banner--error" role="alert">
          <Icon name="alert" />
          <span className="hd-banner__text">{error}</span>
        </p>
      )}

      {query && groups && (
        <p className="hd-results-note" aria-live="polite">
          {total === 0
            ? "No firearms match"
            : `${total} ${total === 1 ? "firearm matches" : "firearms match"}`}{" "}
          “{query}”.{" "}
          <button type="button" className="hd-link" onClick={() => update({ query: "" })}>
            Clear search
          </button>
        </p>
      )}

      {groups && total === 0 && (
        <div className="hd-empty hd-empty--compact">
          <p className="hd-empty__text">
            {query
              ? "Search looks through every field, including notes and accessories. Try a shorter or different term."
              : "No firearms to show."}
          </p>
          {!browse.includeDisposed && disposedCount > 0 && (
            <button
              type="button"
              className="hd-link"
              onClick={() => update({ includeDisposed: true })}
            >
              Include {disposedCount} disposed {disposedCount === 1 ? "firearm" : "firearms"}
            </button>
          )}
        </div>
      )}

      {total > 0 &&
        (browse.view === "list" ? (
          <BrowseList groups={visibleGroups} groupBy={browse.groupBy} onSelect={openRecord} />
        ) : (
          <BrowseTiles groups={visibleGroups} grouped={grouped} onSelect={openRecord} />
        ))}
      {renderLimit < total && <div ref={sentinelRef} className="hd-browse__more" />}
    </>
  );
}

function PageHeader({
  activeCount,
  valueDollars,
  disposedCount,
}: {
  activeCount: number;
  valueDollars: number;
  disposedCount: number;
}) {
  const { openDialog } = useNavigation();
  return (
    <header className="hd-page-head">
      <div>
        <h1 className="hd-page-title">Collection</h1>
        <p className="hd-page-sub">
          {activeCount + disposedCount === 0 ? (
            "No firearms recorded yet."
          ) : (
            <>
              <strong className="hd-num">{activeCount}</strong>{" "}
              {activeCount === 1 ? "firearm" : "firearms"} ·{" "}
              <strong className="hd-num">{formatDollars(valueDollars)}</strong> estimated
              replacement value
              {disposedCount > 0 && (
                <>
                  {" "}
                  · <span className="hd-num">{disposedCount}</span> disposed
                </>
              )}
            </>
          )}
        </p>
      </div>
      {activeCount + disposedCount > 0 && (
        <Button
          variant="primary"
          icon="plus"
          onClick={() => openDialog("addFirearm")}
          title="Add firearm (Ctrl+N)"
          aria-keyshortcuts="Control+N"
        >
          Add firearm
        </Button>
      )}
    </header>
  );
}

/** Summarizes what needs attention on the Insurance page, so problems are
 * visible from the collection without opening it. */
function AttentionBanner() {
  const { firearms, policies } = useCollection();
  const { open } = useNavigation();

  const flagged = firearms.filter((f) => f.status === "active" && f.insuranceWarning !== "none");
  // The backend's warning flags, which already leave out a blanket policy
  // that has been renewed or replaced (FR-028).
  const lapsing = policies.filter((policy) => policy.expiredWarning || policy.expiringWarning);
  if (flagged.length === 0 && lapsing.length === 0) return null;

  const parts: string[] = [];
  if (flagged.length > 0) {
    parts.push(
      `${flagged.length} ${flagged.length === 1 ? "firearm is" : "firearms are"} uninsured or under-insured`,
    );
  }
  for (const policy of lapsing) {
    const daysLeft = daysUntil(policy.effectiveEndDate);
    parts.push(
      policy.expiredWarning
        ? `“${policy.name}” has expired`
        : `“${policy.name}” expires in ${daysLeft} ${daysLeft === 1 ? "day" : "days"}`,
    );
  }

  return (
    <div className="hd-banner hd-attention">
      <Icon name="alert" />
      <p className="hd-banner__text">{sentence(parts)}.</p>
      <Button variant="secondary" size="sm" onClick={() => open({ page: "insurance" })}>
        Review insurance
      </Button>
    </div>
  );
}

function sentence(parts: string[]): string {
  const text =
    parts.length <= 2
      ? parts.join(" and ")
      : `${parts.slice(0, -1).join(", ")}, and ${parts[parts.length - 1]}`;
  return text.charAt(0).toUpperCase() + text.slice(1);
}

function truncateGroups(groups: FirearmGroup[], limit: number): VisibleGroup[] {
  const out: VisibleGroup[] = [];
  let remaining = limit;
  for (const group of groups) {
    if (remaining <= 0) break;
    out.push({
      ...group,
      firearms: group.firearms.slice(0, remaining),
      total: group.firearms.length,
    });
    remaining -= group.firearms.length;
  }
  return out;
}
