import { useEffect, useState } from "react";
import { Button, Checkbox, Icon, SegmentedControl } from "../../components";
import { formatDollars } from "../../lib/money";
import { CommandFailure } from "../../services/tauriClient";
import { useCollection } from "../app/collectionStore";
import { useNavigation } from "../app/navigation";
import { CoverageCell } from "../browse/CoverageCell";
import { FirearmThumbnail } from "../browse/FirearmThumbnail";
import { accessoryNameText } from "../mounts/recordNames";
import * as accessoriesService from "./accessoriesService";
import type { AccessoryGroup, AccessorySummary } from "./types";
import "../browse/collection.css";
import "./accessories.css";

/** What the Accessories page remembers for the session, held by the app
 * shell as the collection page's is (contracts/ui-accessories.md §2).
 * Search and grouping arrive with User Story 4. */
export interface AccessoryBrowseState {
  query: string;
  groupBy: undefined;
  includeDisposed: boolean;
  view: "list" | "tile";
}

export interface AccessoriesPageProps {
  browse: AccessoryBrowseState;
  onBrowseChange: (next: AccessoryBrowseState) => void;
}

/** An accessory's name (FR-005) as one string. */
function nameOf(accessory: AccessorySummary): string {
  return accessoryNameText(accessory.make, accessory.model, accessory.kindName);
}

/** Browse the accessories as a list or tiles (specs/006-accessory-links US1,
 * FR-016; contracts/ui-accessories.md §2), laid out as the collection page. */
export function AccessoriesPage({ browse, onBrowseChange }: AccessoriesPageProps) {
  const { accessories, loaded, revision } = useCollection();
  const { open, openDialog } = useNavigation();
  const [groups, setGroups] = useState<AccessoryGroup[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    accessoriesService
      .listAccessories({ includeDisposed: browse.includeDisposed })
      .then((result) => {
        if (cancelled) return;
        setGroups(result.groups);
        setError(null);
      })
      .catch((e) => {
        if (!cancelled) {
          setError(e instanceof CommandFailure ? e.message : "The accessories couldn't be loaded.");
        }
      });
    return () => {
      cancelled = true;
    };
  }, [browse.includeDisposed, revision]);

  const activeCount = accessories.filter((a) => a.status === "active").length;
  const disposedCount = accessories.length - activeCount;
  const shown = groups?.flatMap((group) => group.accessories) ?? [];
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

      <div className="hd-toolbar">
        <div className="hd-toolbar__end">
          <Checkbox
            label="Show disposed"
            checked={browse.includeDisposed}
            onCheckedChange={(checked) => onBrowseChange({ ...browse, includeDisposed: checked })}
          />
          <SegmentedControl<AccessoryBrowseState["view"]>
            label="View"
            hideLabel
            size="sm"
            value={browse.view}
            onChange={(view) => onBrowseChange({ ...browse, view })}
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

      {groups && shown.length === 0 && (
        <div className="hd-empty hd-empty--compact">
          <p className="hd-empty__text">No accessories to show.</p>
          {!browse.includeDisposed && disposedCount > 0 && (
            <button
              type="button"
              className="hd-link"
              onClick={() => onBrowseChange({ ...browse, includeDisposed: true })}
            >
              Include {disposedCount} disposed {disposedCount === 1 ? "accessory" : "accessories"}
            </button>
          )}
        </div>
      )}

      {shown.length > 0 &&
        (browse.view === "list" ? (
          <AccessoryList groups={groups ?? []} onSelect={openRecord} />
        ) : (
          <AccessoryTiles groups={groups ?? []} onSelect={openRecord} />
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

function AccessoryList({ groups, onSelect }: LayoutProps) {
  return (
    <div className="hd-browse">
      {groups
        .filter((group) => group.accessories.length > 0)
        .map((group, index) => (
          <section key={group.key} className="hd-group">
            <table className="hd-table">
              <thead className={index > 0 ? "hd-sr-only" : undefined}>
                <tr>
                  <th scope="col">Accessory</th>
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

function AccessoryTiles({ groups, onSelect }: LayoutProps) {
  return (
    <div className="hd-browse">
      {groups
        .filter((group) => group.accessories.length > 0)
        .map((group) => (
          <section key={group.key} className="hd-group">
            <ul className="hd-tiles">
              {group.accessories.map((accessory) => (
                <li key={accessory.id}>
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
                </li>
              ))}
            </ul>
          </section>
        ))}
    </div>
  );
}
