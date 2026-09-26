import { useState } from "react";
import { Button, Icon, useToast } from "../../components";
import { useSession } from "../session/sessionStore";
import "./databases.css";

const DISK_ENCRYPTION =
  "Your collection is encrypted with your passphrase. For extra protection, also turn on your computer's disk encryption: BitLocker on Windows, FileVault on macOS, or LUKS on Linux.";

/** The notes shown once at the top of the collection (contracts/ui-databases.md
 * §10). The disk-encryption note stays until it is dismissed, on any
 * computer, since dismissing it is kept in the database (FR-008). */
export function DatabaseNotes() {
  const { status, dismissNote } = useSession();
  const notify = useToast();
  const [dismissing, setDismissing] = useState(false);

  if (!status?.notes.diskEncryption) return null;

  async function dismiss() {
    setDismissing(true);
    try {
      await dismissNote("diskEncryption");
    } catch {
      notify("The note couldn't be dismissed. Try again.", "error");
    } finally {
      setDismissing(false);
    }
  }

  return (
    <div className="hd-db-notes">
      <section className="hd-banner hd-db-note" aria-label="Disk encryption">
        <Icon name="shield" />
        <p className="hd-banner__text">{DISK_ENCRYPTION}</p>
        {/* Opens the guide once it exists ("About databases and security"). */}
        <Button variant="ghost" size="sm">
          Why?
        </Button>
        <Button
          variant="ghost"
          size="sm"
          icon="close"
          aria-label="Dismiss"
          pending={dismissing}
          onClick={() => void dismiss()}
        />
      </section>
    </div>
  );
}
