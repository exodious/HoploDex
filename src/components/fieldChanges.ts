/** What `stored` returns for a value a save can't read yet, such as a
 * half-typed date: it always counts as a change (014 FR-003). An empty entry
 * is not this; it is stored as "not recorded". */
export const UNREADABLE = "\u0000unreadable";

/** The fields whose stored value differs from the saved one (014 FR-002,
 * research.md §1). `stored` says what a save would store for a field, as a
 * comparable string, or {@link UNREADABLE}. Every form in scope reads its
 * markers, Save and unsaved input (014 FR-019) from this one result. */
export function changedFields<F extends string, S>(
  saved: S,
  current: S,
  fields: readonly F[],
  stored: (field: F, state: S) => string,
): ReadonlySet<F> {
  const changed = new Set<F>();
  for (const field of fields) {
    const now = stored(field, current);
    if (now === UNREADABLE || now !== stored(field, saved)) changed.add(field);
  }
  return changed;
}

/** How a revert names a saved value: "Not recorded" when empty, and a long
 * one cut to 40 characters (014 FR-010). */
export function savedText(value: string | null | undefined): string {
  const text = (value ?? "").trim();
  if (!text) return "Not recorded";
  return text.length > 40 ? `${text.slice(0, 40)}…` : text;
}

/** A changed field's marker and revert, as a field component takes it. */
export interface FieldChange {
  /** The saved value, as {@link savedText} gives it. */
  saved: string;
  onRevert: () => void;
}

/** {@link changedFields} with what a form renders from it: `isDirty`, and each
 * field's marker and revert while it is changed. `editing` is false on an add
 * form, which compares with how it opened but marks nothing (014 FR-015a,
 * FR-019a). */
export function formChanges<F extends string, S>({
  saved,
  current,
  fields,
  stored,
  editing,
  display,
  revert,
}: {
  saved: S;
  current: S;
  fields: readonly F[];
  stored: (field: F, state: S) => string;
  editing: boolean;
  /** The saved value as the revert names it, before {@link savedText}. */
  display: (field: F) => string | null | undefined;
  revert: (field: F) => void;
}) {
  const changed = changedFields(saved, current, fields, stored);
  return {
    changed,
    isDirty: changed.size > 0,
    changeFor: (field: F): FieldChange | undefined =>
      editing && changed.has(field)
        ? { saved: savedText(display(field)), onRevert: () => revert(field) }
        : undefined,
  };
}
