import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  application,
  binaryLauncher,
  killApp,
  launchApp,
  relaunchApp,
  waitForAppToQuit,
  type Launcher,
} from "../support/app";
import {
  closeViewer,
  clickRowButton,
  eventMark,
  logLines,
  openDocument,
  recordPreviewEvents,
  viewerTitle,
  waitForEvent,
} from "../support/documentPreview";
import {
  CHECK_LOG_DIR,
  FLATPAK_APP_ID,
  addLogindSession,
  appDataDir,
  appLogPath,
  containerProcesses,
  copiesDir,
  currentRun,
  emitActiveChanged,
  emitPrepareForShutdown,
  emitPrepareForSleep,
  exportNewCommit,
  flatpakLauncher,
  inhibitors,
  installBundle,
  installedCommit,
  installedOrigin,
  lockProbe,
  makeBundle,
  processGone,
  processView,
  removeBundleOrigin,
  removeLogindSession,
  resourceLimit,
  runInSandbox,
  sandboxProcesses,
  startScreenSavers,
  stopScreenSavers,
  terminateApp,
  uninstallApp,
} from "../support/flatpak";
import { importFile, tally } from "../support/importExport";
import { writeTableWorkbook } from "../support/spreadsheets";
import {
  $,
  $$,
  E2E_PASSPHRASE,
  ACKNOWLEDGEMENT,
  addFirearm,
  browser,
  choose,
  chooseMenuItem,
  chooserNames,
  chooserSays,
  clickButton,
  clickEl,
  createDatabase,
  dialogFieldError,
  dialogTitled,
  displayName,
  editNotes,
  expect,
  fieldValue,
  fill,
  goTo,
  isFieldDisabled,
  listedNames,
  openFirearm,
  pressEscape,
  search,
  selectChooserRow,
  selectedChooserRow,
  settle,
  submitPassphrase,
  switchDatabase,
  toggle,
  unlock,
  waitForChooser,
  waitForCollection,
} from "../support/ui";

/**
 * The automated Flatpak check's journey (011 FR-021, 011 US5-2; 011
 * research.md §16). It runs only under `npm run test:flatpak`
 * (HOPLODEX_E2E_FLATPAK=1, scripts/flatpak/check.sh): wdio.conf.ts keeps it out
 * of every other run, installs the check variant before any worker starts,
 * gives this file's worker the run's throwaway home, buses and stand-in
 * services (support/flatpak.ts), and starts the app in its sandbox with the
 * shipped permissions plus `--share=network` for the WebDriver port. The
 * stand-ins a test needs of its own are started and stopped in that test.
 *
 * Its own setup, not another file's: it needs the container's Flatpak
 * installation and buses, so it shares nothing with the other specs
 * (DEVELOPMENT.md, "E2E files").
 */
describe("011 US5 - The Flatpak Is Tested in Its Sandbox, Away From Real Data", () => {
  before(async () => {
    await waitForChooser();
  });

  it("011 US1-1, 011 US5-2: the check variant starts in its sandbox and shows the database chooser, its window titled HoploDex", async () => {
    await expect($(".hd-chooser__title")).toExist();
    expect(await browser.getTitle()).toBe("HoploDex");
    // Inside a Flatpak sandbox: `hoplodex` running /app/bin/hoplodex, the file
    // in the sandbox's own mount namespace, which `flatpak ps` and /proc show
    // from outside.
    const app = sandboxProcesses().find(
      (candidate) => candidate.command[0] === "hoplodex" && candidate.exe === "/app/bin/hoplodex",
    );
    expect(app).toBeDefined();
  });

  it("011 SC-007: the app's data went under the run's throwaway home, not the real one", () => {
    const { home } = currentRun();
    expect(path.isAbsolute(home)).toBe(true);
    expect(path.relative(os.homedir(), home).startsWith("..")).toBe(true);
    // The sandbox made its data folder there as soon as it started...
    expect(fs.existsSync(appDataDir(home))).toBe(true);
    // ...and nothing was made in the real home for it.
    expect(fs.existsSync(path.join(os.homedir(), ".var", "app", FLATPAK_APP_ID))).toBe(false);
  });

  it("011 SC-006: a cold start reaches the chooser within a second of the same build outside the sandbox", async () => {
    // The outside build is `cargo build --profile e2e` of the same sources;
    // scripts/flatpak/check.sh builds it for this comparison.
    if (!fs.existsSync(application)) {
      throw new Error(
        `${application} is not built; scripts/flatpak/check.sh builds it for this comparison ` +
          "(011 research.md §18)",
      );
    }
    /** Milliseconds from starting the app to the chooser being on screen. */
    async function startToChooser(launcher: Launcher): Promise<number> {
      await killApp();
      const started = performance.now();
      await launchApp(launcher);
      await browser.reloadSession();
      await $(".hd-chooser__title").waitForExist({ timeout: 10000 });
      return performance.now() - started;
    }
    /**
     * What a `.deb` install pays before it starts that the `e2e` build in this
     * container doesn't: the WebKit sandbox probe (src-tauri/src/services/
     * preview/sandbox_probe.rs), which a container skips and the Flatpak runs.
     * Milliseconds for one run of it, forced on as the app does.
     */
    function probeMilliseconds(): number {
      const started = performance.now();
      const probe = spawnSync(application, ["--webkit-sandbox-probe"], {
        env: { ...process.env, WEBKIT_FORCE_SANDBOX: "1" },
        stdio: "ignore",
        timeout: 10000,
      });
      if (probe.status !== 0) {
        throw new Error(`the WebKit sandbox probe outside the sandbox exited with ${probe.status}`);
      }
      return performance.now() - started;
    }
    const flatpak = flatpakLauncher();
    // One start of each, not timed, so both find the programs and libraries
    // in the file cache; then one of each, timed.
    await startToChooser(binaryLauncher);
    await startToChooser(flatpak);
    // The probe runs before the app starts, so outside the sandbox it adds to
    // the start as a `.deb` would have it (011 research.md §18).
    const probe = probeMilliseconds();
    const outside = (await startToChooser(binaryLauncher)) + probe;
    const inside = await startToChooser(flatpak);
    console.log(
      `cold start to the chooser: Flatpak ${Math.round(inside)} ms, ` +
        `outside the sandbox ${Math.round(outside)} ms ` +
        `(including a ${Math.round(probe)} ms WebKit sandbox probe), ` +
        `difference ${Math.round(inside - outside)} ms`,
    );
    expect(inside - outside).toBeLessThanOrEqual(1000);
    // The last start was the Flatpak's, which the tests that follow use.
  });
});

// ---------------------------------------------------------------------------
// 011 US1
// ---------------------------------------------------------------------------

const NEW_PASSPHRASE = "a different end to end passphrase";
const FIXTURES = fileURLToPath(
  new URL("../../src-tauri/tests/fixtures/documents/", import.meta.url),
);
const SEED_PHOTOS = fileURLToPath(
  new URL("../../src-tauri/examples/seed-photos/", import.meta.url),
);
/** A tmpfs standing in for a removable drive (the dev container makes it,
 * scripts/dev-container.sh); the manifest grants `/mnt` (011 FR-003). */
const REMOVABLE = "/mnt/hoplodex-check";
/** A folder the container makes for the check that no Flatpak permission
 * covers (Dockerfile; 011 research.md §16). */
const OUTSIDE = "/srv/hoplodex-check";

/** Every file under `dir` (none if it isn't there). */
function filesUnder(dir: string): string[] {
  if (!fs.existsSync(dir)) return [];
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .flatMap((entry) =>
      entry.isDirectory() ? filesUnder(path.join(dir, entry.name)) : [path.join(dir, entry.name)],
    );
}

/** The backups of the database named `name` in `folder`: the ones a normal
 * close makes, `<name> <date> <time> <id>.hoplodex`. */
function regularBackups(folder: string, name: string): string[] {
  if (!fs.existsSync(folder)) return [];
  return fs
    .readdirSync(folder)
    .filter((file) =>
      new RegExp(`^${name} \\d{4}-\\d{2}-\\d{2} \\d{6} [0-9a-f]{8}\\.hoplodex$`).test(file),
    )
    .sort();
}

/** A native drop of files from the file manager onto the window: Tauri's
 * `tauri://drag-drop` event with the paths, which is all WebKitGTK gives
 * the page (WebDriver can't drag from the desktop). */
async function dropFiles(paths: string[]) {
  await browser.execute((dropped: string[]) => {
    const internals = (
      window as unknown as {
        __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> };
      }
    ).__TAURI_INTERNALS__;
    void internals.invoke("plugin:event|emit", {
      event: "tauri://drag-drop",
      payload: { paths: dropped, position: { x: 10, y: 10 } },
    });
  }, paths);
  await settle();
}

/** Selects the chooser's row for `name` unless it is the selected one already
 * (the last-opened database is, and has no select button). */
async function ensureSelected(name: string) {
  if ((await selectedChooserRow()) !== name) await selectChooserRow(name);
}

/** Waits until the record's document list shows `name`. */
async function waitForDocument(name: string) {
  await browser.waitUntil(
    () =>
      browser.execute(
        (wanted: string) =>
          [...document.querySelectorAll("button.hd-doc__name")].some(
            (b) => b.textContent?.trim() === wanted,
          ),
        name,
      ),
    { timeout: 15000, timeoutMsg: `${name} never joined the document list` },
  );
}

/** Every toast's text. */
async function toastTexts(): Promise<string[]> {
  return browser.execute(() =>
    [...document.querySelectorAll(".hd-toast")].map((el) => el.textContent?.trim() ?? ""),
  );
}

/** Removes what the removable-drive test leaves in its folder (a tmpfs of the
 * container's), and only that: its database, the markers beside it and its
 * backups. */
function clearRemovable() {
  if (!fs.existsSync(REMOVABLE)) {
    throw new Error(
      `${REMOVABLE} is missing: the Flatpak check runs in the dev container (--flatpak)`,
    );
  }
  for (const entry of fs.readdirSync(REMOVABLE)) {
    if (/^\.?Removable/.test(entry) || entry === "HoploDex backups") {
      fs.rmSync(path.join(REMOVABLE, entry), { recursive: true, force: true });
    }
  }
}

describe("011 US1 - A Linux User Installs HoploDex as a Flatpak and Keeps Their Collection in It", () => {
  /** Where the suggested location is (011 US1-2): `HoploDex` in the Documents
   * folder the sandbox reads from `xdg-user-dir`. */
  const folder = () => path.join(currentRun().documents, "HoploDex");
  const database = () => path.join(folder(), "Journey.hoplodex");
  const backups = () => path.join(folder(), "HoploDex backups");
  /** The firearm the journey keeps, and its display name. */
  const glock = {
    make: "FlatpakGlock",
    model: "19",
    caliber: "9mm",
    type: "Handgun" as const,
    serial: "FLAT-001",
  };
  /** The journey's files from the file manager, in the throwaway home. */
  const scans = () => path.join(currentRun().documents, "Scans");
  const exportFolder = () => path.join(currentRun().documents, "Export");

  before(async () => {
    await waitForChooser();
    clearRemovable();
    if (fs.readdirSync(OUTSIDE).length > 0) {
      throw new Error(
        `${OUTSIDE} is not empty: the refusal is shown by nothing being written there`,
      );
    }
  });

  after(() => {
    clearRemovable();
  });

  it("011 US1-2: the create dialog suggests HoploDex in the Documents folder the sandbox reads, and a database made there is a file in it", async function () {
    this.timeout(120000);
    await clickButton("Create a new database…");
    await $('[role="dialog"]').waitForExist();
    // The folder is the one `xdg-user-dir DOCUMENTS` answers inside the
    // sandbox (support/flatpak.ts reads it there), as a `.deb` would suggest.
    expect(await fieldValue("Folder")).toBe(folder());
    await fill("Name", "Journey");
    await fill("Passphrase", E2E_PASSPHRASE);
    await fill("Confirm passphrase", E2E_PASSPHRASE);
    await toggle(ACKNOWLEDGEMENT);
    await clickButton("Create database");
    await waitForCollection();

    expect(fs.existsSync(database())).toBe(true);
    // The sandbox's Documents folder is under the throwaway home, not the real one.
    expect(path.relative(os.homedir(), database()).startsWith("..")).toBe(true);
  });

  it("011 US1-3, 011 US1-4: a normal close backs up beside the database, and the file is not locked once it is closed", async function () {
    this.timeout(120000);
    await addFirearm(glock);
    // The exclusive lock holds across the sandbox while the database is open
    // (011 spec.md Edge Cases): a process outside it can't read the file.
    expect(lockProbe(database())).toBe(true);

    await switchDatabase();

    expect(regularBackups(backups(), "Journey")).toHaveLength(1);
    expect(lockProbe(database())).toBe(false);
  });

  it("011 US1-3: the recent list reopens it, the passphrase changes with the file at its own path, and a restore brings the backup back", async function () {
    this.timeout(180000);
    // Reopened from the recent list: the chooser's row, not a typed path.
    expect(await chooserNames()).toEqual(["Journey"]);
    expect(await selectedChooserRow()).toBe("Journey");
    await unlock(E2E_PASSPHRASE);

    await chooseMenuItem("button.hd-db-menu", "Change passphrase…");
    await $('[role="dialog"] input[autocomplete="current-password"]').waitForExist();
    await fill("Current passphrase", E2E_PASSPHRASE);
    await fill("New passphrase", NEW_PASSPHRASE);
    await fill("Confirm new passphrase", NEW_PASSPHRASE);
    await clickButton("Change passphrase");
    await $('[role="dialog"] [role="status"]').waitForExist({
      timeout: 60000,
      timeoutMsg: "the passphrase change never finished",
    });
    await clickButton("Done");
    await $('[role="dialog"]').waitForExist({ reverse: true });
    // The replaced database is at its own path, with no half-made file beside it.
    expect(fs.existsSync(database())).toBe(true);
    expect(fs.readdirSync(folder()).filter((name) => /\.(new|old)$/.test(name))).toEqual([]);

    // The new passphrase opens it.
    await switchDatabase();
    await unlock(NEW_PASSPHRASE);

    await addFirearm({
      make: "AfterBackup",
      model: "Two",
      caliber: "9mm",
      type: "Handgun",
      serial: "FLAT-002",
    });
    const backupsBefore = regularBackups(backups(), "Journey");
    expect(backupsBefore.length).toBeGreaterThanOrEqual(1);

    // Restore the backup made at the first close, which holds the old passphrase.
    await chooseMenuItem("button.hd-db-menu", "Restore from a backup…");
    await $$('[role="dialog"] input[type="radio"]')[0].waitForExist();
    await settle();
    await fill("Passphrase for this backup", E2E_PASSPHRASE);
    await clickEl('[role="dialog"] button[type="submit"]');
    await $('[role="alertdialog"]').waitForExist();
    await clickEl('[role="alertdialog"] .hd-button--danger');
    await $('[role="dialog"]').waitForExist({ reverse: true, timeout: 30000 });
    await waitForCollection();

    await expect($('section[aria-label="Restored from a backup"]')).toExist();
    await goTo("Collection");
    const names = await listedNames();
    expect(names).toContain("FlatpakGlock 19");
    expect(names).not.toContain("AfterBackup Two");
    expect(fs.existsSync(database())).toBe(true);
  });

  it("011 SC-001: a spreadsheet is exported, and a CSV and an XLSX of new firearms import", async function () {
    this.timeout(180000);
    fs.mkdirSync(exportFolder(), { recursive: true });
    await goTo("Collection");
    await clickButton("Export");
    await fill("Save to folder", exportFolder());
    await clickButton("Export");
    await $(".hd-outcome__headline*=Exported").waitForExist({
      timeout: 30000,
      timeoutMsg: "export never finished",
    });
    await expect($('[role="alert"]')).not.toExist();
    await clickButton("Done");

    // The firearms sheet: the exported file whose header has the firearm
    // columns. Its header is the columns a new row is written under.
    const sheet = fs
      .readdirSync(exportFolder())
      .filter((name) => name.endsWith(".csv"))
      .map((name) => path.join(exportFolder(), name))
      .find((file) =>
        fs.readFileSync(file, "utf-8").split(/\r?\n/)[0].split(",").includes("serial_number"),
      );
    expect(sheet).toBeDefined();
    expect(fs.readFileSync(sheet!, "utf-8")).toContain("FlatpakGlock");
    expect(fs.readdirSync(exportFolder()).some((name) => name.endsWith("_photos"))).toBe(true);
    const columns = fs.readFileSync(sheet!, "utf-8").split(/\r?\n/)[0].split(",");
    const row = (make: string, model: string, serial: string) => {
      const cells: Record<string, string> = {
        make,
        model,
        serial_number: serial,
        no_serial_attested: "FALSE",
        caliber: "9mm",
        firearm_type: "Handgun",
      };
      return columns.map((column) => cells[column] ?? "");
    };

    const csv = path.join(exportFolder(), "more.csv");
    fs.writeFileSync(
      csv,
      `${[columns, row("FlatpakCsv", "One", "FLAT-CSV-1")].map((cells) => cells.join(",")).join("\n")}\n`,
    );
    await importFile(csv);
    expect(await tally("added")).toBe(1);
    expect(await tally("failed")).toBe(0);
    await clickButton("Done");

    const workbook = path.join(exportFolder(), "more.xlsx");
    writeTableWorkbook(workbook, [columns, row("FlatpakSheet", "Two", "FLAT-XLSX-2")]);
    await importFile(workbook);
    expect(await tally("added")).toBe(1);
    expect(await tally("failed")).toBe(0);
    await clickButton("Done");

    await search("Flatpak");
    const names = await listedNames();
    expect(names).toContain("FlatpakCsv One");
    expect(names).toContain("FlatpakSheet Two");
    await search("");
  });

  it("011 US1-5, 011 US1-6: a photo dropped from the file manager is attached and shown", async function () {
    this.timeout(120000);
    const pictures = path.join(currentRun().home, "Pictures");
    fs.mkdirSync(pictures, { recursive: true });
    const photo = path.join(pictures, "range-day.jpg");
    fs.copyFileSync(path.join(SEED_PHOTOS, "glock-19-gen3.jpg"), photo);

    await openFirearm("FlatpakGlock 19");
    await dropFiles([photo]);
    await $(".hd-photo").waitForExist({
      timeout: 15000,
      timeoutMsg: "the dropped photo never joined",
    });
    await expect($(".hd-photo__tag*=Thumbnail")).toExist();

    // The image itself, at full size, in the viewer.
    await clickEl("button.hd-photo");
    await browser.waitUntil(
      () =>
        browser.execute(() => {
          const image = document.querySelector<HTMLImageElement>(
            ".hd-viewer__image:not(.hd-viewer__image--loading)",
          );
          return Boolean(image?.complete && image.naturalWidth > 0);
        }),
      { timeout: 15000, timeoutMsg: "the photo never showed at full size" },
    );
    await pressEscape();
    await $('[role="dialog"]').waitForExist({ reverse: true });
  });

  it("011 US1-5, 011 US1-6: dropped documents are attached, and a PDF, a TIFF and a text document preview", async function () {
    this.timeout(180000);
    fs.mkdirSync(scans(), { recursive: true });
    const pdf = path.join(scans(), "three-pages.pdf");
    const tiff = path.join(scans(), "one-page.tif");
    const text = path.join(scans(), "notes.txt");
    fs.copyFileSync(path.join(FIXTURES, "three-pages.pdf"), pdf);
    fs.copyFileSync(path.join(FIXTURES, "one-page.tif"), tiff);
    fs.writeFileSync(text, "HoploDex in a Flatpak keeps this note.\n");

    await dropFiles([pdf, tiff, text]);
    for (const name of ["three-pages.pdf", "one-page.tif", "notes.txt"])
      await waitForDocument(name);
    await recordPreviewEvents();

    // A PDF: the OS web view's own viewer, in a child web view over the page.
    const mark = await eventMark();
    await openDocument("three-pages.pdf");
    await waitForEvent("preview:pdf-ready", mark);
    await closeViewer();

    // A TIFF: decoded in the confined helper, shown as an image.
    await openDocument("one-page.tif");
    await browser.waitUntil(
      () =>
        browser.execute(() => {
          const image = document.querySelector<HTMLImageElement>('img[alt="one-page.tif"]');
          return Boolean(image?.complete && image.naturalWidth > 0);
        }),
      { timeout: 15000, timeoutMsg: "the TIFF never rendered" },
    );
    await closeViewer();

    // A text document.
    await openDocument("notes.txt");
    const shown = await $(".hd-dialog__content--xl pre.hd-preview__text");
    await shown.waitForExist({ timeout: 10000 });
    await expect(shown).toHaveText(
      expect.stringContaining("HoploDex in a Flatpak keeps this note."),
    );
    await closeViewer();
    expect(await viewerTitle()).toBeNull();
  });

  it("011 US1-7, 011 FR-008: a document opens in another application after consent, its copy is on disk while the database is open and gone once it is closed", async function () {
    this.timeout(120000);
    const asked = logLines("HOPLODEX_E2E_CONSENT_LOG").length;
    const opened = logLines("HOPLODEX_E2E_OPENED_LOG").length;

    await clickRowButton("notes.txt", "Open in another app…");

    await browser.waitUntil(() => logLines("HOPLODEX_E2E_OPENED_LOG").length > opened, {
      timeout: 15000,
      timeoutMsg: "the opener never logged the copy",
    });
    const consent = logLines("HOPLODEX_E2E_CONSENT_LOG");
    expect(consent).toHaveLength(asked + 1);
    expect(consent.at(-1)).toContain("notes.txt");
    // The copy the opener log names is on disk, in the app's copies folder.
    const copies = filesUnder(copiesDir());
    expect(copies.filter((file) => path.basename(file) === "notes.txt")).toHaveLength(1);
    const copy = logLines("HOPLODEX_E2E_OPENED_LOG").at(-1)!;
    expect(copies).toContain(copy);
    expect(fs.readFileSync(copy, "utf-8")).toBe("HoploDex in a Flatpak keeps this note.\n");
    await browser.waitUntil(
      async () => (await toastTexts()).includes("Opened notes.txt in another app."),
      { timeout: 5000, timeoutMsg: "no 'Opened … in another app.' toast" },
    );

    // Still locked against a process outside the sandbox.
    expect(lockProbe(database())).toBe(true);

    await switchDatabase();

    expect(filesUnder(copiesDir())).toEqual([]);
    expect(fs.existsSync(copy)).toBe(false);
    expect(lockProbe(database())).toBe(false);
  });

  it("011 US1-4: a saved passphrase opens the database from the recent list after a restart, without typing it", async function () {
    this.timeout(120000);
    await ensureSelected("Journey");
    await toggle("Remember on this computer");
    await $('[role="alertdialog"]').waitForExist();
    await clickButton("Remember passphrase");
    await $('[role="alertdialog"]').waitForExist({ reverse: true });
    await unlock(E2E_PASSPHRASE);
    await switchDatabase();

    await relaunchApp();
    await waitForChooser();

    // The recent list is the earlier run's, and nothing asks for the passphrase.
    expect(await selectedChooserRow()).toBe("Journey");
    await expect($(".hd-db-row--selected .hd-db-row__saved")).toHaveText(
      "Opens without a passphrase on this computer",
    );
    expect(await $(".hd-db-row--selected input").isExisting()).toBe(false);
    await clickButton("Open");
    await waitForCollection();
    await goTo("Collection");
    expect(await listedNames()).toContain("FlatpakGlock 19");
  });

  it("011 US1-3, 011 US1-4: a database on a removable drive is created, backed up beside itself and reopened from the recent list", async function () {
    this.timeout(180000);
    await switchDatabase();
    clearRemovable();
    const file = path.join(REMOVABLE, "Removable.hoplodex");

    await createDatabase({ folder: REMOVABLE, name: "Removable" });
    expect(fs.existsSync(file)).toBe(true);
    await addFirearm({
      make: "RemovableGun",
      model: "One",
      caliber: "9mm",
      type: "Handgun",
      serial: "REM-001",
    });
    expect(lockProbe(file)).toBe(true);
    await switchDatabase();

    expect(regularBackups(path.join(REMOVABLE, "HoploDex backups"), "Removable")).toHaveLength(1);
    expect(lockProbe(file)).toBe(false);
    expect(await chooserNames()).toEqual(expect.arrayContaining(["Journey", "Removable"]));
    await ensureSelected("Removable");
    await unlock(E2E_PASSPHRASE);
    await goTo("Collection");
    expect(await listedNames()).toContain("RemovableGun One");
    await switchDatabase();
  });

  it("011 US1-8, 011 SC-003: a typed location outside what the Flatpak reaches is refused on the folder field, and nothing is written there", async function () {
    this.timeout(120000);
    await clickButton("Create a new database…");
    await $('[role="dialog"]').waitForExist();
    await fill("Name", "Nowhere");
    await fill("Folder", OUTSIDE);
    await fill("Passphrase", E2E_PASSPHRASE);
    await fill("Confirm passphrase", E2E_PASSPHRASE);
    await toggle(ACKNOWLEDGEMENT);
    await clickButton("Create database");

    await browser.waitUntil(async () => (await dialogFieldError("Folder")) !== null, {
      timeout: 15000,
      timeoutMsg: "the folder field showed no refusal",
    });
    const message = (await dialogFieldError("Folder"))!;
    expect(message).toContain("This copy of HoploDex is a Flatpak");
    expect(message).toContain("it can only reach your home folder and removable drives");
    expect(message).toContain(`${OUTSIDE}/Nowhere.hoplodex`);
    expect(message).toContain(`flatpak override --user --filesystem=${OUTSIDE} ${FLATPAK_APP_ID}`);
    // The dialog stays for another folder, and nothing was made outside reach.
    await expect($('[role="dialog"]')).toExist();
    expect(fs.readdirSync(OUTSIDE)).toEqual([]);
    await clickButton("Cancel");
    await $('[role="dialog"]').waitForExist({ reverse: true });
    expect(await chooserNames()).not.toContain("Nowhere");
  });
});

// ---------------------------------------------------------------------------
// 011 US2
// ---------------------------------------------------------------------------

const PROTECTED = "Protected";
/** The one record of the Protected database, and the form the journey leaves
 * unsaved input in. */
const guard = {
  make: "Protect",
  model: "One",
  caliber: "9mm",
  type: "Handgun" as const,
  serial: "PRO-001",
};
const GUARD_NAME = displayName(guard);

/** Whether logind lists a delay inhibitor of HoploDex's for `what`: the stand-in
 * drops one when the app closes its end of the descriptor (011 research.md §6). */
function holdsInhibitor(what: "sleep" | "shutdown"): boolean {
  return inhibitors().some((i) => i.who === "HoploDex" && i.what === what && i.mode === "delay");
}

/** Whether the chooser is on screen. */
function chooserShown(): Promise<boolean> {
  return browser.execute(() => document.querySelector(".hd-chooser__title") !== null);
}

/** Writes a time to the check's log for 011 SC-002's comparison, and the console. */
function noteTime(what: string, ms: number) {
  console.log(`011 SC-002: ${what}: ${ms} ms`);
  fs.mkdirSync(CHECK_LOG_DIR, { recursive: true });
  fs.appendFileSync(path.join(CHECK_LOG_DIR, "lock-times.log"), `${what}: ${ms} ms\n`);
}

/**
 * Sends a signal and times the lock from it: milliseconds until the chooser is
 * on screen, and, for a sleep or shutdown, until logind's list no longer has the
 * delay inhibitor the app held (the app closes its end once the lock is done).
 * The time includes `gdbus` starting to send the signal.
 */
async function timedLock(
  what: string,
  send: () => void,
  inhibitor?: "sleep" | "shutdown",
): Promise<{ lockedMs: number; releasedMs?: number }> {
  const started = performance.now();
  send();
  await browser.waitUntil(chooserShown, {
    timeout: 20000,
    interval: 10,
    timeoutMsg: `${what}: the database never locked`,
  });
  const lockedMs = Math.round(performance.now() - started);
  noteTime(`${what} locked the database after`, lockedMs);
  if (!inhibitor) return { lockedMs };
  await browser.waitUntil(() => !holdsInhibitor(inhibitor), {
    timeout: 10000,
    interval: 10,
    timeoutMsg: `${what}: the ${inhibitor} inhibitor was never released`,
  });
  const releasedMs = Math.round(performance.now() - started);
  noteTime(`${what}: the ${inhibitor} inhibitor was seen released after`, releasedMs);
  return { lockedMs, releasedMs };
}

/** What the latest start of the app wrote to its standard error. */
function appLog(): string {
  return fs.readFileSync(appLogPath(), "utf-8");
}

/** Starts the open of the chooser's selected database, expects the pending
 * changes of `GUARD_NAME`'s edit form, resumes them and checks the notes. */
async function resumePendingNotes(expected: string) {
  await waitForChooser();
  await submitPassphrase(E2E_PASSPHRASE);
  expect(await dialogTitled(`Unsaved changes to ${GUARD_NAME} (edit)`)).toContain(
    "Your changes were kept.",
  );
  await clickButton("Resume editing");
  await dialogTitled(`Edit ${guard.make} ${guard.model}`);
  expect(await fieldValue("Notes")).toBe(expected);
  await clickButton("Cancel");
  await browser.waitUntil(async () => !(await $('[role="dialog"]').isExisting()), {
    timeout: 5000,
    timeoutMsg: "the edit form never closed",
  });
}

/** What `containerProcesses()` knows about a helper or web process, one line each. */
function processLines(processes: { pid: number; ppid?: number; command: string[] }[]): string {
  return processes
    .map((p) => `${p.pid} (parent ${p.ppid ?? "?"}): ${p.command.join(" ")}`)
    .join("\n");
}

describe("011 US2 - The Database's Protections Hold Inside the Sandbox", () => {
  const folder = () => path.join(currentRun().documents, "HoploDex");
  const database = () => path.join(folder(), `${PROTECTED}.hoplodex`);
  const scans = () => path.join(currentRun().documents, "Protected scans");
  /** `flatpak-spawn`'s own flags for a sandboxed web process, as WebKit passed
   * them (read from the running process by the test that follows the launch). */
  let webKitSpawnFlags: string[] = [];
  const slowSpreadsheet = () => path.join(currentRun().documents, "Protected scans", "slow.csv");

  before(async () => {
    await waitForChooser();
    fs.rmSync(path.join(CHECK_LOG_DIR, "lock-times.log"), { force: true });
  });

  after(async () => {
    // What a test may have left changed for the others.
    fs.rmSync(slowSpreadsheet(), { force: true });
    if (currentRun().screenSavers.length === 0) await startScreenSavers();
    try {
      addLogindSession();
    } catch {
      // The session is there already.
    }
  });

  it("011 US2-1, 011 SC-002: logind's PrepareForSleep locks the open database and the app's end of the delay inhibitor is closed afterwards", async function () {
    this.timeout(180000);
    await createDatabase({ name: PROTECTED });
    await addFirearm(guard);
    // A note and a TIFF on the record, for the journeys that follow.
    fs.mkdirSync(scans(), { recursive: true });
    fs.writeFileSync(path.join(scans(), "notes.txt"), "Protected by HoploDex in a Flatpak.\n");
    fs.copyFileSync(path.join(FIXTURES, "one-page.tif"), path.join(scans(), "one-page.tif"));
    await dropFiles([path.join(scans(), "notes.txt"), path.join(scans(), "one-page.tif")]);
    await waitForDocument("notes.txt");
    await waitForDocument("one-page.tif");

    // The app holds a delay inhibitor for each, through the bus proxy.
    expect(holdsInhibitor("sleep")).toBe(true);
    expect(holdsInhibitor("shutdown")).toBe(true);
    expect(lockProbe(database())).toBe(true);

    const { releasedMs } = await timedLock(
      "PrepareForSleep",
      () => emitPrepareForSleep(true),
      "sleep",
    );

    expect(await chooserSays(`HoploDex locked “${PROTECTED}”.`)).toBe(true);
    // The lock released the file and closed the descriptor within logind's own
    // limit on how long a delay inhibitor may hold sleep back (5 s).
    expect(lockProbe(database())).toBe(false);
    expect(releasedMs).toBeLessThan(5000);
    // Only the sleep inhibitor was let go.
    expect(holdsInhibitor("shutdown")).toBe(true);

    // Awake again: the app takes the inhibitor again.
    emitPrepareForSleep(false);
    await browser.waitUntil(() => holdsInhibitor("sleep"), {
      timeout: 10000,
      interval: 20,
      timeoutMsg: "the sleep inhibitor was not taken again after waking",
    });
  });

  it("011 US2-2: the screen saver's ActiveChanged locks the open database when the setting is on", async function () {
    this.timeout(120000);
    await unlock(E2E_PASSPHRASE);
    await chooseMenuItem("button.hd-db-menu", "Database settings…");
    await $('[role="dialog"]').waitForExist();
    const label = "Lock when the computer's screen locks";
    // The stand-ins make the setting available.
    expect(await isFieldDisabled(label)).toBe(false);
    await toggle(label);
    await clickButton("Save");
    await browser.waitUntil(async () => !(await $('[role="dialog"]').isExisting()), {
      timeout: 5000,
      timeoutMsg: "the settings never closed",
    });

    await timedLock("ActiveChanged(true)", () => emitActiveChanged(true));

    expect(await chooserSays(`HoploDex locked “${PROTECTED}”.`)).toBe(true);
    expect(lockProbe(database())).toBe(false);
    // The screen unlocked again, which the app notes and does nothing else for.
    emitActiveChanged(false);
  });

  it("011 SC-002: a sleep locks as fast in the sandbox as outside it", async function () {
    this.timeout(180000);
    // Both locks are of a database that has been backed up today, so neither
    // pays for a backup the other doesn't. Outside the sandbox first.
    await relaunchApp(binaryLauncher);
    await waitForChooser();
    await unlock(E2E_PASSPHRASE);
    const outside = await timedLock("outside the sandbox: PrepareForSleep", () =>
      emitPrepareForSleep(true),
    );
    emitPrepareForSleep(false);

    await relaunchApp(flatpakLauncher());
    await waitForChooser();
    await unlock(E2E_PASSPHRASE);
    const inside = await timedLock("in the Flatpak: PrepareForSleep", () =>
      emitPrepareForSleep(true),
    );
    emitPrepareForSleep(false);

    console.log(
      `011 SC-002: locked after ${inside.lockedMs} ms in the Flatpak and ${outside.lockedMs} ms outside it`,
    );
    expect(inside.lockedMs - outside.lockedMs).toBeLessThanOrEqual(1000);
  });

  it("011 US2-3, 011 FR-013: logind's PrepareForShutdown closes the database and ends the app, keeping the open form's input as pending changes", async function () {
    this.timeout(180000);
    await unlock(E2E_PASSPHRASE);
    await openFirearm(GUARD_NAME);
    await editNotes("typed before the shutdown");

    const app = sandboxProcesses().find(
      (p) => p.command[0] === "hoplodex" && p.command.length === 1,
    );
    expect(app).toBeDefined();
    const started = performance.now();
    emitPrepareForShutdown(true);
    await browser.waitUntil(() => processGone(app!.pid), {
      timeout: 20000,
      interval: 5,
      timeoutMsg: "the app did not end for the shutdown",
    });
    noteTime("PrepareForShutdown ended the app after", Math.round(performance.now() - started));
    // The rest of the sandbox follows it.
    await waitForAppToQuit(20000);
    await browser.waitUntil(() => sandboxProcesses().length === 0, {
      timeout: 20000,
      timeoutMsg: "the sandbox was still running after the app ended",
    });

    // Nothing of the app is left holding the file or a descriptor.
    expect(lockProbe(database())).toBe(false);
    expect(holdsInhibitor("shutdown")).toBe(false);
    expect(holdsInhibitor("sleep")).toBe(false);

    await relaunchApp();
    await resumePendingNotes("typed before the shutdown");
    await switchDatabase();
  });

  it("011 US2-3, 011 FR-013, 011 SC-002: a log-out's SIGTERM ends the app at once, keeping the open form's input, and the opened document's copy is gone", async function () {
    this.timeout(180000);
    await unlock(E2E_PASSPHRASE);
    await openFirearm(GUARD_NAME);
    // A document opened in another application leaves a copy on disk.
    const opened = logLines("HOPLODEX_E2E_OPENED_LOG").length;
    await clickRowButton("notes.txt", "Open in another app…");
    await browser.waitUntil(() => logLines("HOPLODEX_E2E_OPENED_LOG").length > opened, {
      timeout: 15000,
      timeoutMsg: "the opener never logged the copy",
    });
    const copy = logLines("HOPLODEX_E2E_OPENED_LOG").at(-1)!;
    expect(fs.existsSync(copy)).toBe(true);
    expect(filesUnder(copiesDir())).toContain(copy);
    await editNotes("typed before the log-out");

    const started = performance.now();
    await terminateApp();
    noteTime("SIGTERM ended the app after", Math.round(performance.now() - started));

    expect(fs.existsSync(copy)).toBe(false);
    expect(filesUnder(copiesDir())).toEqual([]);
    expect(lockProbe(database())).toBe(false);

    await relaunchApp();
    await resumePendingNotes("typed before the log-out");
    await switchDatabase();
  });

  it("011 US2-6, 011 FR-012, 011 FR-011: the TIFF preview's helper is confined and gone once the app is killed, and WebKit's sandbox is on", async function () {
    this.timeout(180000);
    await unlock(E2E_PASSPHRASE);
    await openFirearm(GUARD_NAME);
    await openDocument("one-page.tif");
    await browser.waitUntil(
      () =>
        browser.execute(() => {
          const image = document.querySelector<HTMLImageElement>('img[alt="one-page.tif"]');
          return Boolean(image?.complete && image.naturalWidth > 0);
        }),
      { timeout: 15000, timeoutMsg: "the TIFF never rendered" },
    );

    // The helper runs in the app's sandbox, started again from the same file.
    const helpers = sandboxProcesses().filter((p) => p.command[1] === "--render-helper");
    expect(helpers).toHaveLength(1);
    const helper = helpers[0].pid;
    const app = sandboxProcesses().find(
      (p) => p.command[0] === "hoplodex" && p.command.length === 1,
    )!;
    // Confined as on a .deb: no core dump, and a data limit that the app,
    // which has none, does not (src-tauri/src/services/helper/confine.rs).
    expect(resourceLimit(helper, "Max core file size")).toEqual({ soft: 0, hard: 0 });
    expect(Number.isFinite(resourceLimit(helper, "Max data size").soft)).toBe(true);
    expect(Number.isFinite(resourceLimit(app.pid, "Max data size").soft)).toBe(false);

    // The log: WebKit's sandbox is on, what the helpers are confined with
    // (once per run) and this helper's start.
    const log = appLog();
    expect(log).toContain("[INFO] WebKit's sandbox is on");
    expect(log).toMatch(/\[INFO\] the helpers are confined with Landlock ABI \d+/);
    expect(log).toContain(`[INFO] started --render-helper (process `);

    await killApp();
    await browser.waitUntil(() => processGone(helper), {
      timeout: 10000,
      timeoutMsg: "the render helper outlived the app",
    });
    expect(sandboxProcesses()).toEqual([]);
  });

  it("011 US2-6, 011 FR-012: the import's helper is confined and gone once the app is killed", async function () {
    this.timeout(180000);
    await relaunchApp();
    await waitForChooser();
    await unlock(E2E_PASSPHRASE);
    await goTo("Collection");
    // A named pipe nobody writes to: the import starts its helper, then waits
    // for the file's first bytes, so the helper is alive when the app is killed.
    fs.mkdirSync(scans(), { recursive: true });
    fs.rmSync(slowSpreadsheet(), { force: true });
    const made = spawnSync("mkfifo", [slowSpreadsheet()]);
    if (made.status !== 0) throw new Error(`mkfifo failed: ${made.stderr}`);
    await clickButton("Import");
    await fill("Spreadsheet file", slowSpreadsheet());
    await clickButton("Import", { settles: false });

    await browser.waitUntil(
      () => sandboxProcesses().some((p) => p.command[1] === "--import-helper"),
      { timeout: 15000, interval: 100, timeoutMsg: "the import's helper never started" },
    );
    const helper = sandboxProcesses().find((p) => p.command[1] === "--import-helper")!.pid;
    expect(resourceLimit(helper, "Max core file size")).toEqual({ soft: 0, hard: 0 });
    expect(Number.isFinite(resourceLimit(helper, "Max data size").soft)).toBe(true);

    const log = appLog();
    expect(log).toContain("[INFO] WebKit's sandbox is on");
    expect(log).toMatch(/\[INFO\] the helpers are confined with Landlock ABI \d+/);
    expect(log).toContain(`[INFO] started --import-helper (process `);

    await killApp();
    await browser.waitUntil(() => processGone(helper), {
      timeout: 10000,
      timeoutMsg: "the import helper outlived the app",
    });
    expect(sandboxProcesses()).toEqual([]);
    fs.rmSync(slowSpreadsheet(), { force: true });
  });

  it("011 US2-7, 011 FR-011: a web process runs in the sub-sandbox WebKit asks the Flatpak portal for", async function () {
    this.timeout(120000);
    await relaunchApp();
    await waitForChooser();
    const processes = containerProcesses();
    const base = (p: { command: string[] }) => path.basename(p.command[0] ?? "");
    const app = processes.find((p) => p.command[0] === "hoplodex" && p.command.length === 1)!;
    const spawners = processes.filter((p) => base(p) === "flatpak-spawn");
    const webProcesses = processes.filter((p) => base(p) === "WebKitWebProcess");
    const bubblewraps = processes.filter((p) => base(p) === "bwrap");
    // What the research record reads (011 research.md §9): the portal's
    // `flatpak-spawn --sandbox` command line, and what the web process sees of
    // its sandbox against what the app sees of its own.
    const looked = ["/", "/home", "/tmp", "/run/user/1000", "/run/flatpak", "/app", "/var/data"];
    const web = webProcesses[0];
    const viewLines = (name: string, pid: number) => {
      const view = processView(pid, looked);
      return [
        `${name} (process ${pid})`,
        `  namespaces: ${JSON.stringify(view.namespaces)}`,
        `  network interfaces: ${view.interfaces.join(", ")}`,
        `  mounts: ${view.mounts.join("; ")}`,
        ...Object.entries(view.folders).map(
          ([folder, entries]) => `  ${folder}: ${entries ? entries.join(" ") : "(not there)"}`,
        ),
      ];
    };
    fs.mkdirSync(CHECK_LOG_DIR, { recursive: true });
    fs.writeFileSync(
      path.join(CHECK_LOG_DIR, "webkit-sandbox.txt"),
      [
        "flatpak-spawn:",
        processLines(spawners),
        "bwrap:",
        processLines(bubblewraps),
        ...viewLines("the app", app.pid),
        ...viewLines("the web process", web.pid),
        "",
      ].join("\n"),
    );
    expect(spawners.length).toBeGreaterThan(0);
    expect(spawners.every((p) => p.command.includes("--sandbox"))).toBe(true);
    // The flags WebKit chose, for the next test to start a sub-sandbox with.
    webKitSpawnFlags = spawners[0].command.filter(
      (argument) =>
        argument === "--sandbox" ||
        argument === "--no-network" ||
        argument.startsWith("--directory=") ||
        argument.startsWith("--sandbox-flag="),
    );
    expect(webProcesses.length).toBeGreaterThan(0);
    // Not in the app's own PID namespace, as it would be unsandboxed.
    expect(webProcesses.every((p) => p.pidNamespace !== app.pidNamespace)).toBe(true);
    expect(appLog()).toContain("[INFO] WebKit's sandbox is on");
  });

  it("011 US2-7, 011 FR-011: a process in a sub-sandbox started with WebKit's own flags reaches no file, no network and no bus name", async function () {
    this.timeout(120000);
    expect(webKitSpawnFlags).toContain("--sandbox");
    const script = [
      "import json, os, socket, subprocess, sys",
      "home, database = sys.argv[1], sys.argv[2]",
      "out = {}",
      'out["interfaces"] = [name for _, name in socket.if_nameindex()]',
      "def listing(path):",
      "    try:",
      "        return sorted(os.listdir(path))",
      "    except OSError as error:",
      '        return "error: " + error.strerror',
      'out["home"] = listing(home)',
      'out["documents"] = listing(os.path.join(home, "Documents"))',
      'out["/media"] = listing("/media")',
      'out["/mnt"] = listing("/mnt")',
      "try:",
      '    open(database, "rb").read(16)',
      '    out["database"] = "read"',
      "except OSError as error:",
      '    out["database"] = "error: " + error.strerror',
      "system = socket.socket(socket.AF_UNIX)",
      "try:",
      '    system.connect("/run/dbus/system_bus_socket")',
      '    out["system bus"] = "connected"',
      "except OSError as error:",
      '    out["system bus"] = "error: " + error.strerror',
      "def ping(name, path):",
      '    call = ["gdbus", "call", "--session", "--dest", name, "--object-path", path,',
      '            "--method", "org.freedesktop.DBus.Peer.Ping"]',
      "    result = subprocess.run(call, capture_output=True, text=True, timeout=20)",
      '    return "answered" if result.returncode == 0 else "refused: " + (result.stderr.strip().splitlines() or ["?"])[-1]',
      "for name, path in [",
      '    ("org.freedesktop.secrets", "/org/freedesktop/secrets"),',
      '    ("org.gnome.ScreenSaver", "/org/gnome/ScreenSaver"),',
      '    ("org.freedesktop.ScreenSaver", "/org/freedesktop/ScreenSaver"),',
      '    ("org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop"),',
      '    ("org.freedesktop.portal.Flatpak", "/org/freedesktop/portal/Flatpak"),',
      "]:",
      '    out["session bus " + name] = ping(name, path)',
      // The portal that started this sandbox is among the names it can reach:
      // asking it for another process must not give that process more than
      // this one has.
      "nested = subprocess.run(",
      '    ["flatpak-spawn", "--directory=/", "python3", "-I", "-c",',
      '     "import os, sys; print(os.listdir(sys.argv[1]))", os.path.join(home, "Documents")],',
      "    capture_output=True, text=True, timeout=30)",
      'out["nested flatpak-spawn lists Documents"] = (nested.stdout.strip() if nested.returncode == 0',
      '    else "failed: " + (nested.stderr.strip().splitlines() or ["?"])[-1])',
      "print(json.dumps(out))",
    ].join("\n");
    const { home } = currentRun();
    const result = runInSandbox("flatpak-spawn", [
      ...webKitSpawnFlags,
      "--sandbox-a11y-own-name=io.github.exodious.HoploDex.Sandboxed.WebProcess-probe",
      "--env=DBUS_SESSION_BUS_ADDRESS=unix:path=/run/flatpak/bus",
      "python3",
      "-I",
      "-c",
      script,
      home,
      database(),
    ]);
    const lines = (result.stdout ?? "").trim().split("\n");
    fs.appendFileSync(
      path.join(CHECK_LOG_DIR, "webkit-sandbox.txt"),
      `\nA process started with flatpak-spawn ${webKitSpawnFlags.join(" ")} reaches:\n${lines.at(-1)}\n(exit ${result.status}; ${result.stderr?.trim()})\n`,
    );
    expect(result.status).toBe(0);
    const reach = JSON.parse(lines.at(-1)!) as Record<string, string | string[]>;
    // No network but the loopback.
    expect(reach.interfaces).toEqual(["lo"]);
    // No file of the home folder or of a removable drive.
    expect(String(reach.database)).toMatch(/^error/);
    for (const folder of ["documents", "/media", "/mnt"]) {
      expect(JSON.stringify(reach[folder])).not.toContain("hoplodex-check");
    }
    expect(JSON.stringify(reach.documents)).not.toContain("HoploDex");
    // Not asserted: a second process asked of the Flatpak portal from inside
    // the sub-sandbox (`flatpak-spawn` without `--sandbox`) runs with the app's
    // own permissions, as the line written above shows. That is how the portal
    // works, and 011 research.md §9 and §20 record it as a limit of what WebKit's
    // sandbox keeps from a compromised web process (011 US2-7). The line is
    // there to be read, and to be updated if Flatpak ever stops it.
    console.log(`011 US2-7: ${reach["nested flatpak-spawn lists Documents"]}`);
    // Neither the system bus nor any name the app talks to on the session bus,
    // apart from the desktop portals, which every sandbox can reach.
    expect(String(reach["system bus"])).toMatch(/^error/);
    for (const name of [
      "org.freedesktop.secrets",
      "org.gnome.ScreenSaver",
      "org.freedesktop.ScreenSaver",
    ]) {
      expect(String(reach[`session bus ${name}`])).toMatch(/^refused/);
    }
    expect(reach["session bus org.freedesktop.portal.Desktop"]).toBe("answered");
  });

  it("011 US2-4: without a screen saver on the bus and without logind's session, Database settings shows the screen-lock option unavailable", async function () {
    this.timeout(180000);
    await stopScreenSavers();
    removeLogindSession();
    await relaunchApp();
    await waitForChooser();
    await unlock(E2E_PASSPHRASE);
    await chooseMenuItem("button.hd-db-menu", "Database settings…");
    await $('[role="dialog"]').waitForExist();
    const label = "Lock when the computer's screen locks";
    expect(await isFieldDisabled(label)).toBe(true);
    const text = await browser.execute(
      () => document.querySelector('[role="dialog"]')?.textContent ?? "",
    );
    expect(text).toContain(
      "Not available: this computer doesn't tell applications when the screen locks.",
    );
    await clickButton("Cancel");
    await switchDatabase();
  });
});

// ---------------------------------------------------------------------------
// 011 US4
// ---------------------------------------------------------------------------

/** Opens Database settings, runs `inside`, and closes them again with Cancel. */
async function inDatabaseSettings<T>(inside: () => Promise<T>): Promise<T> {
  await chooseMenuItem("button.hd-db-menu", "Database settings…");
  await $('[role="dialog"]').waitForExist();
  await settle();
  try {
    return await inside();
  } finally {
    await clickButton("Cancel");
    await $('[role="dialog"]').waitForExist({ reverse: true });
  }
}

/** Whether the "Open documents" radio labelled `label` is the checked one. */
function openingChecked(label: string): Promise<boolean> {
  return browser.execute((wanted: string) => {
    const input = [
      ...document.querySelectorAll<HTMLInputElement>(
        '[role="dialog"] [role="radiogroup"] input[type="radio"]',
      ),
    ].find((r) => r.closest("label")?.textContent?.trim() === wanted);
    return Boolean(input?.checked);
  }, label);
}

describe("011 US4 - Each GitHub Release Carries a Flatpak File That Gets Its Runtime From Flathub", () => {
  const folder = () => path.join(currentRun().documents, "HoploDex");
  const database = () => path.join(folder(), "Upgrade.hoplodex");
  /** This computer's settings and recent list (011 FR-009). */
  const machineJson = () =>
    path.join(appDataDir(currentRun().home), "config", FLATPAK_APP_ID, "machine.json");
  let firstCommit = "";

  before(async () => {
    await waitForChooser();
  });

  after(() => {
    // What the bundle installs left behind: their remote (the next runs of the
    // check install from the check's own).
    removeBundleOrigin();
  });

  it("011 US4-3, 011 FR-024, 011 SC-008: a newer file installed over the first keeps the setting, the recent entry and the saved passphrase, and the database opens without typing it", async function () {
    this.timeout(600000);
    // The check variant, installed from a bundle file as a user installs one:
    // the app is stopped, the build the check installed is removed, and the
    // file goes in. The app's data (under the run's home, keyed by the app id)
    // is not touched by any of it.
    const first = makeBundle("first");
    await killApp();
    uninstallApp();
    installBundle(first);
    expect(installedOrigin()).toMatch(/-origin$/);
    firstCommit = installedCommit();
    await relaunchApp();
    await waitForChooser();

    // A database, its passphrase saved, a per-computer setting changed, then closed.
    await clickButton("Create a new database…");
    await $('[role="dialog"]').waitForExist();
    await fill("Name", "Upgrade");
    await fill("Passphrase", E2E_PASSPHRASE);
    await fill("Confirm passphrase", E2E_PASSPHRASE);
    await toggle(ACKNOWLEDGEMENT);
    await clickButton("Create database");
    await waitForCollection();
    expect(fs.existsSync(database())).toBe(true);
    await addFirearm({
      make: "UpgradeGlock",
      model: "17",
      caliber: "9mm",
      type: "Handgun",
      serial: "UPG-001",
    });

    await inDatabaseSettings(async () => {
      expect(await openingChecked("Preview in HoploDex")).toBe(true);
      await choose("Open in another app");
      await browser.waitUntil(() => openingChecked("Open in another app"), {
        timeout: 10000,
        timeoutMsg: "the setting never took",
      });
    });
    await switchDatabase();

    await ensureSelected("Upgrade");
    await toggle("Remember on this computer");
    await $('[role="alertdialog"]').waitForExist();
    await clickButton("Remember passphrase");
    await $('[role="alertdialog"]').waitForExist({ reverse: true });
    await unlock(E2E_PASSPHRASE);
    await switchDatabase();
    expect(fs.existsSync(machineJson())).toBe(true);
    const before = JSON.parse(fs.readFileSync(machineJson(), "utf-8"));
    expect(JSON.stringify(before.recentDatabases)).toContain("Upgrade.hoplodex");
    expect(before.documentOpening).toBe("external");

    // The next version's file: a new commit of the same app, installed over.
    exportNewCommit("011 US4-3: the next version");
    const second = makeBundle("second");
    await killApp();
    installBundle(second);
    const secondCommit = installedCommit();
    expect(secondCommit).not.toBe(firstCommit);
    await relaunchApp();
    await waitForChooser();

    // Everything is where the first version left it.
    // (the file is this computer's: its id, setting and recent list are the first version's)
    const after = JSON.parse(fs.readFileSync(machineJson(), "utf-8"));
    expect(after.machineId).toBe(before.machineId);
    expect(after.documentOpening).toBe("external");
    expect(after.recentDatabases).toEqual(before.recentDatabases);
    expect(await chooserNames()).toContain("Upgrade");
    expect(await selectedChooserRow()).toBe("Upgrade");
    await expect($(".hd-db-row--selected .hd-db-row__saved")).toHaveText(
      "Opens without a passphrase on this computer",
    );
    expect(await $(".hd-db-row--selected input").isExisting()).toBe(false);
    await clickButton("Open");
    await waitForCollection();
    await goTo("Collection");
    expect(await listedNames()).toContain("UpgradeGlock 17");
    await inDatabaseSettings(async () => {
      expect(await openingChecked("Open in another app")).toBe(true);
    });
    await switchDatabase();
  });
});
