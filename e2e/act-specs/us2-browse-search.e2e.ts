import "@wdio/ai-service";
import { $, $$, browser, createDatabase, expect, listedNames, settle } from "../support/ui";

/**
 * #89 trial: e2e/specs/us2-browse-search.e2e.ts with its interaction steps
 * written as `browser.act()` instructions (WebdriverIO 10's
 * `@wdio/ai-service`) and its assertions kept as they are. Run with
 * `npm run test:e2e -- --act` (e2e/wdio.act.conf.ts). The recorded steps are
 * in __act__/us2-browse-search.e2e.ts.json.
 */

// Each act() is followed by the harness's settle(), as every ui.ts helper
// does. act() calls are direct statements with a literal instruction, the
// shape `npx wdio-ai eject` replaces; those in the before hook have an `id`,
// since a hook has no test title to key them by.

describe("User Story 2 - Browse, Search, and Group (act)", () => {
  before(async () => {
    await createDatabase();
  });

  before(async () => {
    await browser.act(
      `Add a firearm: make "BrowseSig", model "P226", type Handgun, caliber "9mm-Browse", serial number "BR-001", and finish "distinctivefinishnitrocarb" (the finish is in the Physical details group). Save it.`,
      { id: "add BrowseSig" },
    );
    await settle();
    await browser.act("Go back to the collection", { id: "back after BrowseSig" });
    await settle();
    await browser.act(
      `Add a firearm: make "BrowseRuger", model "10-22-Browse", type Rifle, caliber ".22 LR", serial number "BR-002", and notes "distinctivenotecrackedhandle". Save it.`,
      { id: "add BrowseRuger" },
    );
    await settle();
    await browser.act("Go back to the collection", { id: "back after BrowseRuger" });
    await settle();
    await browser.act(
      `Add a firearm: make "BrowseMossberg", model "500-Browse", type Shotgun, caliber "9mm-Browse", serial number "BR-003". Save it.`,
      { id: "add BrowseMossberg" },
    );
    await settle();
    await browser.act("Go back to the collection", { id: "back after BrowseMossberg" });
    await settle();
  });

  it("shows the same firearms in both list and tile view (Scenario 1)", async () => {
    await browser.act('Search the collection for "Browse"');
    await settle();
    const inList = await listedNames();
    expect(inList).toEqual(
      expect.arrayContaining([
        "BrowseSig P226",
        "BrowseRuger 10-22-Browse",
        "BrowseMossberg 500-Browse",
      ]),
    );

    await browser.act("Show the collection as tiles");

    await settle();
    await expect($$(".hd-tile")).toBeElementsArrayOfSize(3);
    expect((await listedNames()).sort()).toEqual([...inList].sort());

    await browser.act("Show the collection as a list");

    await settle();
    await expect($(".hd-table")).toExist();
  });

  it("groups firearms by type (Scenario 2)", async () => {
    await browser.act("Group the collection by type");
    await settle();

    await expect($("h2.hd-group__title*=Handgun")).toExist();
    await expect($("h2.hd-group__title*=Rifle")).toExist();
    await expect($("h2.hd-group__title*=Shotgun")).toExist();

    await browser.act("Stop grouping the collection");

    await settle();
    await expect($$("h2.hd-group__title")).toBeElementsArrayOfSize(0);
  });

  it("searches free-form notes and returns only the matching firearm (Scenario 3)", async () => {
    await browser.act('Search the collection for "distinctivenotecrackedhandle"');
    await settle();
    expect(await listedNames()).toEqual(["BrowseRuger 10-22-Browse"]);
  });

  it("searches a caliber value shared by multiple firearms (Scenario 4)", async () => {
    await browser.act('Search the collection for "9mm-Browse"');
    await settle();
    const names = await listedNames();
    expect(names).toEqual(expect.arrayContaining(["BrowseSig P226", "BrowseMossberg 500-Browse"]));
    expect(names).not.toContain("BrowseRuger 10-22-Browse");
  });

  it("keeps the search when returning from a record", async () => {
    await browser.act('Open the record of "BrowseSig P226"');
    await settle();
    await browser.act("Go back to the collection");
    await settle();
    const value = await $('input[type="search"]').getValue();
    expect(value).toBe("9mm-Browse");
    expect(await listedNames()).not.toContain("BrowseRuger 10-22-Browse");
  });

  it("clearing search shows the full collection again (Scenario 5)", async () => {
    await browser.act("Clear the collection search");
    await settle();
    const names = await listedNames();
    expect(names.length).toBeGreaterThanOrEqual(3);
    expect(names).toEqual(
      expect.arrayContaining([
        "BrowseSig P226",
        "BrowseRuger 10-22-Browse",
        "BrowseMossberg 500-Browse",
      ]),
    );
  });

  it("finds a firearm by a word in its finish (US1 Scenario 17)", async () => {
    await browser.act('Search the collection for "distinctivefinishnitrocarb"');
    await settle();
    expect(await listedNames()).toEqual(["BrowseSig P226"]);
    await browser.act("Clear the collection search");
    await settle();
  });
});
