import { useState } from "react";
import { Button, Dialog } from "../../components";
import { FLATPAK_APP_ID } from "./outOfReach";

/** The command that gives the Flatpak another folder, with the folder to fill in (#260). */
export const FLATPAK_OVERRIDE_TEMPLATE = `flatpak override --user --filesystem=<folder> ${FLATPAK_APP_ID}`;

/**
 * Says, on a screen that chooses a database or backup location, which folders
 * the Flatpak reaches, with a link to how to grant another (#260), since its
 * choosers show only what the sandbox can see. Nothing outside a Flatpak.
 */
export function FlatpakReachNote({ flatpak }: { flatpak: boolean }) {
  const [open, setOpen] = useState(false);
  if (!flatpak) return null;
  return (
    <p className="hd-field__hint hd-flatpak-reach">
      HoploDex can reach Documents/HoploDex and removable drives.{" "}
      <button type="button" className="hd-link" onClick={() => setOpen(true)}>
        Allow another folder…
      </button>
      <AllowFolderDialog open={open} onOpenChange={setOpen} />
    </p>
  );
}

function AllowFolderDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [copied, setCopied] = useState(false);
  async function copy() {
    try {
      await navigator.clipboard.writeText(FLATPAK_OVERRIDE_TEMPLATE);
      setCopied(true);
    } catch {
      // The command stays selectable at a click, so it can still be copied by hand.
    }
  }
  function changeOpen(next: boolean) {
    onOpenChange(next);
    setCopied(false);
  }
  return (
    <Dialog
      open={open}
      onOpenChange={changeOpen}
      title="Allow another folder"
      description="Run this command in a terminal with the folder in place of <folder>, then restart HoploDex."
      size="md"
      footer={
        <Button variant="primary" onClick={() => changeOpen(false)}>
          Done
        </Button>
      }
    >
      <div className="hd-flatpak-reach__row">
        <code className="hd-flatpak-reach__command">{FLATPAK_OVERRIDE_TEMPLATE}</code>
        <Button size="sm" onClick={() => void copy()}>
          {copied ? "Copied" : "Copy"}
        </Button>
      </div>
    </Dialog>
  );
}
