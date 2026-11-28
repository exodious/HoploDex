import { $, answerChooser, browser, clickButton, clickEl } from "./ui";

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

/** Saves a blank import spreadsheet in `folder` from the Import dialog
 * (015 US1), as a workbook or as the two CSV files, and closes both dialogs. */
export async function saveBlankSpreadsheet(folder: string, format: "xlsx" | "csv") {
  await clickButton("Import");
  await clickButton("Get a blank spreadsheet…");
  if (format === "csv") await clickEl('input[type="radio"][value="csv"]');
  await chooseExportFolder(folder);
  await clickButton("Save");
  await $(".hd-outcome__headline=Saved a blank spreadsheet.").waitForExist({
    timeout: 15000,
    timeoutMsg: "the blank spreadsheet was never saved",
  });
  await clickButton("Done");
  await clickButton("Cancel");
}
