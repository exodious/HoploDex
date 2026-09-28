import {
  $,
  E2E_PASSPHRASE,
  chooseMenuItem,
  clickButton,
  createDatabase,
  expect,
  switchDatabase,
  unlock,
} from "../support/ui";

/**
 * User Story 5 (003) on a computer with no keyring service (FR-019): the
 * harness launches this spec's app with `HOPLODEX_E2E_KEYRING=unavailable`
 * (wdio.conf.ts), so remembering a passphrase is offered nowhere and
 * everything else works.
 */

const UNAVAILABLE = "Not available: this computer has no keyring service.";

describe("User Story 5 (003) - without a keyring", () => {
  it("disables remembering in the chooser", async () => {
    await createDatabase({ name: "Keyless" });
    await switchDatabase();

    const remember = $(".hd-db-row--selected .hd-db-row__remember button[role='checkbox']");
    await expect(remember).toBeDisabled();
    await expect($(`p=${UNAVAILABLE}`)).toExist();
  });

  it("says so in the database settings, and the passphrase still opens it", async () => {
    await createDatabase({ name: "Second" });
    await chooseMenuItem("button.hd-db-menu", "Database settings…");
    await $('[role="dialog"]').waitForExist();
    await expect($("fieldset*=This computer")).toHaveText(expect.stringContaining(UNAVAILABLE));
    expect(await $("button*=Remember the passphrase").isExisting()).toBe(false);
    await clickButton("Cancel");
    await $('[role="dialog"]').waitForExist({ reverse: true });

    await switchDatabase();
    await unlock(E2E_PASSPHRASE);
  });
});
