import { $, $$, browser, expect, waitForChooser } from "../support/ui";

/**
 * The vendored WebDriver server (tauri-plugin-wdio-webdriver) returns an
 * element a script returns as a web element reference (#89). WebdriverIO 10's
 * `role/` selector finds elements that way over WebDriver classic, and so do
 * `wdio session`'s snapshot refs; it returned null before.
 */
describe("WebDriver server: elements returned by scripts", () => {
  before(async () => {
    await waitForChooser();
  });

  it("returns an element from a script as one later commands can use", async () => {
    const title = await browser.execute(() => document.querySelector(".hd-chooser__title"));
    const text = await $(title as unknown as WebdriverIO.Element).getText();
    expect(text.length).toBeGreaterThan(0);
  });

  it("returns a list of elements from a script", async () => {
    const buttons = (await browser.execute(() =>
      document.querySelectorAll("button"),
    )) as unknown as WebdriverIO.Element[];
    expect(buttons.length).toBeGreaterThan(0);
    expect(buttons.length).toBe((await $$("button").getElements()).length);
  });

  it("finds and clicks an element by role and accessible name", async () => {
    const create = $('role/button[name="Create a new database…"]');
    await expect(create).toExist();
    await create.click();
    await expect($('role/dialog[name="Create a database"]')).toExist();
  });
});
