import { $, $$, addFirearm, back, choose, expect, groupBy, listedNames } from "../support/ui";
import { createDatabase, search } from "../support/ui";

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

  it("shows the same firearms in both list and tile view, groups them by type and searches notes (Scenarios 1-3, 5)", async () => {
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

    // Scenario 2: the type headings render (the grouping itself is a backend rule).
    await groupBy("Type");
    await expect($("h2.hd-group__title*=Handgun")).toExist();
    await expect($("h2.hd-group__title*=Rifle")).toExist();
    await expect($("h2.hd-group__title*=Shotgun")).toExist();
    await groupBy("None");
    await expect($$("h2.hd-group__title")).toBeElementsArrayOfSize(0);

    // Scenario 3: a notes search narrows the list to the matching firearm.
    await search("distinctivenotecrackedhandle");
    expect(await listedNames()).toEqual(["BrowseRuger 10-22-Browse"]);

    // Scenario 5: clearing the search shows the full collection again.
    await search("");
    expect(await listedNames()).toEqual(expect.arrayContaining(inList));
  });
});
