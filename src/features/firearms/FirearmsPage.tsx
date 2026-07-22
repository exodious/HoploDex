import { useEffect, useState } from "react";
import { Button, Dialog } from "../../components";
import { BrowsePage } from "../browse/BrowsePage";
import * as browseService from "../browse/browseService";
import { ExportWizard } from "../import-export/ExportWizard";
import { ImportWizard } from "../import-export/ImportWizard";
import * as insuranceService from "../insurance/insuranceService";
import { InsurancePolicyForm } from "../insurance/InsurancePolicyForm";
import { ValueSummaryPanel } from "../insurance/ValueSummaryPanel";
import type { ValueSummary } from "../insurance/types";
import { CommandFailure } from "../../services/tauriClient";
import { FirearmDetail } from "./FirearmDetail";
import { FirearmForm } from "./FirearmForm";
import * as firearmsService from "./firearmsService";
import type { Firearm } from "./types";

/**
 * Top-level page wiring FirearmForm/FirearmDetail (User Story 1) to
 * BrowsePage's search/group/list-tile browsing (User Story 2) and the
 * insurance/valuation features (User Story 3). `browseKey` is bumped after
 * every mutating action to force BrowsePage to remount and re-fetch
 * `list_firearms`, since it owns its own query/group/view state.
 * `get_value_summary` is re-invoked after every mutating command so
 * ValueSummaryPanel is never stale (FR-015, SC-003) — there is no separate
 * "refresh" action.
 */
export function FirearmsPage() {
  const [selected, setSelected] = useState<Firearm | null>(null);
  const [creating, setCreating] = useState(false);
  const [addingPolicy, setAddingPolicy] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [browseKey, setBrowseKey] = useState(0);
  const [valueSummary, setValueSummary] = useState<ValueSummary | null>(null);
  const [firearmLabels, setFirearmLabels] = useState<Record<number, string>>({});

  function refreshBrowse() {
    setBrowseKey((k) => k + 1);
  }

  async function refreshValueSummary() {
    try {
      const [summary, listing] = await Promise.all([
        insuranceService.getValueSummary(),
        browseService.listFirearms({ includeDisposed: true }),
      ]);
      setValueSummary(summary);
      const labels: Record<number, string> = {};
      for (const group of listing.groups) {
        for (const firearm of group.firearms) {
          labels[firearm.id] = `${firearm.make} ${firearm.model}`;
        }
      }
      setFirearmLabels(labels);
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "Failed to load the value summary.");
    }
  }

  useEffect(() => {
    void refreshValueSummary();
  }, []);

  async function handleSelect(id: number) {
    setError(null);
    try {
      const firearm = await firearmsService.getFirearm(id);
      setSelected(firearm);
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "Failed to load this firearm.");
    }
  }

  async function handleCreate(input: Parameters<typeof firearmsService.createFirearm>[0]) {
    const created = await firearmsService.createFirearm(input);
    setCreating(false);
    setSelected(created);
    refreshBrowse();
    await refreshValueSummary();
  }

  async function handleUpdate(input: Parameters<typeof firearmsService.updateFirearm>[1]) {
    if (!selected) return;
    const updated = await firearmsService.updateFirearm(selected.id, input);
    setSelected(updated);
    refreshBrowse();
    await refreshValueSummary();
  }

  async function handleDispose(input: Parameters<typeof firearmsService.disposeFirearm>[1]) {
    if (!selected) return;
    const updated = await firearmsService.disposeFirearm(selected.id, input);
    setSelected(updated);
    refreshBrowse();
    await refreshValueSummary();
  }

  async function handleDelete() {
    if (!selected) return;
    try {
      await firearmsService.deleteFirearm(selected.id, true);
      setSelected(null);
      refreshBrowse();
      await refreshValueSummary();
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "Failed to delete this firearm.");
    }
  }

  async function handleFirearmUpdated(updated: Firearm) {
    setSelected(updated);
    refreshBrowse();
    await refreshValueSummary();
  }

  async function handleCreatePolicy(
    input: Parameters<typeof insuranceService.createInsurancePolicy>[0],
  ) {
    await insuranceService.createInsurancePolicy(input);
    setAddingPolicy(false);
    await refreshValueSummary();
  }

  return (
    <div>
      <header className="hd-dialog__actions" style={{ justifyContent: "space-between" }}>
        <h1>Collection</h1>
        <div className="hd-dialog__actions">
          <Button variant="secondary" onClick={() => setImporting(true)}>
            Import
          </Button>
          <Button variant="secondary" onClick={() => setExporting(true)}>
            Export
          </Button>
          <Button variant="secondary" onClick={() => setAddingPolicy(true)}>
            Add insurance policy
          </Button>
          <Button variant="primary" onClick={() => setCreating(true)}>
            Add firearm
          </Button>
        </div>
      </header>

      {error && (
        <p className="hd-field__error" role="alert">
          {error}
        </p>
      )}

      {valueSummary && <ValueSummaryPanel summary={valueSummary} firearmLabels={firearmLabels} />}

      {selected ? (
        <>
          <Button variant="secondary" onClick={() => setSelected(null)}>
            ← Back to collection
          </Button>
          <FirearmDetail
            firearm={selected}
            onUpdate={handleUpdate}
            onDispose={handleDispose}
            onDelete={handleDelete}
            onFirearmUpdated={handleFirearmUpdated}
          />
        </>
      ) : (
        <BrowsePage key={browseKey} onSelectFirearm={handleSelect} />
      )}

      <Dialog open={creating} onOpenChange={setCreating} title="Add firearm">
        <FirearmForm onCancel={() => setCreating(false)} onSubmit={handleCreate} />
      </Dialog>

      <Dialog open={addingPolicy} onOpenChange={setAddingPolicy} title="Add insurance policy">
        <InsurancePolicyForm
          onCancel={() => setAddingPolicy(false)}
          onSubmit={handleCreatePolicy}
        />
      </Dialog>

      <Dialog open={exporting} onOpenChange={setExporting} title="Export collection">
        <ExportWizard hasActiveFilter={false} onClose={() => setExporting(false)} />
      </Dialog>

      <Dialog open={importing} onOpenChange={setImporting} title="Import collection">
        <ImportWizard
          onClose={() => setImporting(false)}
          onImported={() => {
            refreshBrowse();
            void refreshValueSummary();
          }}
        />
      </Dialog>
    </div>
  );
}
