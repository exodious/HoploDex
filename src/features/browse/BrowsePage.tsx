import { useEffect, useState } from "react";
import { Button } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import { BrowseList } from "./BrowseList";
import { BrowseTiles } from "./BrowseTiles";
import * as browseService from "./browseService";
import { GroupByControl } from "./GroupByControl";
import { SearchBar } from "./SearchBar";
import type { FirearmGroup, GroupBy } from "./types";

export interface BrowsePageProps {
  onSelectFirearm: (id: number) => void;
}

/** Browse/search/group the collection (User Story 2): wires SearchBar,
 * GroupByControl, and the list/tile view toggle to `list_firearms`. */
export function BrowsePage({ onSelectFirearm }: BrowsePageProps) {
  const [query, setQuery] = useState("");
  const [groupBy, setGroupBy] = useState<GroupBy | undefined>(undefined);
  const [view, setView] = useState<"list" | "tile">("list");
  const [groups, setGroups] = useState<FirearmGroup[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    browseService
      .listFirearms({ query: query.trim() || undefined, groupBy, view })
      .then((result) => {
        if (!cancelled) setGroups(result.groups);
      })
      .catch((e) => {
        if (!cancelled) {
          setError(e instanceof CommandFailure ? e.message : "Failed to load the collection.");
        }
      });
    return () => {
      cancelled = true;
    };
  }, [query, groupBy, view]);

  return (
    <div>
      <div style={{ display: "flex", gap: 16, alignItems: "flex-end", flexWrap: "wrap" }}>
        <SearchBar value={query} onChange={setQuery} />
        <GroupByControl value={groupBy} onChange={setGroupBy} />
        <div className="hd-dialog__actions" role="group" aria-label="View">
          <Button
            variant={view === "list" ? "primary" : "secondary"}
            onClick={() => setView("list")}
            aria-pressed={view === "list"}
          >
            List
          </Button>
          <Button
            variant={view === "tile" ? "primary" : "secondary"}
            onClick={() => setView("tile")}
            aria-pressed={view === "tile"}
          >
            Tiles
          </Button>
        </div>
      </div>

      {error && (
        <p className="hd-field__error" role="alert">
          {error}
        </p>
      )}

      {view === "list" ? (
        <BrowseList groups={groups} onSelect={onSelectFirearm} />
      ) : (
        <BrowseTiles groups={groups} onSelect={onSelectFirearm} />
      )}
    </div>
  );
}
