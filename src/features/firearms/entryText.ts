/** contracts/ui-entry.md §2: the note under a field that was snapped. */
export function snapNote(changedBy: "catalog" | "record", value: string): string {
  return changedBy === "catalog"
    ? `Changed to the built-in spelling “${value}”.`
    : `Changed to “${value}”, as already in your collection.`;
}

/** specs/004-cartridges-action-types FR-015, mirrored from the backend's
 * `check_entry_text` for an immediate message: once trimmed, at most 100
 * characters (counted as characters, not UTF-16 units) and no control
 * characters. The backend stays the authority. */
export function entryTextError(label: string, text: string): string | undefined {
  const trimmed = text.trim();
  if (Array.from(trimmed).length > 100) return `${label} can be at most 100 characters.`;
  if (/\p{Cc}/u.test(trimmed)) return `${label} can't contain control characters.`;
  return undefined;
}
