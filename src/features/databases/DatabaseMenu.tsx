import { useEffect, useState } from "react";
import { Button, Icon, Menu, MenuItem, MenuSeparator } from "../../components";
import { useSession } from "../session/sessionStore";
import { ChangePassphraseDialog } from "./ChangePassphraseDialog";
import { DatabaseGuide } from "./DatabaseGuide";
import { DatabaseSettingsDialog } from "./DatabaseSettingsDialog";
import { RestoreBackupDialog } from "./RestoreBackupDialog";
import "./databases.css";

/** "Ctrl+L", or "⌘L" on a Mac. */
const LOCK_SHORTCUT =
  typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘L" : "Ctrl+L";

/** The open database's menu, at the left of the top bar's tools
 * (contracts/ui-databases.md §4), with a lock button beside it. Locking
 * asks nothing (FR-035). Switching and closing are normal closes, which ask
 * first about a form with unsaved input (§6). It also holds the database's
 * settings (§7), the passphrase change (§8), the restore dialog (§9) and
 * the guide (§11). */
export function DatabaseMenu() {
  const session = useSession();
  const [dialog, setDialog] = useState<"settings" | "passphrase" | "restore" | "guide" | null>(
    null,
  );
  const { settingsRequested, clearSettingsRequest } = session;

  // Asked for from a failed-backup notice before this database was opened.
  useEffect(() => {
    if (!settingsRequested) return;
    clearSettingsRequest();
    setDialog("settings");
  }, [settingsRequested, clearSettingsRequest]);

  if (!session.status) return null;
  const status = session.status;

  return (
    <>
      <Menu
        align="end"
        trigger={
          <Button variant="ghost" size="sm" className="hd-db-menu">
            <span className="hd-db-menu__name">{status.name}</span>
            <Icon name="chevronDown" size={16} />
          </Button>
        }
      >
        <MenuItem icon="lock" shortcut={LOCK_SHORTCUT} onSelect={() => void session.lockDatabase()}>
          Lock now
        </MenuItem>
        <MenuItem onSelect={() => void session.closeDatabase("switched")}>
          Switch database…
        </MenuItem>
        <MenuItem onSelect={() => void session.closeDatabase("closed")}>Close database</MenuItem>
        <MenuSeparator />
        <MenuItem onSelect={() => setDialog("settings")}>Database settings…</MenuItem>
        <MenuItem onSelect={() => setDialog("passphrase")}>Change passphrase…</MenuItem>
        <MenuItem onSelect={() => setDialog("restore")}>Restore from a backup…</MenuItem>
        <MenuSeparator />
        <MenuItem onSelect={() => setDialog("guide")}>About databases and security</MenuItem>
      </Menu>
      <Button
        variant="ghost"
        size="sm"
        icon="lock"
        aria-label="Lock now"
        title={`Lock now (${LOCK_SHORTCUT})`}
        onClick={() => void session.lockDatabase()}
      />
      <DatabaseSettingsDialog
        open={dialog === "settings"}
        onOpenChange={(open) => setDialog(open ? "settings" : null)}
        status={status}
        onSaved={() => void session.refreshStatus()}
        onRestore={() => setDialog("restore")}
        onPassphraseSavedChange={() => void session.refreshStatus()}
      />
      {dialog === "passphrase" && (
        <ChangePassphraseDialog
          open
          onOpenChange={(open) => setDialog(open ? "passphrase" : null)}
          name={status.name}
        />
      )}
      {dialog === "restore" && (
        <RestoreBackupDialog
          open
          onOpenChange={(open) => setDialog(open ? "restore" : null)}
          name={status.name}
          onChangeLocation={() => setDialog("settings")}
        />
      )}
      <DatabaseGuide
        open={dialog === "guide"}
        onOpenChange={(open) => setDialog(open ? "guide" : null)}
      />
    </>
  );
}
