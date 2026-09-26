import { Button, Icon, Menu, MenuItem } from "../../components";
import { useSession } from "../session/sessionStore";
import "./databases.css";

/** The open database's menu, at the left of the top bar's tools
 * (contracts/ui-databases.md §4). Switching and closing are normal closes,
 * which ask first about a form with unsaved input (§6). */
export function DatabaseMenu() {
  const session = useSession();
  if (!session.status) return null;

  return (
    <Menu
      align="end"
      trigger={
        <Button variant="ghost" size="sm" className="hd-db-menu">
          <span className="hd-db-menu__name">{session.status.name}</span>
          <Icon name="chevronDown" size={16} />
        </Button>
      }
    >
      <MenuItem onSelect={() => void session.closeDatabase("switched")}>Switch database…</MenuItem>
      <MenuItem onSelect={() => void session.closeDatabase("closed")}>Close database</MenuItem>
    </Menu>
  );
}
