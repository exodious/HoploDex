import { $, $$, addFirearm, back, choose, expect, groupBy, listedNames } from "../support/ui";
import { openFirearm, search } from "../support/ui";
import { createDatabase } from "../support/ui";

/**
 * End-to-end coverage of User Story 2's acceptance scenarios (spec.md),
 * driven against the real built app through its embedded WebDriver server.
 * Seeds its own uniquely-named records rather than assuming an empty
 * collection. See e2e/support/ui.ts for why interactions go through page JS.
 */
describe("User Story 2 - Browse, Search, and Group", () => {
  // Each spec's session starts at the chooser with no databases (wdio.conf.ts).
  before(async () => {
    await createDatabase();
  });

  before(async () => {
    await addFirearm({
      make: "BrowseSig",
      model: "P226",
      caliber: "9mm-Browse",
      type: "Handgun",
      serial: "BR-001",
      finish: "distinctivefinishnitrocarb",
    });
    await back();
    await addFirearm({
      make: "BrowseRuger",
      model: "10-22-Browse",
      caliber: ".22 LR",
      type: "Rifle",
      serial: "BR-002",
      notes: "distinctivenotecrackedhandle",
    });
    await back();
    await addFirearm({
      make: "BrowseMossberg",
      model: "500-Browse",
      caliber: "9mm-Browse",
      type: "Shotgun",
      serial: "BR-003",
    });
    await back();
  });

  it("shows the same firearms in both list and tile view (Scenario 1)", async () => {
    await search("Browse");
    const inList = await listedNames();
    expect(inList).toEqual(
      expect.arrayContaining([
        "BrowseSig P226",
        "BrowseRuger 10-22-Browse",
        "BrowseMossberg 500-Browse",
      ]),
    );

    await choose("Tiles");
    await expect($$(".hd-tile")).toBeElementsArrayOfSize(3);
    expect((await listedNames()).sort()).toEqual([...inList].sort());

    await choose("List");
    await expect($(".hd-table")).toExist();
  });

  it("groups firearms by type (Scenario 2)", async () => {
    await groupBy("Type");

    await expect($("h2.hd-group__title*=Handgun")).toExist();
    await expect($("h2.hd-group__title*=Rifle")).toExist();
    await expect($("h2.hd-group__title*=Shotgun")).toExist();

    await groupBy("None");
    await expect($$("h2.hd-group__title")).toBeElementsArrayOfSize(0);
  });

  it("searches free-form notes and returns only the matching firearm (Scenario 3)", async () => {
    await search("distinctivenotecrackedhandle");
    expect(await listedNames()).toEqual(["BrowseRuger 10-22-Browse"]);
  });

  it("searches a caliber value shared by multiple firearms (Scenario 4)", async () => {
    await search("9mm-Browse");
    const names = await listedNames();
    expect(names).toEqual(expect.arrayContaining(["BrowseSig P226", "BrowseMossberg 500-Browse"]));
    expect(names).not.toContain("BrowseRuger 10-22-Browse");
  });

  it("keeps the search when returning from a record", async () => {
    // Regression: opening a record used to discard the search, grouping,
    // and view the user had set up.
    await openFirearm("BrowseSig P226");
    await back();
    const value = await $('input[type="search"]').getValue();
    expect(value).toBe("9mm-Browse");
    expect(await listedNames()).not.toContain("BrowseRuger 10-22-Browse");
  });

  it("clearing search shows the full collection again (Scenario 5)", async () => {
    await search("");
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
    await search("distinctivefinishnitrocarb");
    expect(await listedNames()).toEqual(["BrowseSig P226"]);
    await search("");
  });
});
