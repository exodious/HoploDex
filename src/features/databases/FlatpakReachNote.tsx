import { FLATPAK_APP_ID } from "./outOfReach";

/** The command that gives the Flatpak another folder, with the folder to fill in (#260). */
export const FLATPAK_OVERRIDE_TEMPLATE = `flatpak override --user --filesystem=<folder> ${FLATPAK_APP_ID}`;

/**
 * Says, on a screen that chooses a database or backup location, which folders
 * the Flatpak reaches and how to grant another (#260), since its choosers show
 * only what the sandbox can see. Nothing outside a Flatpak.
 */
export function FlatpakReachNote({ flatpak }: { flatpak: boolean }) {
  if (!flatpak) return null;
  return (
    <p className="hd-field__hint hd-flatpak-reach">
      This copy of HoploDex is a Flatpak, and by default it can reach only the HoploDex folder in
      your Documents folder and removable drives. To give it another folder, run this command with
      that folder in place of &lt;folder&gt;, then restart HoploDex:{" "}
      <code className="hd-flatpak-reach__command">{FLATPAK_OVERRIDE_TEMPLATE}</code>
    </p>
  );
}
