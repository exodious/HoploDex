import { useState } from "react";
import { Button, ConfirmDialog, Dialog, Select, TextField } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import { FirearmForm } from "./FirearmForm";
import { DISPOSITION_TYPE_OPTIONS, FIREARM_TYPE_OPTIONS } from "./types";
import type { DisposeFirearmInput, DispositionType, Firearm, FirearmInput } from "./types";

export interface FirearmDetailProps {
  firearm: Firearm;
  onUpdate: (input: FirearmInput) => Promise<void>;
  onDispose: (input: DisposeFirearmInput) => Promise<void>;
  onDelete: () => Promise<void>;
}

function formatCents(cents: number | null): string {
  return cents == null ? "—" : `$${(cents / 100).toFixed(2)}`;
}

function typeName(firearmTypeId: number): string {
  return FIREARM_TYPE_OPTIONS.find((o) => o.value === String(firearmTypeId))?.label ?? "Unknown";
}

interface DisposeFormState {
  dispositionType: DispositionType | "";
  recipient: string;
  date: string;
  priceDollars: string;
}

function DisposeDialog({
  open,
  onOpenChange,
  onDispose,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onDispose: (input: DisposeFirearmInput) => Promise<void>;
}) {
  const [form, setForm] = useState<DisposeFormState>({
    dispositionType: "",
    recipient: "",
    date: "",
    priceDollars: "",
  });
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit() {
    const price = Math.round(Number.parseFloat(form.priceDollars || "0") * 100);
    if (!form.dispositionType || !form.recipient.trim() || !form.date || !Number.isFinite(price)) {
      setError("Disposition type, recipient, date, and price are all required.");
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      await onDispose({
        dispositionType: form.dispositionType,
        recipient: form.recipient.trim(),
        date: form.date,
        price,
      });
      onOpenChange(false);
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "Failed to mark this firearm disposed.");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange} title="Mark as disposed">
      <Select
        label="Disposition type"
        value={form.dispositionType}
        onValueChange={(value) =>
          setForm((f) => ({ ...f, dispositionType: value as DispositionType }))
        }
        options={DISPOSITION_TYPE_OPTIONS}
      />
      <TextField
        label="Recipient"
        value={form.recipient}
        onChange={(e) => setForm((f) => ({ ...f, recipient: e.target.value }))}
      />
      <TextField
        label="Date"
        type="date"
        value={form.date}
        onChange={(e) => setForm((f) => ({ ...f, date: e.target.value }))}
      />
      <TextField
        label="Price ($)"
        inputMode="decimal"
        value={form.priceDollars}
        onChange={(e) => setForm((f) => ({ ...f, priceDollars: e.target.value }))}
      />
      {error && (
        <p className="hd-field__error" role="alert">
          {error}
        </p>
      )}
      <div className="hd-dialog__actions">
        <Button variant="secondary" onClick={() => onOpenChange(false)} disabled={submitting}>
          Cancel
        </Button>
        <Button variant="primary" onClick={handleSubmit} disabled={submitting}>
          Confirm disposal
        </Button>
      </div>
    </Dialog>
  );
}

/** Detail view for a single firearm: shows its record, and offers edit,
 * dispose, and delete (via the shared ConfirmDialog) actions (US1). */
export function FirearmDetail({ firearm, onUpdate, onDispose, onDelete }: FirearmDetailProps) {
  const [editing, setEditing] = useState(false);
  const [disposing, setDisposing] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  return (
    <div>
      <h2>
        {firearm.make} {firearm.model}
      </h2>
      <dl>
        <dt>Type</dt>
        <dd>{typeName(firearm.firearmTypeId)}</dd>
        <dt>Caliber</dt>
        <dd>{firearm.caliber}</dd>
        <dt>Serial number</dt>
        <dd>{firearm.serialNumber ?? "None (attested)"}</dd>
        <dt>Status</dt>
        <dd>{firearm.status === "disposed" ? "Disposed" : "Active"}</dd>
        <dt>Estimated value</dt>
        <dd>{formatCents(firearm.estimatedValue)}</dd>
        {firearm.notes && (
          <>
            <dt>Notes</dt>
            <dd>{firearm.notes}</dd>
          </>
        )}
        {firearm.accessories && (
          <>
            <dt>Accessories</dt>
            <dd>{firearm.accessories}</dd>
          </>
        )}
        {(firearm.acquisitionSource ||
          firearm.acquisitionDate ||
          firearm.acquisitionPrice != null) && (
          <>
            <dt>Acquired</dt>
            <dd>
              {[
                firearm.acquisitionSource,
                firearm.acquisitionDate,
                formatCents(firearm.acquisitionPrice),
              ]
                .filter(Boolean)
                .join(" · ")}
            </dd>
          </>
        )}
        {firearm.status === "disposed" && (
          <>
            <dt>Disposition</dt>
            <dd>
              {[
                firearm.dispositionType,
                firearm.dispositionRecipient,
                firearm.dispositionDate,
                formatCents(firearm.dispositionPrice),
              ]
                .filter(Boolean)
                .join(" · ")}
            </dd>
          </>
        )}
      </dl>

      <div className="hd-dialog__actions">
        <Button variant="secondary" onClick={() => setEditing(true)}>
          Edit
        </Button>
        {firearm.status === "active" && (
          <Button variant="secondary" onClick={() => setDisposing(true)}>
            Mark disposed
          </Button>
        )}
        <Button variant="danger" onClick={() => setConfirmingDelete(true)}>
          Delete
        </Button>
      </div>

      <Dialog open={editing} onOpenChange={setEditing} title="Edit firearm">
        <FirearmForm
          initialValues={firearm}
          onCancel={() => setEditing(false)}
          onSubmit={async (input) => {
            await onUpdate(input);
            setEditing(false);
          }}
        />
      </Dialog>

      <DisposeDialog open={disposing} onOpenChange={setDisposing} onDispose={onDispose} />

      <ConfirmDialog
        open={confirmingDelete}
        onOpenChange={setConfirmingDelete}
        title="Delete this firearm?"
        description={`This permanently removes ${firearm.make} ${firearm.model} and any photos or documents attached to it. This cannot be undone.`}
        confirmLabel="Delete"
        onConfirm={() => {
          void onDelete();
        }}
      />
    </div>
  );
}
