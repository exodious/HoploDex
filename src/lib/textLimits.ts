/**
 * The free-text maximums the forms enforce, mirroring the backend's
 * `MAX_SHORT_TEXT_CHARS`, `MAX_LONG_TEXT_CHARS` and `check_text_length` in
 * `src-tauri/src/models/rules.rs` (specs/008-hardening-batch research.md
 * §10). The entry fields (make, model, cartridge, caliber, registration form,
 * registered to) keep their own 100, in `features/firearms/entryText.ts`.
 *
 * No `maxLength` attribute is used with these: it cuts pasted text off
 * without saying so. The field shows its error instead, and the backend
 * stays the authority.
 */

/** A nickname, serial number, finish, source, recipient, origin detail, or a
 * policy's name, number or contact. */
export const MAX_SHORT_TEXT_CHARS = 200;

/** Notes, and a firearm's accessories text. */
export const MAX_LONG_TEXT_CHARS = 4000;

/** The length in characters: Unicode scalar values, as the backend counts
 * them, not UTF-16 units (so an emoji is one). */
export function countCharacters(text: string): number {
  return Array.from(text).length;
}

/** The message under a field whose trimmed text is over `max` characters, or
 * `undefined`. The same words as the backend's `check_text_length`. */
export function textLengthError(label: string, text: string, max: number): string | undefined {
  return countCharacters(text.trim()) > max
    ? `${label} can be at most ${max.toLocaleString("en-US")} characters.`
    : undefined;
}
