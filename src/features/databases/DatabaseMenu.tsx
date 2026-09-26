import { useEffect, useState } from "react";
import { Button, Icon, Menu, MenuItem, MenuSeparator } from "../../components";
import { useSession } from "../session/sessionStore";
import { DatabaseSettingsDialog } from "./DatabaseSettingsDialog";
import { RestoreBackupDialog } from "./RestoreBackupDialog";
import "./databases.css";

/** The open database's menu, at the left of the top bar's tools
 * (contracts/ui-databases.md §4). Switching and closing are normal closes,
 * which ask first about a form with unsaved input (§6). It also holds the
 * database's settings (§7) and the restore dialog (§9). */
export function DatabaseMenu() {
  const session = useSession();
  const [dialog, setDialog] = useState<"settings" | "restore" | null>(null);
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
        <MenuItem onSelect={() => void session.closeDatabase("switched")}>
          Switch database…
        </MenuItem>
        <MenuItem onSelect={() => void session.closeDatabase("closed")}>Close database</MenuItem>
        <MenuSeparator />
        <MenuItem onSelect={() => setDialog("settings")}>Database settings…</MenuItem>
        <MenuItem onSelect={() => setDialog("restore")}>Restore from a backup…</MenuItem>
      </Menu>
      <DatabaseSettingsDialog
        open={dialog === "settings"}
        onOpenChange={(open) => setDialog(open ? "settings" : null)}
        status={status}
        onSaved={() => void session.refreshStatus()}
        onRestore={() => setDialog("restore")}
      />
      {dialog === "restore" && (
        <RestoreBackupDialog
          open
          onOpenChange={(open) => setDialog(open ? "restore" : null)}
          name={status.name}
          onChangeLocation={() => setDialog("settings")}
        />
      )}
    </>
  );
}
