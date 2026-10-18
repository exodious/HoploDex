// The notices generator and its command line: --check, --all-targets, --out
// (010 research.md §5, §6, §10, §13; 010 contracts/notices.md).
//
// `buildNotices()` makes one target's notices document and collects every
// failure; `renderJson()` and `renderText()` write it as `notices.json` and
// `NOTICES.txt`. The build's Vite plugin (vitePlugin.mjs) and the command line
// below both call it, so the two can't differ. Every path and both command
// runners are parameters so the tests need neither `cargo` nor the real
// `node_modules`; the defaults are the real repository's.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { isAllowed } from "./policy.mjs";
import {
  checkClarifications,
  checkExceptions,
  checkHandRecorded,
  checkNativeCode,
  readRecords,
  unusedAcrossTargets,
} from "./records.mjs";
import { alternativesOf, findTextsAmong, placeTexts } from "./licenseFiles.mjs";
import { crateShipped, npmShipped, runCargo, targetTriple } from "./shipped.mjs";
import { choose } from "./spdx.mjs";

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

/** The three targets `--all-targets` checks (010 contracts/notices.md, "Commands"). */
export const ALL_TARGETS = [
  "x86_64-unknown-linux-gnu",
  "x86_64-pc-windows-msvc",
  "aarch64-apple-darwin",
];

const PROGRAM_NAME = "HoploDex";
const COPYRIGHT_HOLDER = "exodious and the HoploDex contributors";
const FIRST_YEAR = 2026;
const FORMAT = 1;

/** The GPL statement `NOTICES.txt` opens with (010 contracts/notices.md, "dist/NOTICES.txt"). */
const STATEMENT = [
  "HoploDex is free software: you can redistribute it and/or modify it under the",
  "terms of the GNU General Public License as published by the Free Software",
  "Foundation, version 3 only. It is distributed in the hope that it will be",
  "useful, but WITHOUT ANY WARRANTY, to the extent permitted by law; without even",
  "the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.",
  "See the GNU General Public License below for more details.",
].join("\n");

const RULE = "=".repeat(78);
const DASHES = "-".repeat(78);

/** `git log`'s output for `root`, or "" when there is no git. */
function realGit(root) {
  return (args) =>
    execFileSync("git", args, { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] });
}

/**
 * The program block compiled into the interface and repeated in `notices.json`
 * (010 data-model.md, "Program block"; 010 research.md §13). `git(args)` returns a
 * git command's stdout and throws, or returns "", when there is no git; `now`
 * is then the build's own year.
 */
export function programBlock({ root = REPO, git = realGit(root), now = new Date() } = {}) {
  const pkg = JSON.parse(readFileSync(path.join(root, "package.json"), "utf8"));
  let year;
  try {
    year = /^(\d{4})-/.exec(String(git(["log", "-1", "--format=%cs"])).trim())?.[1];
  } catch {
    year = undefined;
  }
  const last = Math.max(FIRST_YEAR, year === undefined ? now.getUTCFullYear() : Number(year));
  const url = typeof pkg.repository === "string" ? pkg.repository : pkg.repository?.url;
  return {
    name: PROGRAM_NAME,
    version: pkg.version,
    copyrightYears: last === FIRST_YEAR ? `${FIRST_YEAR}` : `${FIRST_YEAR}–${last}`,
    copyrightHolder: COPYRIGHT_HOLDER,
    sourceUrl: String(url ?? "")
      .replace(/^git\+/, "")
      .replace(/\.git$/, ""),
  };
}

/** `buildNotices`' inputs with the real repository's defaults filled in. */
export function resolveInputs(options = {}) {
  const root = options.root ?? REPO;
  const env = options.env ?? process.env;
  return {
    root,
    lockFile: options.lockFile ?? path.join(root, "package-lock.json"),
    modulesDir: options.modulesDir ?? path.join(root, "node_modules"),
    licensesDir: options.licensesDir ?? path.join(root, "licenses"),
    licenseFile: options.licenseFile ?? path.join(root, "LICENSE"),
    cargo: options.cargo ?? runCargo(root),
    git: options.git ?? realGit(root),
    now: options.now ?? new Date(),
    env,
  };
}

/**
 * A crate's `license` as an SPDX expression: older crates write a choice with
 * a slash (`MIT/Apache-2.0`), which cargo and cargo-deny read as `OR`.
 */
const spdxOf = (license) =>
  typeof license === "string" ? license.replace(/\s*\/\s*/g, " OR ") : license;

/**
 * Whether the package or crate in `item.folder` ships a text for its license.
 * An exception is for the version that doesn't; another version of the same
 * name (`zune-core` 0.5 beside 0.4) uses its own files.
 */
function shipsOwnText(item, declared, chosen, label) {
  if (!existsSync(item.folder)) return false;
  try {
    const alternatives = alternativesOf(declared, chosen, item.name);
    return findTextsAmong(item.folder, alternatives, { declared, label }).failure === null;
  } catch {
    return false;
  }
}

const compare = (a, b) => (a < b ? -1 : a > b ? 1 : 0);

/** A failure on one line: a command's error, whatever it printed. */
function oneLine(error) {
  const text = String(error?.stderr || error?.message || error).trim();
  const flat = text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .join(" / ");
  return flat.length > 600 ? `${flat.slice(0, 600)}...` : flat;
}

/**
 * The npm packages that ship, each with its folder in `modulesDir`; one per
 * name and version. A shipped package the lockfile gives no version is a
 * failure, never skipped: its notice would drop out silently.
 */
function shippedNpm(lock, modulesDir) {
  const seen = new Set();
  const packages = [];
  const failures = [];
  for (const pkg of npmShipped(lock)) {
    if (pkg.version === undefined) {
      failures.push(
        `npm package ${pkg.name} (${pkg.location}): package-lock.json records no version for it. Run npm install to repair the lockfile.`,
      );
      continue;
    }
    const key = `${pkg.name}@${pkg.version}`;
    if (seen.has(key)) continue;
    seen.add(key);
    packages.push({
      name: pkg.name,
      version: pkg.version,
      folder: path.join(modulesDir, pkg.location.replace(/^node_modules\//, "")),
      license: pkg.license,
    });
  }
  return { packages, failures };
}

/** A notice with nothing to show fails: it would otherwise drop out of the document silently. */
const noText = (subject, file) =>
  `${subject}: no license text to show; a notice needs at least one. Check its record (${file}).`;

/** The text of a record's text entry (`{ file, text }` or `{ text }`). */
const textOf = (entry) => entry.text;

/**
 * Makes one target's notices (010 contracts/notices.md). Never throws for a
 * check that fails: every failure is a string in `failures`, and `document`
 * holds what was found. Returns `{ document, failures, warnings, used,
 * records }`; `used` (the records this target uses) and `records` serve the
 * `--all-targets` check across targets.
 *
 * `allTargets` is the `--all-targets` check of one target (OpenSSL's version
 * isn't read); `development: true` marks the document, for the development
 * server; `env` holds the variables a Windows check finds OpenSSL's headers
 * through (default: the process's; see `openSslIncludeDir` in records.mjs).
 */
export function buildNotices(options = {}) {
  const inputs = resolveInputs(options);
  const { root, lockFile, modulesDir, licensesDir, licenseFile, cargo, git, now } = inputs;
  const target = options.target ?? targetTriple({ argv: [], env: inputs.env });
  const failures = [];
  const warnings = [];
  const program = options.program ?? programBlock({ root, git, now });

  const records = readRecords(licensesDir);
  failures.push(...records.failures);

  let npm = [];
  try {
    const shipped = shippedNpm(JSON.parse(readFileSync(lockFile, "utf8")), modulesDir);
    npm = shipped.packages;
    failures.push(...shipped.failures);
  } catch (error) {
    failures.push(`npm packages: can't read ${lockFile} (${oneLine(error)}).`);
  }

  let crates = [];
  try {
    const shipped = crateShipped(target, { cargo, root });
    crates = shipped.crates.map((crate) => ({ ...crate, license: spdxOf(crate.license) }));
    failures.push(...shipped.failures);
  } catch (error) {
    failures.push(`crates (${target}): cargo failed (${oneLine(error)}).`);
  }

  // A committed text resolves against the folder above `licenses/`.
  const ctx = {
    root: path.dirname(licensesDir),
    triple: target,
    allTargets: options.allTargets === true,
    env: inputs.env,
    crates,
    npm,
  };
  const hand = checkHandRecorded(records.handRecorded, ctx);
  const exceptions = checkExceptions(records.exceptions, ctx);
  const clarifications = checkClarifications(records.clarifications, ctx);
  const native = checkNativeCode(records.nativeCode, ctx);
  for (const result of [hand, exceptions, clarifications, native]) {
    failures.push(...result.failures);
    warnings.push(...(result.warnings ?? []));
  }

  const components = [];

  const packaged = (item, kind) => {
    const word = kind === "crate" ? "crate" : "npm";
    const label =
      kind === "crate"
        ? `crate ${item.name} ${item.version} (${target})`
        : `npm ${item.name} ${item.version}`;
    const id = `${kind === "crate" ? "crate" : "npm"}:${item.name}`;
    const declared = item.license;
    if (typeof declared !== "string" || declared === "") {
      failures.push(
        `license: ${word} ${item.name} ${item.version} declares no license expression.`,
      );
      return;
    }
    let chosen;
    try {
      chosen = choose(declared, (license) => isAllowed(license, { package: item.name }));
    } catch (error) {
      failures.push(
        `license: ${word} ${item.name} ${item.version} declares "${declared}", which can't be read (${error.message}).`,
      );
      return;
    }
    if (chosen === undefined) {
      failures.push(
        `license: ${word} ${item.name} ${item.version} is under ${declared}, which isn't allowed.`,
      );
      return;
    }

    let source;
    let texts;
    if (clarifications.used.includes(id)) {
      // A clarification that failed has said why; the component waits for its fix.
      texts = clarifications.resolved[id];
      source = "clarified";
    } else if (exceptions.used.includes(id) && !shipsOwnText(item, declared, chosen, label)) {
      texts = exceptions.resolved[id];
      source = "exception";
      // A choice is used under the alternative the exception's text is a text of.
      const alternatives = alternativesOf(declared, chosen, item.name);
      if (alternatives.length > 1) {
        const placed = placeTexts(texts, alternatives);
        if (placed === undefined) {
          failures.push(
            `${label}: none of its exception's texts (${texts.map((t) => t.file).join(", ")}) is a text for ${alternatives.join(", ")}. Review the exception (licenses/exceptions.json).`,
          );
          return;
        }
        chosen = placed;
      }
    } else if (!existsSync(item.folder)) {
      failures.push(
        `${label}: its folder ${item.folder} doesn't exist${kind === "crate" ? " (cargo fetch should have fetched it)" : " (run npm ci)"}.`,
      );
      return;
    } else {
      let found;
      try {
        found = findTextsAmong(item.folder, alternativesOf(declared, chosen, item.name), {
          declared,
          label,
        });
        if (found.failure === null) chosen = found.license;
      } catch (error) {
        failures.push(`${label}: can't read its license files (${oneLine(error)}).`);
        return;
      }
      if (found.failure) {
        failures.push(found.failure);
        return;
      }
      texts = found.texts;
      source = "package";
    }
    if (texts === undefined) return;
    if (texts.length === 0) {
      failures.push(
        noText(
          label,
          source === "clarified" ? "licenses/clarify.json" : "licenses/exceptions.json",
        ),
      );
      return;
    }

    const component = {
      name: item.name,
      version: item.version,
      kind: kind === "crate" ? "crate" : item.name.startsWith("@fontsource") ? "font" : "npm",
      license: chosen,
    };
    if (/\sOR\s/.test(declared)) component.declared = declared;
    components.push({ ...component, texts: texts.map(textOf), source });
  };
  for (const item of npm) packaged(item, "npm");
  for (const item of crates) packaged(item, "crate");

  for (const entry of hand.components) {
    let chosen;
    try {
      chosen = choose(entry.license, (license) => isAllowed(license, { entry: entry.name }));
    } catch {
      chosen = undefined;
    }
    if (chosen === undefined) {
      failures.push(
        `license: hand-recorded "${entry.name}" is under ${entry.license}, which isn't allowed.`,
      );
      continue;
    }
    if (entry.texts.length === 0) {
      failures.push(noText(`hand-recorded "${entry.name}"`, "licenses/hand-recorded.json"));
      continue;
    }
    components.push({ ...entry, texts: entry.texts.map(textOf) });
  }

  components.sort(
    (a, b) =>
      compare(a.name.toLowerCase(), b.name.toLowerCase()) ||
      compare(a.version, b.version) ||
      compare(a.name, b.name),
  );

  // Each distinct text once, under an id that is stable within this document.
  const ids = new Map();
  const texts = {};
  const idOf = (text) => {
    let id = ids.get(text);
    if (id === undefined) {
      id = `t${ids.size + 1}`;
      ids.set(text, id);
      texts[id] = text;
    }
    return id;
  };
  const listed = components.map((c) => {
    const entry = {
      name: c.name,
      version: c.version,
      kind: c.kind,
      license: c.license,
    };
    if (c.declared !== undefined) entry.declared = c.declared;
    entry.texts = c.texts.map(idOf);
    entry.source = c.source;
    if (c.carrier !== undefined) entry.carrier = c.carrier;
    return entry;
  });

  const readWhole = (file, what) => {
    try {
      return readFileSync(file, "utf8");
    } catch (error) {
      failures.push(`${what}: can't be read (${oneLine(error)}).`);
      return "";
    }
  };
  const document = {
    format: FORMAT,
    program,
    target,
    gpl: readWhole(licenseFile, "LICENSE"),
    additionalPermission: readWhole(
      path.join(licensesDir, "additional-permission.txt"),
      "licenses/additional-permission.txt",
    ),
    components: listed,
    texts,
  };
  if (options.development === true) document.development = true;

  return {
    document,
    failures: [...new Set(failures)],
    warnings: [...new Set(warnings)],
    used: {
      handRecorded: hand.used,
      exceptions: exceptions.used,
      clarifications: clarifications.used,
      nativeCode: native.used,
    },
    records,
  };
}

/** `notices.json`: two-space indent and a final newline (010 contracts/notices.md, "The JSON is stable"). */
export function renderJson(document) {
  return `${JSON.stringify(document, null, 2)}\n`;
}

const trimEnd = (text) => text.replace(/\n+$/, "");

/** `NOTICES.txt` (010 contracts/notices.md, "dist/NOTICES.txt"): sections are separated by one blank line. */
export function renderText(document) {
  const p = document.program;
  const heading = (c) =>
    c.declared === undefined
      ? `${c.name} ${c.version}  (${c.license})`
      : `${c.name} ${c.version}  (${c.license}, used under ${c.license} of ${c.declared})`;
  const sections = [
    `${p.name} ${p.version}\nCopyright (C) ${p.copyrightYears} ${p.copyrightHolder}`,
    STATEMENT,
    `Source code: ${p.sourceUrl}`,
    trimEnd(document.additionalPermission),
    `This copy was built for ${document.target}.`,
    `${RULE}\nThird-party components\n${RULE}`,
    ...document.components.map(
      (c) =>
        `${DASHES}\n${heading(c)}\n${DASHES}\n${c.texts.map((id) => trimEnd(document.texts[id])).join("\n\n")}`,
    ),
    `${RULE}\nGNU General Public License, version 3\n${RULE}`,
    trimEnd(document.gpl),
  ];
  return `${sections.join("\n\n")}\n`;
}

/** The options `main` accepts, each with whether it takes a value. */
const OPTIONS = {
  "--check": false,
  "--all-targets": false,
  "--target": true,
  "--out": true,
};

/** Reads the command line: `{ check, allTargets, target, out }` or `{ error }`. */
function parseArguments(argv) {
  const parsed = { check: false, allTargets: false };
  for (let i = 0; i < argv.length; i++) {
    const [name, inline] = argv[i].startsWith("--") ? argv[i].split(/=(.*)/s, 2) : [argv[i]];
    if (!(name in OPTIONS)) return { error: `unknown option ${argv[i]}` };
    let value;
    if (OPTIONS[name]) {
      value = inline ?? argv[++i];
      if (value === undefined || value === "") return { error: `${name} needs a value` };
    } else if (inline !== undefined) {
      return { error: `${name} takes no value` };
    }
    if (name === "--check") parsed.check = true;
    else if (name === "--all-targets") parsed.allTargets = true;
    else if (name === "--target") parsed.target = value;
    else parsed.out = value;
  }
  if (!parsed.check && parsed.out === undefined) {
    return { error: "give --check or --out <dir>" };
  }
  if (parsed.allTargets && (parsed.out !== undefined || parsed.target !== undefined)) {
    return { error: "--all-targets can't be combined with --out or --target" };
  }
  return parsed;
}

const USAGE =
  "usage: notices.mjs --check [--all-targets | --target <triple>] | --out <dir> [--target <triple>]";

/**
 * The command line (010 contracts/notices.md, "Commands"). `argv` is
 * `process.argv.slice(2)`; `deps` holds `buildNotices`' inputs plus `env`,
 * `hostTriple` (a triple, or a function that finds it), `stdout(text)` and
 * `stderr(text)`. Returns the exit code: 0 when every check passed, else 1.
 */
export async function main(argv, deps = {}) {
  const {
    env = process.env,
    hostTriple,
    stdout = (text) => process.stdout.write(text),
    stderr = (text) => process.stderr.write(text),
    ...inputs
  } = deps;
  const args = parseArguments(argv);
  if (args.error) {
    stderr(`notices: ${args.error}\n${USAGE}\n`);
    return 1;
  }
  const build = (target, extra = {}) => buildNotices({ ...inputs, env, target, ...extra });

  if (args.allTargets) {
    const results = ALL_TARGETS.map((target) => build(target, { allTargets: true }));
    const failures = results.flatMap((result) => result.failures);
    failures.push(
      ...unusedAcrossTargets(
        results[0].records,
        results.map((result) => result.used),
      ),
    );
    const counts = results.map(
      (result, i) => `${ALL_TARGETS[i]} ${result.document.components.length}`,
    );
    // A warning on one target is a failure here when no target uses the entry, so none are printed.
    return report(counts.join(", "), [...new Set(failures)], [], stdout, stderr);
  }

  let target = args.target ?? env.TAURI_ENV_TARGET_TRIPLE;
  if (!target) {
    try {
      target =
        typeof hostTriple === "function"
          ? hostTriple()
          : (hostTriple ?? targetTriple({ argv: [], env: {} }));
    } catch (error) {
      stderr(`notices: can't tell the target (${oneLine(error)}); give --target.\n`);
      return 1;
    }
  }
  const result = build(target);
  const code = report(
    `${target} ${result.document.components.length}`,
    result.failures,
    result.warnings,
    stdout,
    stderr,
  );
  if (code === 0 && args.out !== undefined) {
    mkdirSync(args.out, { recursive: true });
    writeFileSync(path.join(args.out, "notices.json"), renderJson(result.document));
    writeFileSync(path.join(args.out, "NOTICES.txt"), renderText(result.document));
  }
  return code;
}

/**
 * Prints each warning (`warning: <message>`, on stderr), then the summary, or
 * every failure and then the summary; returns the exit code. A warning never
 * changes it.
 */
function report(counts, failures, warnings, stdout, stderr) {
  if (warnings.length > 0) stderr(`${warnings.map((w) => `warning: ${w}`).join("\n")}\n`);
  if (failures.length === 0) {
    stdout(`notices: ${counts} components; 0 failures\n`);
    return 0;
  }
  stderr(
    `${failures.join("\n")}\nnotices: ${counts} components; ${failures.length} ${failures.length === 1 ? "failure" : "failures"}\n`,
  );
  return 1;
}

// Run as a command (not when a test or the Vite plugin imports it).
if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
  main(process.argv.slice(2), {
    root: REPO,
    hostTriple: () => targetTriple({ argv: [], env: {} }),
  }).then(
    (code) => {
      process.exitCode = code;
    },
    (error) => {
      process.stderr.write(`notices: ${error?.stack ?? error}\n`);
      process.exitCode = 1;
    },
  );
}
