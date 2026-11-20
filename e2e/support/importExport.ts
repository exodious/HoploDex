import { $, answerChooser, browser, clickButton } from "./ui";

/**
 * What the import specs share: the Import dialog with its file picked in the
 * chooser the backend opens, which an E2E build answers from a queue (the
 * OS's chooser is out of WebDriver's reach), and the tally it ends on.
 */

/** Picks `files` in the Import dialog's chooser. */
export async function chooseImportFiles(...files: string[]) {
  answerChooser(...files);
  await clickButton("Choose a file…");
}

/** Picks `folder` in the Export dialog's chooser. */
export async function chooseExportFolder(folder: string) {
  answerChooser(folder);
  await clickButton("Choose a folder…");
}

/** Imports the spreadsheet at `file` through the Import dialog and waits for
 * the tally; the dialog stays open on it, for the caller to read and `Done`. */
export async function importFile(file: string) {
  await clickButton("Import");
  await chooseImportFiles(file);
  await clickButton("Import");
  await $(".hd-tally").waitForExist({ timeout: 15000, timeoutMsg: "import never finished" });
}

/** One count of the import's tally by its label ("added", "failed"…). */
export async function tally(label: string): Promise<number> {
  return browser.execute((wanted: string) => {
    const item = [...document.querySelectorAll(".hd-tally__item")].find(
      (i) => i.querySelector(".hd-tally__label")?.textContent?.trim() === wanted,
    );
    return Number(item?.querySelector(".hd-tally__value")?.textContent ?? "NaN");
  }, label);
}
