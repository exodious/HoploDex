import { useState } from "react";
import { Button, Icon, useToast } from "../../components";
import { formatDateTime } from "../../lib/dates";
import { useSession } from "../session/sessionStore";
import { DatabaseGuideLink } from "./DatabaseGuide";
import type { NoteKind } from "./types";
import "./databases.css";

const DISK_ENCRYPTION =
  "Your collection is encrypted with your passphrase. For extra protection, also turn on your computer's disk encryption: BitLocker on Windows, FileVault on macOS, or LUKS on Linux.";

/** The notes shown once at the top of the collection (contracts/ui-databases.md
 * §10). The disk-encryption note stays until it is dismissed, on any
 * computer, since dismissing it is kept in the database (FR-008). A backup
 * opened directly (research.md §9) and a restore (FR-028) are told once, in
 * this session. */
export function DatabaseNotes() {
  const { status, dismissNote } = useSession();
  const notify = useToast();
  const [dismissing, setDismissing] = useState<NoteKind | null>(null);

  if (!status) return null;
  const { diskEncryption, openedBackup, restoredWithPassphraseOf, damagedFileKeptAt } =
    status.notes;
  if (!diskEncryption && !openedBackup && !restoredWithPassphraseOf && !damagedFileKeptAt) {
    return null;
  }

  async function dismiss(note: NoteKind) {
    setDismissing(note);
    try {
      await dismissNote(note);
    } catch {
      notify("The note couldn't be dismissed. Try again.", "error");
    } finally {
      setDismissing(null);
    }
  }

  const dismissButton = (note: NoteKind) => (
    <Button
      variant="ghost"
      size="sm"
      icon="close"
      aria-label="Dismiss"
      pending={dismissing === note}
      onClick={() => void dismiss(note)}
    />
  );

  return (
    <div className="hd-db-notes">
      {(restoredWithPassphraseOf || damagedFileKeptAt) && (
        <section className="hd-banner hd-db-note" aria-label="Restored from a backup">
          <Icon name="archive" />
          <p className="hd-banner__text">
            {status.name} was restored from a backup
            {restoredWithPassphraseOf &&
              ` and now opens with the passphrase it had on ${formatDateTime(restoredWithPassphraseOf)}`}
            .{damagedFileKeptAt && ` The damaged file was kept as ${damagedFileKeptAt}.`}
          </p>
          {dismissButton("restored")}
        </section>
      )}
      {openedBackup && (
        <section className="hd-banner hd-db-note" aria-label="A backup">
          <Icon name="archive" />
          <p className="hd-banner__text">
            This is a backup of {openedBackup.backupOfName} made on{" "}
            {formatDateTime(openedBackup.madeAt)}. Changes here aren&apos;t part of{" "}
            {openedBackup.backupOfName}. It&apos;s still in the backup folder, where it may be
            removed when older backups are cleared: move the file elsewhere to keep it.
          </p>
          {dismissButton("openedBackup")}
        </section>
      )}
      {diskEncryption && (
        <section className="hd-banner hd-db-note" aria-label="Disk encryption">
          <Icon name="shield" />
          <p className="hd-banner__text">{DISK_ENCRYPTION}</p>
          <DatabaseGuideLink>Why?</DatabaseGuideLink>
          {dismissButton("diskEncryption")}
        </section>
      )}
    </div>
  );
}
