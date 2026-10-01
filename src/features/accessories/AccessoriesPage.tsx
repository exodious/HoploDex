import { useEffect, useMemo, useRef, useState } from "react";
import { Button, Checkbox, Icon, SegmentedControl } from "../../components";
import { formatDollars } from "../../lib/money";
import { CommandFailure } from "../../services/tauriClient";
import { useCollection } from "../app/collectionStore";
import { useNavigation } from "../app/navigation";
import { CoverageCell } from "../browse/CoverageCell";
import { FirearmThumbnail } from "../browse/FirearmThumbnail";
import { MountLines } from "../browse/MountLines";
import { GroupByMenu } from "../browse/GroupByMenu";
import type { GroupByOption } from "../browse/GroupByMenu";
import { SearchBar } from "../browse/SearchBar";
import { SEARCH_DEBOUNCE_MS, useDebounced, useSearchShortcut } from "../browse/searchHooks";
import { RecordName } from "../mounts/RecordName";
import { accessoryNameText, recordNameText } from "../mounts/recordNames";
import * as accessoriesService from "./accessoriesService";
import type { AccessoryGroup, AccessoryGroupBy, AccessorySummary } from "./types";
import "../browse/collection.css";
import "./accessories.css";

/** What the Accessories page remembers for the session, held by the app
 * shell as the collection page's is (contracts/ui-accessories.md §2). */
export interface AccessoryBrowseState {
  query: string;
  groupBy: AccessoryGroupBy | undefined;
  includeDisposed: boolean;
  view: "list" | "tile";
}

export interface AccessoriesPageProps {
  browse: AccessoryBrowseState;
  onBrowseChange: (next: AccessoryBrowseState) => void;
}

/** The grouping menu's choices (FR-017), in order. */
const GROUP_BY_OPTIONS: GroupByOption<AccessoryGroupBy>[] = [
  { value: "kind", label: "Kind" },
  { value: "make", label: "Make" },
  { value: "caliber", label: "Caliber" },
  { value: "cartridge", label: "Cartridge" },
  { value: "mounted_on", label: "Mounted on" },
];

/** An accessory's name (FR-005) as one string. */
function nameOf(accessory: AccessorySummary): string {
  return accessoryNameText(accessory.make, accessory.model, accessory.kindName);
}

/** Browse, search and group the accessories, as a list or tiles
 * (specs/006-accessory-links US1 and US4, FR-016 to FR-018;
 * contracts/ui-accessories.md §2), laid out as the collection page. */
export function AccessoriesPage({ browse, onBrowseChange }: AccessoriesPageProps) {
  const { accessories, loaded, revision } = useCollection();
  const { open, openDialog } = useNavigation();
  const [groups, setGroups] = useState<AccessoryGroup[] | null>(null);
  const [fetching, setFetching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  const query = useDebounced(browse.query.trim(), SEARCH_DEBOUNCE_MS);

  function update(patch: Partial<AccessoryBrowseState>) {
    onBrowseChange({ ...browse, ...patch });
  }

  useEffect(() => {
    let cancelled = false;
    setFetching(true);
    accessoriesService
      .listAccessories({
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
          setError(e instanceof CommandFailure ? e.message : "The accessories couldn't be loaded.");
        }
      })
      .finally(() => {
        if (!cancelled) setFetching(false);
      });
    return () => {
      cancelled = true;
    };
  }, [query, browse.groupBy, browse.includeDisposed, revision]);

  useSearchShortcut(searchRef);

  const activeCount = accessories.filter((a) => a.status === "active").length;
  const disposedCount = accessories.length - activeCount;
  const shown = useMemo(() => groups?.flatMap((group) => group.accessories) ?? [], [groups]);
  const grouped = browse.groupBy !== undefined;
  const openRecord = (id: number) => open({ page: "accessory", id, from: "accessories" });

  const addButton = (
    <Button variant="primary" icon="plus" onClick={() => openDialog("addAccessory")}>
      Add accessory
    </Button>
  );

  if (loaded && accessories.length === 0) {
    return (
      <>
        <PageHeader activeCount={0} disposedCount={0} />
        <div className="hd-empty">
          <p className="hd-empty__text">No accessories recorded yet.</p>
          <div className="hd-empty__actions">{addButton}</div>
        </div>
      </>
    );
  }

  return (
    <>
      <PageHeader activeCount={activeCount} disposedCount={disposedCount}>
        {addButton}
      </PageHeader>

      <div className="hd-toolbar" role="search">
        <SearchBar
          ref={searchRef}
          value={browse.query}
          onChange={(value) => update({ query: value })}
          busy={fetching}
          label="Search accessories"
          placeholder="Search accessories…"
        />
        <GroupByMenu<AccessoryGroupBy>
          value={browse.groupBy}
          onChange={(groupBy) => update({ groupBy })}
          options={GROUP_BY_OPTIONS}
          noneLabel="No grouping"
        />
        <div className="hd-toolbar__end">
          <Checkbox
            label="Show disposed"
            checked={browse.includeDisposed}
            onCheckedChange={(checked) => update({ includeDisposed: checked })}
          />
          <SegmentedControl<AccessoryBrowseState["view"]>
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
          {shown.length === 0
            ? "No accessories match"
            : `${shown.length} ${shown.length === 1 ? "accessory matches" : "accessories match"}`}{" "}
          “{query}”.{" "}
          <button type="button" className="hd-link" onClick={() => update({ query: "" })}>
            Clear search
          </button>
        </p>
      )}

      {groups && shown.length === 0 && (
        <div className="hd-empty hd-empty--compact">
          <p className="hd-empty__text">
            {query
              ? "Search looks through every field, including notes and serial numbers. Try a shorter or different term."
              : "No accessories to show."}
          </p>
          {!browse.includeDisposed && disposedCount > 0 && (
            <button
              type="button"
              className="hd-link"
              onClick={() => update({ includeDisposed: true })}
            >
              Include {disposedCount} disposed {disposedCount === 1 ? "accessory" : "accessories"}
            </button>
          )}
        </div>
      )}

      {shown.length > 0 &&
        (browse.view === "list" ? (
          <AccessoryList groups={groups ?? []} groupBy={browse.groupBy} onSelect={openRecord} />
        ) : (
          <AccessoryTiles groups={groups ?? []} grouped={grouped} onSelect={openRecord} />
        ))}
    </>
  );
}

function PageHeader({
  activeCount,
  disposedCount,
  children,
}: {
  activeCount: number;
  disposedCount: number;
  children?: React.ReactNode;
}) {
  return (
    <header className="hd-page-head">
      <div>
        <h1 className="hd-page-title">Accessories</h1>
        <p className="hd-page-sub">
          {activeCount + disposedCount === 0 ? null : (
            <>
              <strong className="hd-num">{activeCount}</strong>{" "}
              {activeCount === 1 ? "accessory" : "accessories"}
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
      {children}
    </header>
  );
}

interface LayoutProps {
  groups: AccessoryGroup[];
  onSelect: (id: number) => void;
}

/** A group's identity: a host's group by its record (two hosts can share a
 * name), any other by its heading. */
function groupKey(group: AccessoryGroup): string {
  return group.host ? `${group.host.record.kind}:${group.host.record.id}` : group.key;
}

/** The names that more than one group's host carries, so those groups say
 * which host they mean by its serial number (contracts/ui-accessories.md §2). */
function sharedHostNames(groups: AccessoryGroup[]): Set<string> {
  const seen = new Set<string>();
  const shared = new Set<string>();
  for (const { host } of groups) {
    if (!host) continue;
    const name = recordNameText(host);
    if (seen.has(name)) shared.add(name);
    seen.add(name);
  }
  return shared;
}

/** A group's heading and its count. Grouped by Mounted on, a host's heading is
 * its name as a link, with its serial number in muted text when another host
 * has the same name. */
function AccessoryGroupHeading({ group, shared }: { group: AccessoryGroup; shared: Set<string> }) {
  const { host } = group;
  return (
    <h2 className="hd-group__title">
      {host ? <RecordName label={host} link /> : group.key}
      {host?.serialNumber && shared.has(recordNameText(host)) && (
        <span className="hd-group__serial">
          <span className="hd-serial">{host.serialNumber}</span>
        </span>
      )}
      <span className="hd-group__count hd-num">{group.accessories.length}</span>
    </h2>
  );
}

function AccessoryList({
  groups,
  groupBy,
  onSelect,
}: LayoutProps & { groupBy: AccessoryGroupBy | undefined }) {
  const grouped = groupBy !== undefined;
  // A column repeating the group heading adds nothing: the host's name is the
  // heading when grouped by Mounted on.
  const showMountedOn = groupBy !== "mounted_on";
  const shared = sharedHostNames(groups);
  return (
    <div className="hd-browse">
      {groups
        .filter((group) => group.accessories.length > 0)
        .map((group, index) => (
          <section
            key={groupKey(group)}
            className="hd-group"
            aria-label={grouped ? group.key : undefined}
          >
            {grouped && <AccessoryGroupHeading group={group} shared={shared} />}
            <table className="hd-table hd-table--accessories">
              <thead className={index > 0 ? "hd-sr-only" : undefined}>
                <tr>
                  <th scope="col">Accessory</th>
                  {showMountedOn && <th scope="col">Mounted on</th>}
                  <th scope="col" className="hd-table__num">
                    Value
                  </th>
                  <th scope="col">Coverage</th>
                </tr>
              </thead>
              <tbody>
                {group.accessories.map((accessory) => (
                  <tr
                    key={accessory.id}
                    className={
                      accessory.status === "disposed" ? "hd-row hd-row--disposed" : "hd-row"
                    }
                    onClick={() => onSelect(accessory.id)}
                  >
                    <td>
                      <div className="hd-row__identity">
                        <FirearmThumbnail
                          className="hd-row__thumb"
                          thumbnailPhotoId={accessory.thumbnailPhotoId}
                          genericThumbnailKey={accessory.genericThumbnailKey}
                        />
                        <div className="hd-row__names">
                          <button
                            type="button"
                            className="hd-row__name"
                            onClick={(e) => {
                              e.stopPropagation();
                              onSelect(accessory.id);
                            }}
                          >
                            {nameOf(accessory)}
                          </button>
                          {accessory.serialNumber && (
                            <span className="hd-row__serial">
                              <span className="hd-serial">{accessory.serialNumber}</span>
                            </span>
                          )}
                        </div>
                      </div>
                    </td>
                    {showMountedOn && (
                      <td>
                        {/* FR-013: the direct host only. */}
                        {accessory.mountedOn ? (
                          <RecordName label={accessory.mountedOn} link />
                        ) : (
                          <>
                            <span aria-hidden>—</span>
                            <span className="hd-sr-only">Not mounted</span>
                          </>
                        )}
                      </td>
                    )}
                    <td className="hd-table__num hd-num">
                      {formatDollars(accessory.estimatedValue)}
                    </td>
                    <td>
                      <CoverageCell firearm={accessory} />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
        ))}
    </div>
  );
}

function AccessoryTiles({ groups, grouped, onSelect }: LayoutProps & { grouped: boolean }) {
  const shared = sharedHostNames(groups);
  return (
    <div className="hd-browse">
      {groups
        .filter((group) => group.accessories.length > 0)
        .map((group) => (
          <section
            key={groupKey(group)}
            className="hd-group"
            aria-label={grouped ? group.key : undefined}
          >
            {grouped && <AccessoryGroupHeading group={group} shared={shared} />}
            <ul className="hd-tiles">
              {group.accessories.map((accessory) => (
                <li key={accessory.id}>
                  {/* As the collection's tiles: the host is a link, which cannot
                      sit inside the tile's button. */}
                  <div className="hd-tile-card">
                    <button
                      type="button"
                      className={
                        accessory.status === "disposed" ? "hd-tile hd-tile--disposed" : "hd-tile"
                      }
                      onClick={() => onSelect(accessory.id)}
                    >
                      <FirearmThumbnail
                        className="hd-tile__image"
                        thumbnailPhotoId={accessory.thumbnailPhotoId}
                        genericThumbnailKey={accessory.genericThumbnailKey}
                      />
                      <span className="hd-tile__body">
                        <span className="hd-tile__name">{nameOf(accessory)}</span>
                        {accessory.serialNumber && (
                          <span className="hd-tile__meta">
                            <span className="hd-serial">{accessory.serialNumber}</span>
                          </span>
                        )}
                        <span className="hd-tile__foot">
                          <span className="hd-tile__value hd-num">
                            {formatDollars(accessory.estimatedValue)}
                          </span>
                          <CoverageCell firearm={accessory} />
                        </span>
                      </span>
                    </button>
                    {accessory.mountedOn && (
                      <div className="hd-tile__mounts">
                        <MountLines mountedOn={accessory.mountedOn} />
                      </div>
                    )}
                  </div>
                </li>
              ))}
            </ul>
          </section>
        ))}
    </div>
  );
}
