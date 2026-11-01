// npm deprecation audit: `npm run audit:deprecated`. See "Dependency audit" in
// DEVELOPMENT.md.
//
// Reads package-lock.json and reports every package whose lockfile entry
// carries a `deprecated` message. A deprecated package fails the audit unless
// audit-deprecated.json holds a valid exception for it (same package, version
// within its range, same scope), and a shipped direct dependency always fails.
// Scope is the license audit's definition (`dev`/`devOptional` is
// development-only; 010 research.md §5, §6), taken from npmPackages() in
// scripts/licenses/shipped.mjs so the two audits agree.
//
// No dependencies of its own, so the audit doesn't grow the tree it audits:
// the range matcher below is local (012 research.md §4).

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { npmPackages } from "./licenses/shipped.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const EXCEPTIONS_FILE = "audit-deprecated.json";

const PARTIAL =
  /^(\d+|[xX*])(?:\.(\d+|[xX*]))?(?:\.(\d+|[xX*]))?(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;

/** `10.2` -> `{ tuple: [10, 2, 0], n: 2 }`; `1.x` -> n 1; `*` -> n 0. Throws when unreadable. */
function parsePartial(text) {
  const m = PARTIAL.exec(text);
  if (!m) throw new Error(`can't read "${text}" as a version`);
  const parts = [m[1], m[2], m[3]];
  let n = 0;
  while (n < 3 && parts[n] !== undefined && /^\d+$/.test(parts[n])) n++;
  return { tuple: [0, 1, 2].map((i) => (i < n ? Number(parts[i]) : 0)), n };
}

function compare(a, b) {
  for (let i = 0; i < 3; i++) if (a[i] !== b[i]) return a[i] < b[i] ? -1 : 1;
  return 0;
}

/** The first `n` parts with the last one raised by one, the rest zero: `1.2` (n 2) -> `1.3.0`. */
function bump({ tuple, n }, at = n - 1) {
  return tuple.map((v, i) => (i < at ? v : i === at ? v + 1 : 0));
}

/** One comparator as `[op, tuple]` bounds that must all hold. */
function bounds(comparator) {
  const [, op, text] = /^(\^|~|>=|<=|>|<|=)?(.*)$/.exec(comparator);
  const p = parsePartial(text);
  if (p.n === 0) return [];
  const ge = [">=", p.tuple];
  if (op === ">=") return [ge];
  if (op === "<") return [["<", p.tuple]];
  if (op === ">") return [p.n === 3 ? [">", p.tuple] : [">=", bump(p)]];
  if (op === "<=") return [p.n === 3 ? ["<=", p.tuple] : ["<", bump(p)]];
  if (op === "^") {
    const first = p.tuple.slice(0, p.n).findIndex((v) => v !== 0);
    return [ge, ["<", bump(p, first === -1 ? p.n - 1 : first)]];
  }
  if (op === "~") return [ge, ["<", bump(p, p.n === 1 ? 0 : 1)]];
  return p.n === 3 ? [ge, ["<=", p.tuple]] : [ge, ["<", bump(p)]];
}

const HOLDS = {
  ">=": (c) => c >= 0,
  ">": (c) => c > 0,
  "<=": (c) => c <= 0,
  "<": (c) => c < 0,
};

/**
 * Whether `version` is within `range`: an exact version, `=`, `>`, `>=`, `<`,
 * `<=`, `^`, `~`, `*` or empty, partial versions (`10`, `1.x`), spaces for AND
 * and `||` for OR. Throws on anything else (012 research.md §4).
 */
export function satisfiesRange(version, range) {
  // shortcut: a prerelease is compared on major.minor.patch only; upgrade if a deprecated prerelease ever needs its own exception
  const v = parsePartial(version);
  if (v.n !== 3) throw new Error(`can't read "${version}" as a full version`);
  // map before some: every alternative is read, so one unreadable alternative throws (012 FR-011)
  return range
    .split("||")
    .map((alternative) =>
      alternative
        .split(/\s+/)
        .filter((c) => c !== "")
        .flatMap(bounds)
        .every(([op, tuple]) => HOLDS[op](compare(v.tuple, tuple))),
    )
    .some(Boolean);
}

const KEYS = ["package", "range", "scope", "reason", "via", "successor", "review"];

function isDate(text) {
  if (typeof text !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(text)) return false;
  const date = new Date(`${text}T00:00:00Z`);
  return !Number.isNaN(date.getTime()) && date.toISOString().slice(0, 10) === text;
}

/** What is wrong with one exception, as `field: why` strings (012 FR-011). */
function exceptionErrors(e) {
  if (e === null || typeof e !== "object" || Array.isArray(e)) return ["must be an object"];
  const errors = Object.keys(e)
    .filter((key) => !KEYS.includes(key))
    .map((key) => `${key}: unknown key`);
  for (const key of ["package", "range", "reason", "via"]) {
    // `range` may be empty (any version); the others may not.
    if (typeof e[key] !== "string" || (key !== "range" && e[key].trim() === "")) {
      errors.push(`${key}: must be ${key === "range" ? "a string" : "a non-empty string"}`);
    }
  }
  if (typeof e.range === "string") {
    try {
      satisfiesRange("0.0.0", e.range);
    } catch (error) {
      errors.push(`range: ${error.message}`);
    }
  }
  if (e.scope !== "shipped" && e.scope !== "dev") errors.push(`scope: must be "shipped" or "dev"`);
  if (e.successor !== null && typeof e.successor !== "string") {
    errors.push("successor: must be a string, or null when none is known");
  }
  if (!isDate(e.review)) errors.push("review: must be a real date, YYYY-MM-DD");
  return errors;
}

/**
 * Checks the lockfile against the exceptions file (012 data-model.md, "Rules,
 * in order"). `today` is `YYYY-MM-DD` (UTC). Returns `{ findings, problems }`:
 * a finding is `{ name, version, location, scope, direct, message, outcome:
 * "fails" | "excepted", review? }`; every `problems` entry fails the audit.
 */
export function checkDeprecated(lock, exceptionsFile, today) {
  const problems = [];
  const file = exceptionsFile;
  let list = [];
  if (file === null || typeof file !== "object" || Array.isArray(file)) {
    problems.push(`${EXCEPTIONS_FILE}: must hold a JSON object`);
  } else {
    for (const key of Object.keys(file)) {
      if (key !== "//" && key !== "exceptions")
        problems.push(`${EXCEPTIONS_FILE}: ${key}: unknown key`);
    }
    if (Array.isArray(file.exceptions)) list = file.exceptions;
    else problems.push(`${EXCEPTIONS_FILE}: exceptions: must be an array`);
  }

  // An invalid exception is reported and never matches.
  const valid = [];
  list.forEach((e, index) => {
    const errors = exceptionErrors(e);
    for (const error of errors) problems.push(`${EXCEPTIONS_FILE} exceptions[${index}]: ${error}`);
    if (errors.length === 0) valid.push({ ...e, index, matched: false });
  });

  const root = lock.packages[""] ?? {};
  const findings = [];
  for (const pkg of npmPackages(lock)) {
    const message = lock.packages[pkg.location].deprecated;
    if (typeof message !== "string" || message === "") continue;
    const scope = pkg.development ? "dev" : "shipped";
    const key = pkg.location.slice(pkg.location.lastIndexOf("node_modules/") + 13);
    const direct =
      pkg.location === `node_modules/${key}` &&
      (scope === "shipped" ? root.dependencies : root.devDependencies)?.[key] !== undefined;
    const finding = { ...pkg, scope, direct, message, outcome: "fails" };
    delete finding.license;
    delete finding.development;
    findings.push(finding);

    const id = `${pkg.name}@${pkg.version} at ${pkg.location}`;
    const what =
      scope === "dev"
        ? `is development-only and deprecated: ${message}`
        : `${direct ? "ships in the app as a direct dependency" : "ships in the app"} and is deprecated: ${message}`;
    if (scope === "shipped" && direct) {
      problems.push(`${id} ${what}. Replace it: a shipped direct dependency can't be excepted.`);
      continue;
    }
    const matches = valid.filter((e) => {
      if (e.package !== pkg.name || e.scope !== scope) return false;
      try {
        return satisfiesRange(pkg.version, e.range);
      } catch {
        return false;
      }
    });
    if (matches.length === 0) {
      problems.push(
        `${id} ${what}. Replace it, or add an exception to ${EXCEPTIONS_FILE} (DEVELOPMENT.md, "Dependency audit").`,
      );
      continue;
    }
    // Every match counts as matched; any unexpired one excepts the finding. An
    // expired one is reported once, below, not once per finding.
    for (const e of matches) e.matched = true;
    const live = matches.filter((e) => e.review >= today).map((e) => e.review);
    if (live.length > 0)
      Object.assign(finding, { outcome: "excepted", review: live.sort().at(-1) });
  }

  for (const e of valid) {
    const id = `${EXCEPTIONS_FILE} exceptions[${e.index}] (${e.package} ${JSON.stringify(e.range)}, ${e.scope})`;
    if (!e.matched) problems.push(`${id} matches no deprecated package: remove the exception.`);
    else if (e.review < today) {
      problems.push(
        `${id} expired on ${e.review}: review it. Replace the package, or record a new date with the reason.`,
      );
    }
  }
  return { findings, problems };
}

function readJson(file) {
  return JSON.parse(readFileSync(path.join(repoRoot, file), "utf8"));
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  let exceptionsFile = { exceptions: [] };
  try {
    exceptionsFile = readJson(EXCEPTIONS_FILE);
  } catch (error) {
    if (error.code !== "ENOENT") {
      console.error(`${EXCEPTIONS_FILE}: can't read it (${error.message})`);
      process.exit(1);
    }
  }
  const today = new Date().toISOString().slice(0, 10);
  const { findings, problems } = checkDeprecated(
    readJson("package-lock.json"),
    exceptionsFile,
    today,
  );
  for (const f of findings) {
    const outcome = f.outcome === "excepted" ? `excepted until ${f.review}` : "fails";
    console.log(`${f.scope}  ${f.name}@${f.version}  ${f.location}  ${outcome}\n    ${f.message}`);
  }
  if (problems.length > 0) {
    console.error(`\n${problems.length} problem(s) with deprecated npm packages:`);
    for (const problem of problems) console.error(`  ${problem}`);
    process.exit(1);
  }
  console.log(
    `npm deprecations ok: ${findings.length === 0 ? "no deprecated packages" : `${findings.length} deprecated, all excepted`}.`,
  );
}
