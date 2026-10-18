import { readFileSync } from "node:fs";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { AboutDialog } from "./AboutDialog";

// 010 SC-005: About calls no command. Every `invoke` goes through this mock.
vi.mock("@tauri-apps/api/core", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@tauri-apps/api/core")>()),
  invoke: vi.fn(),
}));

/** The program block the build compiles in with the Vite define
 * `__HOPLODEX_PROGRAM__` (010 contracts/notices.md, "The Vite plugin"). The
 * tests stub it as a global, so they hold whether or not the define is set. */
const PROGRAM = {
  name: "HoploDex",
  version: "0.1.0",
  copyrightYears: "2026",
  copyrightHolder: "exodious and the HoploDex contributors",
  sourceUrl: "https://github.com/exodious/HoploDex",
};

const GPL =
  "                    GNU GENERAL PUBLIC LICENSE\n" +
  "                       Version 3, 29 June 2007\n\n" +
  " Copyright (C) 2007 Free Software Foundation, Inc. <https://fsf.org/>\n";

const PERMISSION_HEADING = "Additional permission under GNU GPL version 3 section 7";
const PERMISSION =
  `${PERMISSION_HEADING}\n\n` +
  "If you modify this Program, or any covered work, by linking or combining it with " +
  "the Microsoft WebView2 loader, the licensors of this Program grant you additional " +
  "permission to convey the resulting work.";

const TEXTS: Record<string, string> = {
  t1: "MIT License\n\nCopyright (c) 2022 WorkOS\n\nPermission is hereby granted, free of charge.",
  t2: "Copyright (c) 2014, Kang Seonghoon.\n\nPermission is hereby granted, free of charge (chrono).",
  t3: "                                 Apache License\n                           Version 2.0, January 2004\n",
  t4: "cool-apache\nCopyright 2020 Example Corp.\n\nThis product includes software developed at\nExample Corp. (https://example.com/).",
  t5: "Open Database License (ODC-BY 1.0)\n\nYou are free to share, create and adapt this database.",
  t6: "The dictionary contains passwords from “Common Passwords” © 2019 Example Authors,\nused under the ODC-BY attribution requirement.",
  t7: "Copyright (c) 2008-2020 Zetetic LLC\nAll rights reserved.\n\nRedistribution and use in source and binary forms are permitted.",
};

/** In `notices.json`'s order: by name in lower case. The order is the
 * document's, and the dialog shows it as it is. */
const COMPONENTS = [
  {
    name: "@radix-ui/react-dialog",
    version: "1.1.15",
    kind: "npm",
    license: "MIT",
    texts: ["t1"],
    source: "package",
  },
  {
    name: "chrono",
    version: "0.4.42",
    kind: "crate",
    license: "MIT",
    declared: "MIT OR Apache-2.0",
    texts: ["t2"],
    source: "package",
  },
  {
    name: "cool-apache",
    version: "2.0.0",
    kind: "npm",
    license: "Apache-2.0",
    texts: ["t3", "t4"],
    source: "package",
  },
  {
    name: "SQLCipher",
    version: "4.14.0 community",
    kind: "compiled-in",
    license: "BSD-3-Clause",
    texts: ["t7"],
    source: "hand-recorded",
    carrier: { kind: "crate", name: "libsqlite3-sys", version: "0.38.1" },
  },
  {
    name: "zxcvbn-ts dictionary",
    version: "2024",
    kind: "data",
    license: "ODC-BY-1.0",
    texts: ["t5", "t6"],
    source: "hand-recorded",
    carrier: { kind: "npm", name: "@zxcvbn-ts/language-common", version: "3.0.4" },
  },
];

const DOCUMENT = {
  format: 1,
  program: PROGRAM,
  target: "x86_64-unknown-linux-gnu",
  gpl: GPL,
  additionalPermission: PERMISSION,
  components: COMPONENTS,
  texts: TEXTS,
};

const FAILED =
  "The license notices couldn't be loaded. They are also in the NOTICES.txt file that came with HoploDex.";

/** The text of an element's own text nodes holds `sentence`, wherever the
 * markup splits the paragraph. */
const hasText = (sentence: string) => (content: string) => content.includes(sentence);

const fetchNotices = vi.fn();

/** Answers the fetch with the fixture document. */
function answerWith(document: unknown = DOCUMENT) {
  fetchNotices.mockImplementation(() => Promise.resolve(new Response(JSON.stringify(document))));
}

/** An answer that needs no further reads, for the tests that fake the timers
 * and so can't wait for a real stream to be read. */
function plainResponse(document: unknown = DOCUMENT) {
  return {
    ok: true,
    status: 200,
    json: () => Promise.resolve(document),
    text: () => Promise.resolve(JSON.stringify(document)),
  } as Response;
}

/** A fetch that never answers: the dialog stays loading. */
function neverAnswer() {
  fetchNotices.mockReturnValue(new Promise(() => {}));
}

/** The opener owns the open state, as `SessionProvider` does. */
function Harness({ onOpenChange }: { onOpenChange?: (open: boolean) => void }) {
  const [open, setOpen] = useState(true);
  return (
    <AboutDialog
      open={open}
      onOpenChange={(next) => {
        onOpenChange?.(next);
        setOpen(next);
      }}
    />
  );
}

function renderAbout(onOpenChange?: (open: boolean) => void) {
  const user = userEvent.setup();
  render(<Harness onOpenChange={onOpenChange} />);
  return user;
}

const dialog = () => screen.getByRole("dialog", { name: "About HoploDex" });
/** A disclosure's button: named by its title and then its summary. */
const disclosure = (title: string) =>
  screen.getByRole("button", { name: (name) => name.startsWith(`${title} `) });
const licenseButton = () => disclosure("GNU General Public License, version 3");
const filterField = () => screen.getByRole("searchbox", { name: "Find a component" });
/** Waits for the document to be shown. */
const ready = () => screen.findByText("5 components");

beforeEach(() => {
  vi.stubGlobal("__HOPLODEX_PROGRAM__", PROGRAM);
  fetchNotices.mockReset();
  answerWith();
  vi.stubGlobal("fetch", fetchNotices);
  vi.mocked(invoke).mockReset();
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("010 US1 - AboutDialog's program block (010 US1-1, 010 FR-002, 010 FR-019)", () => {
  it("shows the name, version, copyright and both statements before the fetch resolves", () => {
    neverAnswer();
    renderAbout();

    expect(dialog()).toHaveClass("hd-dialog__content--lg");
    expect(within(dialog()).getByRole("heading", { level: 3, name: "HoploDex" })).toBeVisible();
    expect(screen.getByText(hasText("Version 0.1.0"))).toBeInTheDocument();
    expect(
      screen.getByText(hasText("Copyright © 2026 exodious and the HoploDex contributors")),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        hasText(
          "HoploDex is free software: you can share it and change it under the terms of the GNU General Public License, version 3 only, as published by the Free Software Foundation.",
        ),
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        hasText(
          "It comes with no warranty, to the extent permitted by law. See the license for details.",
        ),
      ),
    ).toBeInTheDocument();
    // Nothing of the document is shown yet, the additional permission among it.
    expect(screen.queryByText(PERMISSION_HEADING)).not.toBeInTheDocument();
    expect(dialog().querySelector(".hd-dialog__footer")).toBeNull();
  });

  it("takes the years and the holder from the compiled-in program block", () => {
    vi.stubGlobal("__HOPLODEX_PROGRAM__", {
      ...PROGRAM,
      version: "0.2.0",
      copyrightYears: "2026–2028",
    });
    neverAnswer();
    renderAbout();

    expect(screen.getByText(hasText("Version 0.2.0"))).toBeInTheDocument();
    expect(
      screen.getByText(hasText("Copyright © 2026–2028 exodious and the HoploDex contributors")),
    ).toBeInTheDocument();
  });

  it("shows the additional permission after the statements once loaded, its heading as a strong run-in", async () => {
    renderAbout();

    const heading = await screen.findByText(PERMISSION_HEADING, { selector: "strong" });
    expect(screen.getByText(hasText("the Microsoft WebView2 loader"))).toBeInTheDocument();
    const noWarranty = screen.getByText(hasText("It comes with no warranty"));
    expect(
      noWarranty.compareDocumentPosition(heading) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("gives the source address as selectable text in a monospace span, not a link (010 US1-6, 010 FR-006)", async () => {
    const open = vi.spyOn(window, "open").mockReturnValue(null);
    const user = renderAbout();
    await ready();

    expect(screen.getByText(hasText("Source code:"))).toBeInTheDocument();
    const address = screen.getByText("https://github.com/exodious/HoploDex");
    expect(address.tagName).toBe("SPAN");
    expect(address.closest("a")).toBeNull();
    expect(within(dialog()).queryByRole("link")).not.toBeInTheDocument();
    expect(dialog().querySelector("a")).toBeNull();

    // jsdom applies no style sheet, so the rule is read from the file: the
    // class on the span is the one that selects all and sets monospace.
    const css = readFileSync("src/features/app/about.css", "utf8");
    const rule = [...css.matchAll(/\.([\w-]+)\s*\{([^}]*)\}/g)].find(
      ([, name, body]) => address.classList.contains(name) && /user-select:\s*all/.test(body),
    );
    expect(rule, "a rule on the span's class with user-select: all").toBeDefined();
    expect(rule?.[2]).toMatch(/font-family:\s*var\(--font-mono\)/);

    await user.click(address);
    expect(open).not.toHaveBeenCalled();
    open.mockRestore();
  });
});

describe("010 US1 - AboutDialog's development line (010 contracts/ui.md §2)", () => {
  const DEVELOPMENT_LINE = "Development build: these notices aren't checked.";

  it("says in a muted line that a development build's notices aren't checked", async () => {
    answerWith({ ...DOCUMENT, development: true });
    renderAbout();
    await ready();

    const line = screen.getByText(DEVELOPMENT_LINE);
    expect(line.closest(".hd-about__program")).not.toBeNull();
    // jsdom applies no style sheet, so the rule is read from the file: muted is
    // the secondary ink, as the program block's other muted text is.
    const css = readFileSync("src/features/app/about.css", "utf8");
    const rule = [...css.matchAll(/\.([\w-]+)\s*\{([^}]*)\}/g)].find(([, name]) =>
      line.classList.contains(name),
    );
    expect(rule?.[2]).toMatch(/color:\s*var\(--ink-2\)/);
  });

  it("says nothing for a build's document, which has no development field", async () => {
    renderAbout();
    await ready();

    expect(screen.queryByText(DEVELOPMENT_LINE)).not.toBeInTheDocument();
  });

  it("says nothing for a document whose development is false", async () => {
    answerWith({ ...DOCUMENT, development: false });
    renderAbout();
    await ready();

    expect(screen.queryByText(DEVELOPMENT_LINE)).not.toBeInTheDocument();
  });

  it("says nothing while the notices load or when they fail", async () => {
    neverAnswer();
    const { unmount } = render(<Harness />);
    expect(screen.queryByText(DEVELOPMENT_LINE)).not.toBeInTheDocument();
    unmount();

    fetchNotices.mockRejectedValue(new TypeError("Failed to fetch"));
    renderAbout();
    await screen.findByText(FAILED);
    expect(screen.queryByText(DEVELOPMENT_LINE)).not.toBeInTheDocument();
  });
});

describe("010 US1 - AboutDialog's license (010 US1-3, 010 FR-003)", () => {
  it("is a disclosure titled GNU General Public License, version 3, closed until asked", async () => {
    renderAbout();
    await ready();

    const heading = screen.getByRole("heading", {
      level: 3,
      name: /^GNU General Public License, version 3/,
    });
    expect(heading).toContainElement(licenseButton());
    expect(licenseButton()).toHaveAccessibleName(
      "GNU General Public License, version 3 The full text of the license HoploDex is under.",
    );
    expect(licenseButton()).toHaveAttribute("aria-expanded", "false");
    expect(
      screen.queryByRole("region", { name: "GNU General Public License text" }),
    ).not.toBeInTheDocument();
  });

  it("opens to the whole document's gpl in a focusable pre named GNU General Public License text", async () => {
    const user = renderAbout();
    await ready();

    await user.click(licenseButton());

    const text = screen.getByRole("region", { name: "GNU General Public License text" });
    expect(text.tagName).toBe("PRE");
    expect(text).toHaveAttribute("tabindex", "0");
    expect(text.textContent).toBe(GPL);
    expect(licenseButton()).toHaveAttribute("aria-expanded", "true");
  });

  it("closes again", async () => {
    const user = renderAbout();
    await ready();
    await user.click(licenseButton());

    await user.click(licenseButton());

    expect(
      screen.queryByRole("region", { name: "GNU General Public License text" }),
    ).not.toBeInTheDocument();
  });
});

describe("010 US1 - AboutDialog's components (010 US1-4, 010 US1-5, 010 FR-005)", () => {
  it("heads the list Built with, and counts the components in a polite live region", async () => {
    renderAbout();

    const count = await ready();
    expect(count.closest('[aria-live="polite"], [role="status"]')).not.toBeNull();
    expect(screen.getByRole("heading", { level: 3, name: "Built with" })).toBeInTheDocument();
    expect(
      screen.getByText(
        hasText(
          "HoploDex includes these components by other authors. Each is used under the license shown, whose text follows its name.",
        ),
      ),
    ).toBeInTheDocument();
  });

  it("lists every component, closed, in the document's order, one h4 each", async () => {
    renderAbout();
    await ready();

    const names = screen.getAllByRole("heading", { level: 4 });
    expect(
      names.map((heading) => heading.querySelector(".hd-disclosure__title")?.textContent),
    ).toEqual(COMPONENTS.map((component) => component.name));
    expect(screen.queryAllByRole("region")).toHaveLength(0);
  });

  it("summarises a row as version · license, and a choice as used under the one chosen", async () => {
    renderAbout();
    await ready();

    expect(
      within(disclosure("@radix-ui/react-dialog")).getByText("1.1.15 · MIT"),
    ).toBeInTheDocument();
    expect(
      within(disclosure("chrono")).getByText("0.4.42 · MIT OR Apache-2.0, used under MIT"),
    ).toBeInTheDocument();
    expect(
      within(disclosure("SQLCipher")).getByText("4.14.0 community · BSD-3-Clause"),
    ).toBeInTheDocument();
  });

  it("renders a component's texts only when its row opens, in regions named for it", async () => {
    const user = renderAbout();
    await ready();
    expect(screen.queryByRole("region", { name: "chrono license text" })).not.toBeInTheDocument();
    expect(screen.queryByText(hasText("Kang Seonghoon"))).not.toBeInTheDocument();

    await user.click(disclosure("chrono"));

    const text = screen.getByRole("region", { name: "chrono license text" });
    expect(text.textContent).toBe(TEXTS.t2);
    expect(disclosure("chrono")).toHaveAttribute("aria-expanded", "true");

    await user.click(disclosure("chrono"));
    expect(screen.queryByRole("region", { name: "chrono license text" })).not.toBeInTheDocument();
  });

  // 010 US1-5
  it("shows a NOTICE after the license text, word for word", async () => {
    const user = renderAbout();
    await ready();

    await user.click(disclosure("cool-apache"));

    const texts = screen.getAllByRole("region", { name: "cool-apache license text" });
    expect(texts.map((text) => text.textContent)).toEqual([TEXTS.t3, TEXTS.t4]);
  });

  // 010 US1-5
  it("shows an attribution after the license text, word for word", async () => {
    const user = renderAbout();
    await ready();

    await user.click(disclosure("zxcvbn-ts dictionary"));

    const texts = screen.getAllByRole("region", { name: "zxcvbn-ts dictionary license text" });
    expect(texts.map((text) => text.textContent)).toEqual([TEXTS.t5, TEXTS.t6]);
  });

  it("says what carries a hand-recorded component, and only for one", async () => {
    const user = renderAbout();
    await ready();

    await user.click(disclosure("SQLCipher"));
    expect(screen.getByText("Built into libsqlite3-sys 0.38.1")).toBeInTheDocument();
    await user.click(disclosure("zxcvbn-ts dictionary"));
    expect(screen.getByText("Built into @zxcvbn-ts/language-common 3.0.4")).toBeInTheDocument();
    await user.click(disclosure("chrono"));
    expect(screen.getAllByText(hasText("Built into "))).toHaveLength(2);
  });

  it("narrows by a case-insensitive substring of the name, and counts n of N", async () => {
    const user = renderAbout();
    await ready();

    await user.type(filterField(), "SQL");

    expect(screen.getByText("1 of 5 components")).toBeInTheDocument();
    expect(
      screen.getByText("1 of 5 components").closest('[aria-live="polite"], [role="status"]'),
    ).not.toBeNull();
    expect(screen.getAllByRole("heading", { level: 4 })).toHaveLength(1);
    expect(disclosure("SQLCipher")).toBeInTheDocument();

    await user.clear(filterField());
    await user.type(filterField(), "RADIX");

    expect(screen.getByText("1 of 5 components")).toBeInTheDocument();
    expect(disclosure("@radix-ui/react-dialog")).toBeInTheDocument();
  });

  it("filters on every keystroke, with no delay", async () => {
    const user = renderAbout();
    await ready();

    await user.type(filterField(), "s");
    // SQLCipher, zxcvbn-ts dictionary
    expect(screen.getByText("2 of 5 components")).toBeInTheDocument();
    await user.type(filterField(), "q");
    expect(screen.getByText("1 of 5 components")).toBeInTheDocument();
  });

  it("matches the name only, not the version or the license", async () => {
    const user = renderAbout();
    await ready();

    await user.type(filterField(), "MIT");

    expect(screen.getByText("No component matches “MIT”.")).toBeInTheDocument();
  });

  it("trims the filter before it matches, and shows the trimmed text in the no-match line", async () => {
    const user = renderAbout();
    await ready();

    await user.type(filterField(), " sqlcipher ");

    expect(screen.getByText("1 of 5 components")).toBeInTheDocument();
    expect(disclosure("SQLCipher")).toBeInTheDocument();

    await user.clear(filterField());
    await user.type(filterField(), "  zzzz  ");

    expect(screen.getByText("No component matches “zzzz”.")).toBeInTheDocument();
  });

  it("reads 1 component for a one-component document, and 0 of 1 component when the filter leaves none", async () => {
    answerWith({ ...DOCUMENT, components: [COMPONENTS[0]] });
    const user = renderAbout();

    expect(await screen.findByText("1 component")).toBeInTheDocument();

    await user.type(filterField(), "zzzz");

    expect(screen.getByText("0 of 1 component")).toBeInTheDocument();
  });

  it("shows the no-match line in place of the list", async () => {
    const user = renderAbout();
    await ready();
    await user.click(disclosure("chrono"));

    await user.type(filterField(), "zzzz");

    expect(screen.getByText("No component matches “zzzz”.")).toBeInTheDocument();
    expect(screen.queryAllByRole("heading", { level: 4 })).toHaveLength(0);
    expect(screen.queryByRole("region", { name: "chrono license text" })).not.toBeInTheDocument();

    await user.clear(filterField());
    expect(screen.queryByText(hasText("No component matches"))).not.toBeInTheDocument();
    expect(screen.getByText("5 components")).toBeInTheDocument();
    expect(screen.getAllByRole("heading", { level: 4 })).toHaveLength(5);
  });
});

describe("010 US1 - AboutDialog's keyboard (010 contracts/ui.md §3)", () => {
  it("focuses the license disclosure's button once it opens", async () => {
    renderAbout();

    await ready();
    await waitFor(() => expect(licenseButton()).toHaveFocus());
  });

  it("clears a filter with Escape, and a second Escape closes the dialog", async () => {
    const onOpenChange = vi.fn();
    const user = renderAbout(onOpenChange);
    await ready();
    await user.type(filterField(), "sql");

    await user.keyboard("{Escape}");

    expect(filterField()).toHaveValue("");
    expect(screen.getByText("5 components")).toBeInTheDocument();
    expect(dialog()).toBeInTheDocument();
    expect(onOpenChange).not.toHaveBeenCalled();

    await user.keyboard("{Escape}");

    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("closes with Escape when the filter is empty", async () => {
    const onOpenChange = vi.fn();
    const user = renderAbout(onOpenChange);
    await ready();

    await user.keyboard("{Escape}");

    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
  });

  it("does nothing on Enter in the filter: the dialog is no form", async () => {
    const onOpenChange = vi.fn();
    const user = renderAbout(onOpenChange);
    await ready();

    await user.type(filterField(), "sql{Enter}");

    expect(dialog().querySelector("form")).toBeNull();
    expect(filterField()).toHaveValue("sql");
    expect(screen.getByText("1 of 5 components")).toBeInTheDocument();
    expect(onOpenChange).not.toHaveBeenCalled();
  });

  it("toggles a disclosure with Enter and with Space", async () => {
    const user = renderAbout();
    await ready();
    disclosure("chrono").focus();

    await user.keyboard("{Enter}");
    expect(disclosure("chrono")).toHaveAttribute("aria-expanded", "true");
    await user.keyboard(" ");
    expect(disclosure("chrono")).toHaveAttribute("aria-expanded", "false");
  });
});

describe("010 US1 - AboutDialog while it loads (010 contracts/ui.md §2 items 4 and 5)", () => {
  it("fetches the notices file of the app's own origin when it opens, and not before", async () => {
    const { rerender } = render(<AboutDialog open={false} onOpenChange={() => {}} />);
    expect(fetchNotices).not.toHaveBeenCalled();

    rerender(<AboutDialog open onOpenChange={() => {}} />);
    await ready();

    expect(fetchNotices).toHaveBeenCalledTimes(1);
    expect(String(fetchNotices.mock.calls[0][0])).toMatch(/^\.?\/?notices\.json$/);
  });

  it("says Loading the license notices… only after one second, with the program block shown", () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    neverAnswer();
    renderAbout();

    act(() => void vi.advanceTimersByTime(900));
    expect(screen.queryByText("Loading the license notices…")).not.toBeInTheDocument();

    act(() => void vi.advanceTimersByTime(200));
    expect(screen.getByText("Loading the license notices…")).toBeInTheDocument();
    expect(screen.getByText(hasText("Version 0.1.0"))).toBeInTheDocument();
    expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
    expect(screen.queryByText(FAILED)).not.toBeInTheDocument();
  });

  it("drops the loading line when the notices arrive", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    let answer: (response: Response) => void = () => {};
    fetchNotices.mockReturnValue(
      new Promise<Response>((resolve) => {
        answer = resolve;
      }),
    );
    renderAbout();
    act(() => void vi.advanceTimersByTime(1500));
    expect(screen.getByText("Loading the license notices…")).toBeInTheDocument();

    await act(async () => answer(plainResponse()));

    expect(screen.getByText("5 components")).toBeInTheDocument();
    expect(screen.queryByText("Loading the license notices…")).not.toBeInTheDocument();
  });

  it("shows no loading line for notices that arrive at once", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    fetchNotices.mockResolvedValue(plainResponse());
    renderAbout();

    await act(async () => {});
    expect(screen.getByText("5 components")).toBeInTheDocument();
    act(() => void vi.advanceTimersByTime(5000));

    expect(screen.queryByText("Loading the license notices…")).not.toBeInTheDocument();
  });

  it.each([
    ["a rejected fetch", () => fetchNotices.mockRejectedValue(new TypeError("Failed to fetch"))],
    [
      "a response that is not OK",
      () => fetchNotices.mockResolvedValue(new Response("not found", { status: 404 })),
    ],
    [
      "a notices file of format 2",
      () =>
        fetchNotices.mockResolvedValue(new Response(JSON.stringify({ ...DOCUMENT, format: 2 }))),
    ],
    [
      "a format 1 notices file whose component names an unknown text id",
      () =>
        fetchNotices.mockResolvedValue(
          new Response(
            JSON.stringify({
              ...DOCUMENT,
              components: [{ ...COMPONENTS[0], texts: ["t1", "missing"] }],
            }),
          ),
        ),
    ],
    [
      "a notices file that is not JSON",
      () => fetchNotices.mockResolvedValue(new Response("<html>not notices</html>")),
    ],
  ])(
    "says the notices couldn't be loaded after %s, and keeps the program block",
    async (_name, fail) => {
      fail();
      renderAbout();

      const message = await screen.findByText(FAILED);
      expect(message).toHaveClass("hd-banner--error");
      expect(screen.getByText(hasText("Version 0.1.0"))).toBeInTheDocument();
      expect(
        screen.getByText(hasText("HoploDex is free software: you can share it")),
      ).toBeInTheDocument();
      expect(screen.getByText("https://github.com/exodious/HoploDex")).toBeInTheDocument();
      expect(screen.queryByText("Loading the license notices…")).not.toBeInTheDocument();
      expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
      expect(screen.queryByText(PERMISSION_HEADING)).not.toBeInTheDocument();
      expect(
        screen.queryByRole("button", { name: /^GNU General Public License/ }),
      ).not.toBeInTheDocument();
    },
  );
});

describe("010 US1 - AboutDialog sends no command (010 SC-005)", () => {
  it("never calls invoke while rendering, filtering and opening rows", async () => {
    const user = renderAbout();
    await ready();

    await user.click(licenseButton());
    await user.type(filterField(), "sql");
    await user.click(disclosure("SQLCipher"));
    await user.clear(filterField());
    await user.click(disclosure("cool-apache"));

    expect(invoke).not.toHaveBeenCalled();
    expect(fetchNotices).toHaveBeenCalledTimes(1);
  });

  it("never calls invoke when the notices fail to load", async () => {
    fetchNotices.mockRejectedValue(new TypeError("Failed to fetch"));
    renderAbout();

    await screen.findByText(FAILED);

    expect(invoke).not.toHaveBeenCalled();
  });
});

describe("010 US1 - AboutDialog's narrow layout (010 FR-007)", () => {
  it("keeps one column at the app's 900 px breakpoint", () => {
    // jsdom lays nothing out, so the rule is read from the file; screenshot 102
    // is the narrow layout's visual check (010 plan.md, Test Levels).
    const css = readFileSync("src/features/app/about.css", "utf8");
    const narrow = css.match(/@media\s*\(max-width:\s*900px\)\s*\{([\s\S]*)\}\s*$/);
    expect(narrow, "a rule block at max-width: 900px").not.toBeNull();
    expect(narrow?.[1]).toMatch(/\.hd-about\s*\{[^}]*gap:/);
    // The base rule is a single column, and no rule makes it a grid or a row.
    expect(css).toMatch(/\.hd-about\s*\{[^}]*flex-direction:\s*column/);
    expect(css).not.toMatch(/grid-template-columns|flex-direction:\s*row/);
  });
});
