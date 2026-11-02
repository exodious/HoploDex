// The attack-surface list check (scripts/attack-surface.mjs): its rules on
// small lists, then the repository's own list.
import { describe, expect, it } from "vitest";
import {
  cargoFeatures,
  check,
  checkList,
  globToRegExp,
  parseList,
  unscopedCommands,
} from "./attack-surface.mjs";

const LIST = [
  "# Attack surface",
  "",
  "## ipc-commands: what the web view can call",
  "",
  "- **Code:** `src-tauri/src/main.rs` (`generate_handler!`, built in `setup()`); `src-tauri/src/commands/**`",
  "- **Ask:** is `open_database` checked?",
  "",
  "## not-surface: checked and left out",
  "",
  "- **Code:** `src-tauri/tests/**`",
].join("\n");

const FILES = [
  "src-tauri/src/main.rs",
  "src-tauri/src/commands/databases.rs",
  "src-tauri/tests/a_test.rs",
  "src/App.tsx",
];

function messages(text: string, files = FILES, names: { kind: string; name: string }[] = []) {
  return checkList(text, files, names).map((p: { message: string }) => p.message);
}

describe("attack-surface list", () => {
  it("reads each entry's id and Code paths, leaving out notes in parentheses", () => {
    expect(parseList(LIST)).toEqual([
      {
        id: "ipc-commands",
        line: 3,
        code: { line: 5, paths: ["src-tauri/src/main.rs", "src-tauri/src/commands/**"] },
      },
      { id: "not-surface", line: 8, code: { line: 10, paths: ["src-tauri/tests/**"] } },
    ]);
  });

  it("matches `*` within a folder and `**` across folders", () => {
    expect(globToRegExp("src/**/*.test.*").test("src/a/b/c.test.ts")).toBe(true);
    expect(globToRegExp("scripts/windows/setup-*.ps1").test("scripts/windows/setup-user.ps1")).toBe(
      true,
    );
    expect(globToRegExp("scripts/*.sh").test("scripts/macos/a.sh")).toBe(false);
    expect(globToRegExp("vite.config.ts").test("viteXconfigXts")).toBe(false);
  });

  it("accepts a list that places every production file", () => {
    expect(messages(LIST)).toEqual([]);
  });

  it("(a) refuses a path or glob that matches no file", () => {
    const moved = LIST.replace("src-tauri/src/main.rs", "src-tauri/src/old_main.rs");
    expect(messages(moved, [...FILES])).toContain(
      "`src-tauri/src/old_main.rs` (in `ipc-commands`) matches no file",
    );
  });

  it("(b) refuses a production file no entry claims, but not a frontend file", () => {
    expect(messages(LIST, [...FILES, "src-tauri/src/new.rs", ".github/workflows/ci.yml"])).toEqual([
      "`src-tauri/src/new.rs` is in no entry's **Code**: place it, in `not-surface` if it is none",
      "`.github/workflows/ci.yml` is in no entry's **Code**: place it, in `not-surface` if it is none",
    ]);
  });

  it("(c) refuses an unscoped command or Cargo feature the list doesn't name", () => {
    expect(
      messages(LIST, FILES, [
        { kind: "unscoped command", name: "open_database" },
        { kind: "Cargo feature", name: "test-kdf" },
      ]),
    ).toEqual(["Cargo feature `test-kdf` isn't named in the list"]);
  });

  it("refuses a repeated id and an entry with no Code paths", () => {
    const text = `${LIST}\n\n## not-surface: again\n\n- **Ask:** nothing`;
    expect(messages(text)).toEqual([
      "surface id `not-surface` is used twice",
      "`not-surface` has no **Code** paths",
    ]);
  });

  it("reads UNSCOPED_COMMANDS and the Cargo features", () => {
    expect(
      unscopedCommands('pub const UNSCOPED_COMMANDS: &[&str] = &[\n    "a",\n    "b_c",\n];'),
    ).toEqual(["a", "b_c"]);
    expect(
      cargoFeatures(
        '[features]\n# note\ne2e = ["x"]\ntest-kdf = []\n\n[dependencies]\nfoo = "1"\n',
      ),
    ).toEqual(["e2e", "test-kdf"]);
  });

  it("finds nothing wrong with docs/attack-surface.md", { timeout: 30_000 }, () => {
    expect(check()).toEqual([]);
  });
});
