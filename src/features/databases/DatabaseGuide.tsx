import { useId, useState } from "react";
import type { ReactNode } from "react";
import { Dialog } from "../../components";
import "./databases.css";

export interface DatabaseGuideProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

interface Section {
  title: string;
  paragraphs: ReactNode[];
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
      "By default a database also locks after 10 minutes without use, and when the computer goes to sleep. Only typing, clicking and touching in HoploDex counts as use. Turning the idle lock off in Database settings stops both. Locking when the computer's screen locks is a separate setting, off by default.",
      "If you were in the middle of editing a firearm or a policy, the unsaved changes are kept inside the database, encrypted like the rest of it, and offered the next time it opens, on any computer. Nothing else you had typed is kept, and passphrases never are.",
      "Some Linux desktops, such as a bare window manager with its own screen locker, lock the screen without telling other applications. There the screen-lock setting can look available but never lock HoploDex; use the idle lock or Lock now instead.",
    ],
  },
  {
    title: "Backups",
    paragraphs: [
      "When you close a database you have changed, HoploDex makes a backup, at most once a day, and keeps the latest 5. By default they go in a HoploDex backups folder next to the database. The number kept and the folder can be changed, and backups turned off, in Database settings.",
      "Each backup is a complete copy of the collection, photos and documents included, encrypted like the database. It opens only with the passphrase the database had when the backup was made, so after a passphrase change, older backups still need the old one. Restoring a backup brings its passphrase back with it.",
      "Firearms and policies you have deleted stay in backups made before you deleted them, until those backups are removed, either as newer ones replace them or with Delete all backups.",
      "A backup on the same disk as the database protects against a damaged file or a mistake, but not against losing the disk or the computer. For that, choose a folder on another drive.",
      "HoploDex never sends backups anywhere. If the database or its backup folder is synced by a cloud service, that service holds only encrypted files, but it may keep copies and older versions of its own that HoploDex cannot delete.",
      "A spreadsheet export is not a backup: it is an unencrypted copy of the collection's data, readable by anyone who can open the file.",
    ],
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
      "The backup and lock settings travel inside the database, so it works the same way on every computer. The list of recent databases and a remembered passphrase stay on each computer. A backup folder chosen on one computer may not exist on another; HoploDex says so there.",
    ],
  },
  {
    title: "Whole-disk encryption",
    paragraphs: [
      "The passphrase protects the database and its backups. Other files on the computer can still hold collection data: spreadsheet exports, the photos and documents you added from, opened document copies before they are deleted, and the system's swap and hibernation files.",
      "Whole-disk encryption protects all of it if the computer is lost or stolen, and covers what secure deletion cannot reach. Turn it on: BitLocker on Windows, FileVault on macOS, or LUKS on Linux. HoploDex doesn't check whether it is on.",
    ],
  },
];

/** "About databases and security" (FR-030, contracts/ui-databases.md §11):
 * a shared `Dialog` with headed sections, in the style of the origin guide.
 * Static text; nothing in it depends on the open database. */
export function DatabaseGuide({ open, onOpenChange }: DatabaseGuideProps) {
  const id = useId();
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
          </section>
        ))}
      </div>
    </Dialog>
  );
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
