import { useState } from "react";
import { Button, Dialog } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import { FirearmDetail } from "./FirearmDetail";
import { FirearmForm } from "./FirearmForm";
import * as firearmsService from "./firearmsService";
import type { Firearm } from "./types";

/**
 * Minimal top-level page wiring FirearmForm/FirearmDetail to the backend
 * commands for User Story 1. Browsing/searching/grouping the full
 * collection is User Story 2's `list_firearms`-backed BrowsePage; this page
 * only needs enough navigation to add and revisit records independently.
 */
export function FirearmsPage() {
  const [firearms, setFirearms] = useState<Firearm[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selected = firearms.find((f) => f.id === selectedId) ?? null;

  function replaceFirearm(updated: Firearm) {
    setFirearms((prev) => prev.map((f) => (f.id === updated.id ? updated : f)));
  }

  async function handleCreate(input: Parameters<typeof firearmsService.createFirearm>[0]) {
    const created = await firearmsService.createFirearm(input);
    setFirearms((prev) => [...prev, created]);
    setCreating(false);
    setSelectedId(created.id);
  }

  async function handleUpdate(input: Parameters<typeof firearmsService.updateFirearm>[1]) {
    if (!selected) return;
    const updated = await firearmsService.updateFirearm(selected.id, input);
    replaceFirearm(updated);
  }

  async function handleDispose(input: Parameters<typeof firearmsService.disposeFirearm>[1]) {
    if (!selected) return;
    const updated = await firearmsService.disposeFirearm(selected.id, input);
    replaceFirearm(updated);
  }

  async function handleDelete() {
    if (!selected) return;
    try {
      await firearmsService.deleteFirearm(selected.id, true);
      setFirearms((prev) => prev.filter((f) => f.id !== selected.id));
      setSelectedId(null);
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "Failed to delete this firearm.");
    }
  }

  return (
    <div>
      <header className="hd-dialog__actions" style={{ justifyContent: "space-between" }}>
        <h1>Collection</h1>
        <Button variant="primary" onClick={() => setCreating(true)}>
          Add firearm
        </Button>
      </header>

      {error && (
        <p className="hd-field__error" role="alert">
          {error}
        </p>
      )}

      {selected ? (
        <>
          <Button variant="secondary" onClick={() => setSelectedId(null)}>
            ← Back to collection
          </Button>
          <FirearmDetail
            firearm={selected}
            onUpdate={handleUpdate}
            onDispose={handleDispose}
            onDelete={handleDelete}
          />
        </>
      ) : (
        <ul>
          {firearms.map((firearm) => (
            <li key={firearm.id}>
              <button type="button" onClick={() => setSelectedId(firearm.id)}>
                {firearm.make} {firearm.model}
                {firearm.status === "disposed" ? " (disposed)" : ""}
              </button>
            </li>
          ))}
          {firearms.length === 0 && <p>No firearms yet. Add your first one to get started.</p>}
        </ul>
      )}

      <Dialog open={creating} onOpenChange={setCreating} title="Add firearm">
        <FirearmForm onCancel={() => setCreating(false)} onSubmit={handleCreate} />
      </Dialog>
    </div>
  );
}
