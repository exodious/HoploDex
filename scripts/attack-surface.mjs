// The release security review's attack surface is defined once, in
// docs/attack-surface.md (constitution, "Development Workflow & Quality
// Gates"). This keeps that list from going stale. It fails when:
//
// (a) a **Code** path or glob matches no file, so a rename fails the pull
//     request that makes it;
// (b) a production file (`PRODUCTION`) is claimed by no entry's **Code**,
//     `not-surface` included, so a new backend file, workflow or capability
//     fails until someone places it;
// (c) a command in `UNSCOPED_COMMANDS`, or a feature in Cargo.toml's
//     `[features]`, isn't named (in backticks) anywhere in the list.
//
// It also checks that each entry's id is unique and that it has a **Code**
// line. It can't see new surface inside a file an entry already claims; the
// per-PR review and peer review catch that.
//
// `node scripts/attack-surface.mjs` lists what's wrong;
// `scripts/attack-surface.test.ts` runs it in Vitest.

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { walkedFiles } from "./citations.mjs";

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

export const LIST = "docs/attack-surface.md";

/**
 * What must be placed in the list, file by file: the backend, its build and
 * configuration, the workflows, the Flatpak and the build scripts. The
 * frontend isn't checked file by file: its risk is HTML sinks, which the
 * `webview-config` entry asks about.
 */
export const PRODUCTION = [
  "src-tauri/src/**",
  "src-tauri/build.rs",
  "src-tauri/Cargo.toml",
  "src-tauri/tauri.conf.json",
  "src-tauri/packaging/**",
  "src-tauri/capabilities/**",
  "src-tauri/vendor/**",
  ".github/**",
  "flatpak/**",
  "scripts/build-appimage.sh",
  "scripts/check-no-webdriver.mjs",
  "scripts/flatpak/build.sh",
  "scripts/flatpak/sources.mjs",
  "scripts/flatpak/contents.mjs",
  "scripts/licenses/vitePlugin.mjs",
  "vite.config.ts",
  "package.json",
];

/** `*` matches within a folder, `**` across folders; anything else is literal. */
export function globToRegExp(glob) {
  let source = "";
  for (let i = 0; i < glob.length; i++) {
    if (glob.startsWith("**", i)) {
      source += ".*";
      i++;
    } else if (glob[i] === "*") {
      source += "[^/]*";
    } else {
      source += glob[i].replace(/[.+?^${}()|[\]\\]/g, "\\$&");
    }
  }
  return new RegExp(`^${source}$`);
}

/**
 * The entries in the list's text: `## <id>: <title>` headings, each with the
 * paths and globs on its `- **Code:**` line. A note in parentheses isn't a
 * path.
 */
export function parseList(text) {
  const entries = [];
  const lines = text.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const heading = /^## ([a-z0-9]+(?:-[a-z0-9]+)*): /.exec(lines[i]);
    if (heading) {
      entries.push({ id: heading[1], line: i + 1, code: null });
      continue;
    }
    const code = /^- \*\*Code:\*\*(.*)$/.exec(lines[i]);
    if (code && entries.length > 0 && entries.at(-1).code === null) {
      // Parenthesized notes hold symbols (`COMMANDS`), not paths.
      let outside = code[1];
      for (let last; last !== outside;) {
        last = outside;
        outside = outside.replace(/\([^()]*\)/g, "");
      }
      entries.at(-1).code = {
        line: i + 1,
        paths: [...outside.matchAll(/`([^`]+)`/g)].map((m) => m[1]),
      };
    }
  }
  return entries;
}

/** The commands in `UNSCOPED_COMMANDS` (commands_list.rs). */
export function unscopedCommands(rust) {
  const block = /UNSCOPED_COMMANDS: &\[&str\] = &\[([^\]]*)\]/.exec(rust);
  if (!block) throw new Error("UNSCOPED_COMMANDS not found in commands_list.rs");
  return [...block[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

/** The feature names in Cargo.toml's `[features]` table. */
export function cargoFeatures(toml) {
  const table = /^\[features\]\n([\s\S]*?)(?=^\[)/m.exec(toml);
  if (!table) throw new Error("[features] not found in Cargo.toml");
  return [...table[1].matchAll(/^([A-Za-z0-9_-]+)\s*=/gm)].map((m) => m[1]);
}

/**
 * Every problem with the list `text`, against the repository's `files`
 * (relative paths) and the `names` it must mention, as `line: message`.
 */
export function checkList(text, files, names = []) {
  const problems = [];
  const entries = parseList(text);
  if (entries.length === 0) problems.push({ line: 1, message: "no `## <id>: <title>` entries" });
  const seen = new Set();
  const claims = [];
  for (const entry of entries) {
    if (seen.has(entry.id)) {
      problems.push({ line: entry.line, message: `surface id \`${entry.id}\` is used twice` });
    }
    seen.add(entry.id);
    if (!entry.code || entry.code.paths.length === 0) {
      problems.push({ line: entry.line, message: `\`${entry.id}\` has no **Code** paths` });
      continue;
    }
    for (const glob of entry.code.paths) {
      const regex = globToRegExp(glob);
      claims.push(regex);
      if (!files.some((file) => regex.test(file))) {
        problems.push({
          line: entry.code.line,
          message: `\`${glob}\` (in \`${entry.id}\`) matches no file`,
        });
      }
    }
  }
  const production = PRODUCTION.map(globToRegExp);
  for (const file of files) {
    if (production.some((r) => r.test(file)) && !claims.some((r) => r.test(file))) {
      problems.push({
        line: 1,
        message: `\`${file}\` is in no entry's **Code**: place it, in \`not-surface\` if it is none`,
      });
    }
  }
  for (const { kind, name } of names) {
    if (!text.includes(`\`${name}\``)) {
      problems.push({ line: 1, message: `${kind} \`${name}\` isn't named in the list` });
    }
  }
  return problems;
}

/**
 * The repository's files: tracked, and new ones git doesn't ignore; or every
 * file, less what the root `.gitignore` keeps out, where git can't read the
 * checkout (a copy, or a worktree in the dev container).
 */
export function repositoryFiles(root = ROOT) {
  let files;
  try {
    files = execFileSync(
      "git",
      ["-C", root, "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
      { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] },
    ).split("\0");
  } catch {
    return walkedFiles(root, [""]);
  }
  return files.filter((file) => file && fs.existsSync(path.join(root, file)));
}

/** Every problem with the repository's list, as `docs/attack-surface.md:<line>: message`. */
export function check(root = ROOT) {
  const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
  const names = [
    ...unscopedCommands(read("src-tauri/src/commands_list.rs")).map((name) => ({
      kind: "unscoped command",
      name,
    })),
    ...cargoFeatures(read("src-tauri/Cargo.toml")).map((name) => ({ kind: "Cargo feature", name })),
  ];
  return checkList(read(LIST), repositoryFiles(root), names).map(
    (p) => `${LIST}:${p.line}: ${p.message}`,
  );
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const report = check();
  for (const line of report) console.log(line);
  console.log(`${report.length} problem${report.length === 1 ? "" : "s"}`);
  process.exitCode = report.length > 0 ? 1 : 0;
}
