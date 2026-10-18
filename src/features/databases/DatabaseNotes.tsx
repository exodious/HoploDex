import { useState } from "react";
import { Button, Icon, useToast } from "../../components";
import { formatDateTime } from "../../lib/dates";
import { useSession } from "../session/sessionStore";
import { DatabaseGuideLink } from "./DatabaseGuide";
import type { NoteKind } from "./types";
import "./databases.css";

const DISK_ENCRYPTION =
  "Your collection is encrypted with your passphrase. For extra protection, also turn on your computer's disk encryption: BitLocker on Windows, FileVault on macOS, or LUKS on Linux.";

/** The notes shown once at the top of the Collection, Accessories and
 * Insurance pages (003 contracts/ui-databases.md §10, #59). The disk-encryption note stays until it is dismissed, on any
 * computer, since dismissing it is kept in the database (003 FR-008). A backup
 * opened directly (003 research.md §9) and a restore (003 FR-028) are told once, in
 * this session. */
export function DatabaseNotes() {
  const { status, dismissNote } = useSession();
  const notify = useToast();
  const [dismissing, setDismissing] = useState<NoteKind | null>(null);

  if (!status) return null;
  const { diskEncryption, openedBackup, restoredWithPassphraseOf, damagedFileKeptAt, upgraded } =
    status.notes;
  if (
    !diskEncryption &&
    !openedBackup &&
    !restoredWithPassphraseOf &&
    !damagedFileKeptAt &&
    !upgraded
  ) {
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

  // A restore that upgraded is told after the restored note (009 contracts/ui.md §5).
  const restored = Boolean(restoredWithPassphraseOf || damagedFileKeptAt);
  const quoted = `“${status.name}”`;

  return (
    <div className="hd-db-notes">
      {(restoredWithPassphraseOf || damagedFileKeptAt) && (
        <section className="hd-banner hd-db-note" aria-label="Restored from a backup">
          <Icon name="archive" />
          <p className="hd-banner__text">
            The database was restored from a backup
            {restoredWithPassphraseOf &&
              ` and now opens with the passphrase it had on ${formatDateTime(restoredWithPassphraseOf)}`}
            .{damagedFileKeptAt && ` The damaged file was kept as ${damagedFileKeptAt}.`}
          </p>
          {dismissButton("restored")}
        </section>
      )}
      {upgraded && (
        <section className="hd-banner hd-db-note" aria-label="Upgraded">
          <Icon name="archive" />
          <p className="hd-banner__text">
            {restored
              ? `The backup was made by an earlier version of HoploDex, so ${quoted} was upgraded for HoploDex ${upgraded.version} as it was restored. Earlier versions can still open the backup it came from, ${upgraded.backupPath}.`
              : `${quoted} was upgraded for HoploDex ${upgraded.version}. A backup of it from before the upgrade is ${upgraded.backupPath}. Earlier versions of HoploDex can open that backup, but not the upgraded database.`}
            {!restored &&
              !upgraded.atBackupLocation &&
              " That folder isn't your backup location, so the backup isn't in the database's list of backups."}
          </p>
          {dismissButton("upgraded")}
        </section>
      )}
      {openedBackup && (
        <section className="hd-banner hd-db-note" aria-label="A backup">
          <Icon name="archive" />
          <p className="hd-banner__text">
            This is a backup of “{openedBackup.backupOfName}” made on{" "}
            {formatDateTime(openedBackup.madeAt)}
            {openedBackup.beforeUpgradeTo &&
              `, before it was upgraded to HoploDex ${openedBackup.beforeUpgradeTo}`}
            . Changes here aren&apos;t part of “{openedBackup.backupOfName}”.{" "}
            {openedBackup.beforeUpgradeTo
              ? "It's still in the backup folder: move the file elsewhere to keep it."
              : "It's still in the backup folder, where it may be removed when older backups are cleared: move the file elsewhere to keep it."}
          </p>
          {dismissButton("openedBackup")}
        </section>
      )}
      {diskEncryption && (
        <section className="hd-banner hd-db-note" aria-label="Disk encryption">
          <Icon name="shield" />
          <p className="hd-banner__text">
            {DISK_ENCRYPTION}
            <DatabaseGuideLink>Why?</DatabaseGuideLink>
          </p>
          {dismissButton("diskEncryption")}
        </section>
      )}
    </div>
  );
}
