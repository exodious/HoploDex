import { useState } from "react";
import { Button, Select } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import * as importExportService from "./importExportService";
import type { ConflictAction, ImportConflict } from "./types";

export interface ImportConflictResolverProps {
  sessionId: string;
  conflicts: ImportConflict[];
  onResolved: () => void;
}

const ACTION_OPTIONS: { value: ConflictAction; label: string }[] = [
  { value: "skip", label: "Skip (keep existing)" },
  { value: "overwrite", label: "Overwrite existing" },
  { value: "duplicate", label: "Import as a new duplicate" },
];

/** Per-row skip/overwrite/duplicate resolution for imported rows matching
 * an existing `(make, model, serial_number)` record (US5, FR-026), plus an
 * "apply to remaining" shortcut for rows the user hasn't set individually. */
export function ImportConflictResolver({
  sessionId,
  conflicts,
  onResolved,
}: ImportConflictResolverProps) {
  const [actions, setActions] = useState<Record<string, ConflictAction>>({});
  const [applyToRemaining, setApplyToRemaining] = useState<ConflictAction | "">("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit() {
    setSubmitting(true);
    setError(null);
    try {
      await importExportService.resolveImportConflicts({
        importSessionId: sessionId,
        resolutions: Object.entries(actions).map(([conflictId, action]) => ({
          conflictId,
          action,
        })),
        applyToRemaining: applyToRemaining || undefined,
      });
      onResolved();
    } catch (e) {
      setError(e instanceof CommandFailure ? e.message : "Failed to resolve import conflicts.");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div>
      <h2>Resolve import conflicts</h2>
      <p>
        These rows match an existing record by make, model, and serial number. Choose how to resolve
        each one.
      </p>

      <ul>
        {conflicts.map((conflict) => (
          <li key={conflict.conflictId}>
            <span>
              Row {conflict.row}: {conflict.make} {conflict.model}
              {conflict.serialNumber ? ` (${conflict.serialNumber})` : ""}
            </span>
            <Select
              label={`Resolution for row ${conflict.row}`}
              value={actions[conflict.conflictId] ?? "skip"}
              onValueChange={(value) =>
                setActions((prev) => ({ ...prev, [conflict.conflictId]: value as ConflictAction }))
              }
              options={ACTION_OPTIONS}
            />
          </li>
        ))}
      </ul>

      <Select
        label="Apply to remaining rows not set above"
        value={applyToRemaining}
        onValueChange={(value) => setApplyToRemaining(value as ConflictAction)}
        options={ACTION_OPTIONS}
        placeholder="Leave unset to skip them"
      />

      {error && (
        <p className="hd-field__error" role="alert">
          {error}
        </p>
      )}

      <div className="hd-dialog__actions">
        <Button variant="primary" onClick={handleSubmit} disabled={submitting}>
          Resolve all
        </Button>
      </div>
    </div>
  );
}
