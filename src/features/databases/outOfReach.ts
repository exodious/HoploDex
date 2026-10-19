/**
 * The one frontend wording for a location this copy of HoploDex can't reach
 * (011 FR-004, 011 US1-8). The backend's `LOCATION_OUT_OF_REACH` message is
 * built from the same sentences (specs/011-flatpak/data-model.md, "Error
 * `LOCATION_OUT_OF_REACH`"), and names the grants this copy has; this file
 * words the availability flags that carry no message (`unavailableReason:
 * "outOfReach"`, a close notice's `failureReason: "outOfReach"`, a backup
 * location's `reason`) from the path the UI already has, so the error and the
 * flags read alike (constitution III). The flags don't know the grants, so
 * they say only what is true of any copy.
 */

/** The Flatpak's application id, as `flatpak override` names it. */
export const FLATPAK_APP_ID = "io.github.exodious.HoploDex";

/** The command that gives this copy access to `folder` (011 data-model.md). */
export function overrideCommand(folder: string): string {
  return `flatpak override --user --filesystem=${shellQuote(folder)} ${FLATPAK_APP_ID}`;
}

/** `text` as one POSIX shell word, so the command works if pasted: as is when
 * every character is plain, else in single quotes with each embedded single
 * quote written `'\''`. */
function shellQuote(text: string): string {
  if (/^[A-Za-z0-9_@%+=:,./-]+$/.test(text)) return text;
  return `'${text.replace(/'/g, `'\\''`)}'`;
}

/** What the thing out of reach is: a database file (named by its folder) or a
 * folder (a backup location, or one chosen for a move). */
export type OutOfReachSubject = "database" | "folder";

/**
 * Says that this copy can't reach `folder`, why, and the two fixes: move the
 * data somewhere it can reach, or run the override command and restart.
 * `folder` is the folder that would need access.
 */
export function outOfReachText(subject: OutOfReachSubject, folder: string): string {
  const cant =
    subject === "database"
      ? "This copy of HoploDex can't reach this database's folder."
      : "This copy of HoploDex can't reach this folder.";
  const move =
    subject === "database"
      ? "Move the file into your home folder or onto a removable drive"
      : "Choose a folder in your home folder or on a removable drive";
  // The grants can be changed (`flatpak override`), so this says only what a
  // copy reaches by default (011 T099); the backend's message names the actual
  // grants.
  return `${cant} It is a Flatpak, and by default it can reach only your home folder and removable drives. ${move}, or give HoploDex access to ${folder} by running: ${overrideCommand(folder)} and then restarting HoploDex.`;
}

/** The reason a failed backup gives, as a clause in a sentence (the chooser's
 * "was not backed up: …" notice, a restore's "before" backup). */
export const BACKUP_OUT_OF_REACH_REASON =
  "this copy of HoploDex (a Flatpak) can't reach the backup location";
