import { $, browser, clickButton, fill } from "./ui";

/**
 * What the import specs share: the Import dialog driven by a typed path (the
 * OS's file picker is out of WebDriver's reach), and the tally it ends on.
 */

/** Imports the spreadsheet at `file` through the Import dialog and waits for
 * the tally; the dialog stays open on it, for the caller to read and `Done`. */
export async function importFile(file: string) {
  await clickButton("Import");
  await fill("Spreadsheet file", file);
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
