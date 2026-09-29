import { $, createDatabase, settleChooserPlate, waitForChooser } from "../support/ui";
import { chooseTheme, shot } from "../support/screenshots";

/**
 * The screenshot screens that need a sandbox with no databases: the first
 * run's chooser, and a new database's collection with its disk-encryption
 * note (the seeded databases have it dismissed). Run by `npm run
 * screenshots` after screens.e2e.ts; the harness doesn't seed this one.
 */

describe("Screenshots: first run", () => {
  for (const theme of ["Light", "Dark"] as const) {
    it(`first-run chooser (${theme.toLowerCase()})`, async () => {
      await waitForChooser();
      await chooseTheme(theme);
      await settleChooserPlate();
      await shot(`15-chooser-first-run-${theme.toLowerCase()}`);
    });
  }

  for (const theme of ["Light", "Dark"] as const) {
    it(`disk-encryption note (${theme.toLowerCase()})`, async () => {
      if (!(await $('nav[aria-label="Sections"]').isExisting())) {
        await createDatabase({ name: "My collection" });
      }
      await chooseTheme(theme);
      await $('section[aria-label="Disk encryption"]').waitForExist();
      await shot(`24-disk-encryption-note-${theme.toLowerCase()}`);
    });
  }
});
