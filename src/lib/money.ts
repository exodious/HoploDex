/** Formats an integer cents amount as a dollar string, or an em dash when unset. */
export function formatCents(cents: number | null): string {
  return cents == null ? "—" : `$${(cents / 100).toFixed(2)}`;
}
