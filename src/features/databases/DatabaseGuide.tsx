import { useContext, useId, useState } from "react";
import type { ReactNode } from "react";
import { Dialog } from "../../components";
import { SessionContext } from "../session/sessionStore";
import { backupRows, DEFAULT_SETTINGS, lockRows, minutesLabel } from "./settings";
import type { SettingRow } from "./settings";
import type { DatabaseStatus } from "./types";
import "./databases.css";

export interface DatabaseGuideProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

interface Section {
  title: string;
  paragraphs: ReactNode[];
  /** The open database's settings this section explains, beside their
   * defaults. */
  settings?: (status: DatabaseStatus) => SettingRow[];
}

/** The guide's text (FR-030, contracts/ui-databases.md §11). The dialogs
 * say each point in a sentence or two; this is where it is explained. */
const SECTIONS: Section[] = [
  {
    title: "Your passphrase",
    paragraphs: [
      "Each database is encrypted with a key made from its passphrase. The file and the passphrase are all it takes to open it, on any computer, and nothing else opens it.",
      "Length matters most. Anyone with a copy of the file can try passphrases on their own computer as fast as it allows, with no limit on attempts, so a long passphrase is what keeps them out. Several unrelated words make one that is long and still easy to remember.",
      "A forgotten passphrase cannot be recovered, by HoploDex or by anyone else, and the collection is lost with it. Keep it somewhere safe. A spreadsheet export is the only other copy of the collection, and it isn't encrypted.",
      "Wherever it goes (another computer, a USB drive, a cloud folder, a backup), a copy of the file is protected only by the passphrase.",
    ],
  },
  {
    title: "Remembering the passphrase on this computer",
    paragraphs: [
      "This is off unless you turn it on, for one database at a time. When it is on, the passphrase is kept in this computer's keyring (Keychain on macOS, Credential Manager on Windows, the Secret Service keyring on Linux), and the database opens here without asking for it.",
      "Anyone who can use this computer account, or its keyring while it's unlocked, can then open the database without knowing the passphrase. On a shared account that defeats the passphrase. Locking no longer needs the passphrase here either, though it still does everything else described under Locking.",
      "Other computers, and other databases, still ask. Forget the saved passphrase at any time in Database settings; removing the database from the list forgets it too.",
    ],
  },
  {
    title: "Locking",
    paragraphs: [
      "Lock now (Ctrl+L, or ⌘L on a Mac) closes the database completely: its data is cleared from memory, opened document copies are deleted, a backup is made if one is due, and the database is released for other computers. Unlocking is opening it again with its passphrase.",
      `By default a database also locks after ${minutesLabel(DEFAULT_SETTINGS.idleMinutes)} without use, and when the computer goes to sleep. Only typing, clicking and touching in HoploDex counts as use. Turning the idle lock off in Database settings stops both. Locking when the computer's screen locks is a separate setting, off by default.`,
      "If you were in the middle of editing a firearm or a policy, the unsaved changes are kept inside the database, encrypted like the rest of it, and offered the next time it opens, on any computer. Nothing else you had typed is kept, and passphrases never are.",
      "Some Linux desktops, such as a bare window manager with its own screen locker, lock the screen without telling other applications. There the screen-lock setting can look available but never lock HoploDex; use the idle lock or Lock now instead.",
    ],
    settings: (status) => lockRows(status.settings.lock, status.screenLockSupported),
  },
  {
    title: "Backups",
    paragraphs: [
      `When a database you have changed closes, because it locks or HoploDex quits, HoploDex makes a backup, at most once a day. By default it keeps the latest ${DEFAULT_SETTINGS.keepCount}, in a HoploDex backups folder next to the database. The number kept and the folder can be changed, and backups turned off, in Database settings.`,
      "Each backup is a complete copy of the collection, photos and documents included, encrypted like the database. It opens only with the passphrase the database had when the backup was made, so after a passphrase change, older backups still need the old one. Restoring a backup brings its passphrase back with it.",
      "Firearms and policies you have deleted stay in backups made before you deleted them, until those backups are removed, either as newer ones replace them or with Delete all backups.",
      "A backup on the same disk as the database protects against a damaged file or a mistake, but not against losing the disk or the computer. For that, choose a folder on another drive.",
      "HoploDex never sends backups anywhere. If the database or its backup folder is synced by a cloud service, that service holds only encrypted files, but it may keep copies and older versions of its own that HoploDex cannot delete.",
      "A spreadsheet export is not a backup: it is an unencrypted copy of the collection's data, readable by anyone who can open the file.",
    ],
    settings: (status) => backupRows(status.settings.backups),
  },
  {
    title: "Secure deletion",
    paragraphs: [
      "When HoploDex removes a file that held collection data (the old file after a passphrase change, backups, opened document copies), it overwrites the file, deletes it, and asks the drive to discard the space.",
      "This is best effort. SSDs and flash drives move data around internally, journaling and copy-on-write filesystems can keep earlier versions, filesystem snapshots keep their own copies, and so do cloud-synced folders. No application can reach those.",
      "Whatever is left of an old database file or backup is still protected by the passphrase it had, so it matters only if someone else knows that passphrase. Backups and copies made earlier, or elsewhere, are not touched and still open with the passphrase they had when they were made.",
    ],
  },
  {
    title: "Using a database on more than one computer",
    paragraphs: [
      "A database file opens on Windows, macOS and Linux with its passphrase alone. You can copy it to another computer, or keep it on a network drive or in a synced folder and use it from several.",
      "Use it on one computer at a time. While it is open, the database records which computer has it open and since when, and another computer that tries to open it is told so and refused. The mark is cleared when the database is closed normally.",
      "If that computer crashed, lost its connection, or hasn't finished syncing, you can choose Take over. If the other computer does still have the database open, or its latest changes haven't synced yet, changes can be lost. Once it notices, the other computer stops saving to the database.",
      "The backup and lock settings travel inside the database, so it works the same way on every computer. The list of recent databases and a remembered passphrase stay on each computer. A backup folder chosen on one computer may not exist on another. There, Database settings shows the folder as not available, and HoploDex tells you when a backup couldn't be made, so you can choose another folder on that computer.",
    ],
  },
  {
    title: "Whole-disk encryption",
    paragraphs: [
      "The passphrase protects the database and its backups. Other files on the computer can still hold collection data: spreadsheet exports, the original files of photos and documents you added, opened document copies before they are deleted, and the system's swap and hibernation files.",
      "Whole-disk encryption protects all of it if the computer is lost or stolen, and covers what secure deletion cannot reach. Turn it on: BitLocker on Windows, FileVault on macOS, or LUKS on Linux. HoploDex doesn't check whether it is on.",
    ],
  },
];

/** "About databases and security" (FR-030, contracts/ui-databases.md §11):
 * a shared `Dialog` with headed sections, in the style of the origin guide.
 * The text is the same for every database; with one open, the backup and
 * lock sections also show how it is set up, beside the defaults. */
export function DatabaseGuide({ open, onOpenChange }: DatabaseGuideProps) {
  const id = useId();
  // Also shown where no session is, such as a test of the dialog alone.
  const status = useContext(SessionContext)?.status ?? null;
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="About databases and security"
      description="How HoploDex protects a database, and what it can't do for you."
      size="lg"
    >
      <div className="hd-db-guide">
        {SECTIONS.map((section, index) => (
          <section
            key={section.title}
            className="hd-db-guide__section"
            aria-labelledby={`${id}-${index}`}
          >
            <h3 id={`${id}-${index}`} className="hd-form-section__title">
              {section.title}
            </h3>
            {section.paragraphs.map((paragraph, n) => (
              <p key={n}>{paragraph}</p>
            ))}
            {status && section.settings && (
              <DatabaseSettingsTable rows={section.settings(status)} />
            )}
          </section>
        ))}
      </div>
    </Dialog>
  );
}

/** How the open database is set up, one row a setting: its value, and
 * the default beside it, "(default)" when they are the same. */
function DatabaseSettingsTable({ rows }: { rows: SettingRow[] }) {
  const id = useId();
  return (
    <div className="hd-db-guide__settings" role="group" aria-labelledby={id}>
      <h4 id={id} className="hd-db-guide__settings-title">
        How this database is set up
      </h4>
      <dl>
        {rows.map((row) => {
          const changed = row.value !== row.defaultValue;
          return (
            <div
              key={row.label}
              className="hd-db-guide__setting"
              data-changed={changed || undefined}
            >
              <dt>{row.label}</dt>
              <dd>
                <span className="hd-db-guide__value">{row.value}</span>{" "}
                <span className="hd-db-guide__default">
                  {changed ? `(default: ${lowerFirst(row.defaultValue)})` : "(default)"}
                </span>
                {row.note && <span className="hd-db-guide__note">{row.note}</span>}
              </dd>
            </div>
          );
        })}
      </dl>
    </div>
  );
}

/** "On" reads "(default: on)" after a value. */
function lowerFirst(text: string): string {
  return text.charAt(0).toLowerCase() + text.slice(1);
}

/** A link that opens the guide, for the texts that point to it: the
 * disk-encryption note's **Why?**, and the backup and passphrase texts
 * that say "see About databases and security". */
export function DatabaseGuideLink({ children }: { children?: ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" className="hd-link" onClick={() => setOpen(true)}>
        {children ?? "About databases and security"}
      </button>
      <DatabaseGuide open={open} onOpenChange={setOpen} />
    </>
  );
}
