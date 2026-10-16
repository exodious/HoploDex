import {
  $,
  $$,
  addFirearm,
  back,
  browser,
  choose,
  expect,
  goTo,
  groupBy,
  listedNames,
} from "../support/ui";
import { createDatabase, invokeCommand, search } from "../support/ui";
import { resizeWindow } from "../support/screenshots";

/**
 * End-to-end coverage of User Story 2's acceptance scenarios (spec.md),
 * driven against the real built app through its embedded WebDriver server.
 * Seeds its own uniquely-named records rather than assuming an empty
 * collection. See e2e/support/ui.ts for why interactions go through page JS.
 */
describe("001 US2 - Browse, Search, and Group the Collection (was us2-browse-search)", () => {
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

  // Issue #51: one table for the whole list, so a column is where it was in
  // every group, and its heading row and the current group's title stay in view
  // under the top bar however far the list is scrolled. Sticky positioning is
  // the engine's, so this is checked in the real app.
  it("keeps one set of columns and a sticky heading and group title down a long grouped list (#51)", async () => {
    // Setup, not timed: thirty more firearms of the three types, through the
    // backend, so the list is longer than the window.
    const listed = (await invokeCommand("list_firearm_types", {})) as {
      types: { id: number; name: string }[];
    };
    const typeIds = ["Handgun", "Rifle", "Shotgun"].map(
      (name) => listed.types.find((type) => type.name === name)!.id,
    );
    for (let i = 0; i < 30; i++) {
      await invokeCommand("create_firearm", {
        input: {
          make: "ScrollMake",
          model: `Model ${String(i).padStart(2, "0")}`,
          serialNumber: `SCR-${i}`,
          noSerialAttested: false,
          caliber: i % 2 ? "9mm" : ".308 Winchester",
          firearmTypeId: typeIds[i % 3],
          status: "active",
          estimatedValue: 100 + i,
        },
      });
    }
    // The collection page loads its list when it opens.
    await goTo("Accessories");
    await goTo("Collection");
    await search("ScrollMake");
    await choose("List");
    await groupBy("Type");
    await expect($$(".hd-table")).toBeElementsArrayOfSize(1);
    await expect($$(".hd-group-row")).toBeElementsArrayOfSize(3);

    // The table rounds its corners with `contain: paint`, not `overflow: clip`,
    // which made every batch of rows and every scrolled frame of a 10,000-row
    // list about a tenth slower (collection.css, "List view").
    const clipping = await browser.execute(() => {
      const style = getComputedStyle(document.querySelector(".hd-table")!);
      return `${style.overflowX}/${style.contain}`;
    });
    expect(clipping).toBe("visible/paint");

    // Anything in a row that takes focus, such as a "Mounted on" link, scrolls
    // clear of both sticky rows as the name does (collection.css). The rows
    // here hold no mounts, so a link and a button are put in one to look at.
    const margins = await browser.execute(() => {
      const cell = document.querySelector<HTMLElement>(".hd-row td")!;
      const probes = ["a", "button"].map((tag) => cell.appendChild(document.createElement(tag)));
      const result = [document.querySelector(".hd-row__name")!, ...probes].map(
        (el) => getComputedStyle(el).scrollMarginTop,
      );
      probes.forEach((el) => el.remove());
      return result;
    });
    expect(parseFloat(margins[0])).toBeGreaterThan(100);
    expect(margins).toEqual([margins[0], margins[0], margins[0]]);

    // The Caliber column starts and ends in the same place in every group.
    const columns = await browser.execute(() =>
      [...document.querySelectorAll<HTMLElement>("th.hd-col--caliber, td.hd-col--caliber")].map(
        (cell) => {
          const box = cell.getBoundingClientRect();
          return `${Math.round(box.left)}+${Math.round(box.width)}`;
        },
      ),
    );
    expect(columns.length).toBeGreaterThan(20);
    expect(new Set(columns).size).toBe(1);

    // Scrolled into the second group, the heading row sits just under the top
    // bar and the second group's title just under that, each uncovered by the
    // rows passing beneath.
    const stuck = await browser.execute(() => {
      const titles = [...document.querySelectorAll<HTMLElement>(".hd-group-row")];
      const second = titles[1];
      const topbar = document.querySelector(".hd-topbar")!.getBoundingClientRect().bottom;
      const head = document.querySelector<HTMLElement>("th.hd-col--caliber")!;
      const headHeight = head.getBoundingClientRect().height;
      window.scrollTo({
        top: window.scrollY + second.getBoundingClientRect().top - topbar - headHeight + 150,
        behavior: "instant",
      });
      const x = head.getBoundingClientRect().left + 10;
      const at = (y: number) => document.elementFromPoint(x, y);
      return {
        topbar,
        headTop: head.getBoundingClientRect().top,
        headCovered: at(topbar + headHeight / 2)?.closest("th") !== head,
        groupAtTop: at(topbar + headHeight + 8)?.closest(".hd-group-row")?.textContent ?? null,
        secondTitle: second.textContent,
        scrolled: window.scrollY,
      };
    });
    expect(stuck.scrolled).toBeGreaterThan(300);
    expect(Math.abs(stuck.headTop - stuck.topbar)).toBeLessThanOrEqual(1);
    expect(stuck.headCovered).toBe(false);
    expect(stuck.groupAtTop).toBe(stuck.secondTitle);
  });

  // #108: the window's minimum width is 800px (main.rs); the top bar used to
  // run past it, so the page scrolled sideways. Last in the file because it
  // rewrites the tab counts in place to the widest ones a person could see.
  it("keeps the top bar and the page inside the window at its minimum width (#108)", async () => {
    // getWindowSize reports no height without screenshots' window manager
    // setup, so the size to restore is the page's own.
    const [width, height] = await browser.execute(() => [window.innerWidth, window.innerHeight]);
    const original = { width, height };
    await resizeWindow(800, original.height);
    try {
      const fit = await browser.execute(() => {
        for (const count of document.querySelectorAll(".hd-tab__count")) count.textContent = "9999";
        const insurance = [...document.querySelectorAll(".hd-tab")].find((tab) =>
          tab.textContent?.startsWith("Insurance"),
        )!;
        if (!insurance.querySelector(".hd-tab__alert")) {
          const alert = document.createElement("span");
          alert.className = "hd-tab__alert hd-num";
          alert.textContent = "9999";
          insurance.append(alert);
        }
        // The widest Coverage pill, in the first row: the table used to cut
        // it off at its right edge.
        const coverage = document.querySelector(".hd-table td.hd-col--coverage");
        coverage?.replaceChildren();
        const pill = document.createElement("span");
        pill.className = "hd-badge hd-badge--warn";
        const icon = document.createElementNS("http://www.w3.org/2000/svg", "svg");
        icon.setAttribute("width", "16");
        icon.setAttribute("height", "16");
        pill.append(icon, "Under-insured");
        coverage?.append(pill);
        const table = document.querySelector(".hd-table")!.getBoundingClientRect();
        const clipped = [...document.querySelectorAll<HTMLElement>(".hd-table td, .hd-table th")]
          .filter((cell) => cell.scrollWidth > cell.clientWidth)
          .map((cell) => `${cell.className}: ${cell.textContent?.slice(0, 30)}`);
        const inner = document.querySelector<HTMLElement>(".hd-topbar__inner")!;
        return {
          clipped,
          // The columns together fill the table (a group title that spanned
          // the hidden columns made the table add one of its own).
          headRight: [...document.querySelectorAll(".hd-table th")]
            .filter((th) => getComputedStyle(th).display !== "none")
            .reduce((right, th) => Math.max(right, th.getBoundingClientRect().right), 0),
          pillRight: pill.getBoundingClientRect().right,
          tableRight: table.right,
          innerWidth: window.innerWidth,
          pageWidth: document.documentElement.scrollWidth,
          barWidth: inner.scrollWidth,
          barClient: inner.clientWidth,
          wordmark: getComputedStyle(document.querySelector(".hd-brand__name")!).display,
        };
      });
      expect(fit.innerWidth).toBeLessThan(900);
      expect(fit.pageWidth).toBeLessThanOrEqual(fit.innerWidth);
      expect(fit.barWidth).toBeLessThanOrEqual(fit.barClient);
      expect(fit.wordmark).toBe("none");
      // And nothing in the collection table is cut off at that width.
      expect(fit.clipped).toEqual([]);
      expect(fit.pillRight).toBeLessThanOrEqual(fit.tableRight);
      expect(fit.headRight).toBeGreaterThanOrEqual(fit.tableRight - 2);
    } finally {
      await resizeWindow(original.width, original.height);
    }
  });
});
