// OpenSSL advisory audit: `npm run audit:openssl`. See
// docs/development/dependency-audit.md.
//
// The Windows build links OpenSSL statically from vcpkg, pinned by the
// `builtin-baseline` (a microsoft/vcpkg commit) in
// scripts/windows/vcpkg/vcpkg.json (#25). Dependabot moves that pin but raises
// no security alerts for vcpkg, so this reads the OpenSSL version the pin
// resolves to (vcpkg's versions/baseline.json at that commit) and fails when
// any of OpenSSL's own advisories lists it as affected, whatever the severity.
// The advisories are the CVE records OpenSSL publishes for its website, in
// github.com/openssl/release-metadata (secjson/), read from one tarball of
// that repository. It also fails when vcpkg's master resolves OpenSSL to a
// newer version than the pin does.
//
// Once the version's release branch is past its end of life there
// (data.json), OpenSSL stops listing it in new advisories, so this warns, and
// fails on each advisory published since that affects the nearest supported
// branch below or above it, which share its code, until someone has checked
// the pinned version and recorded it in audit-openssl.json.
//
// It needs the network, like the other audits; without it, it fails saying
// what it couldn't fetch, never passes unchecked. No dependencies of its own.

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const MANIFEST = "scripts/windows/vcpkg/vcpkg.json";
const EXCEPTIONS_FILE = "audit-openssl.json";
const BASELINE_URL = (commit) =>
  `https://raw.githubusercontent.com/microsoft/vcpkg/${commit}/versions/baseline.json`;
const LATEST_BASELINE_URL = BASELINE_URL("master");
const ADVISORIES_URL =
  "https://codeload.github.com/openssl/release-metadata/tar.gz/refs/heads/main";
// Fewer records than this means the repository's layout changed under us.
const MIN_RECORDS = 100;

/** The vcpkg commit the manifest pins; throws unless OpenSSL comes from it alone. */
export function pinnedBaseline(manifest) {
  const deps = (manifest.dependencies ?? []).map((d) => (typeof d === "string" ? { name: d } : d));
  const openssl = deps.find((d) => d.name === "openssl");
  if (!openssl) throw new Error(`${MANIFEST}: has no openssl dependency`);
  // The version is read from the baseline, so nothing else may choose it.
  if (openssl["version>="] !== undefined || (manifest.overrides ?? []).length > 0)
    throw new Error(`${MANIFEST}: the audit reads OpenSSL's version from builtin-baseline alone`);
  const commit = manifest["builtin-baseline"];
  if (!/^[0-9a-f]{40}$/.test(commit ?? ""))
    throw new Error(`${MANIFEST}: builtin-baseline must be a full commit hash`);
  return commit;
}

/** OpenSSL's version in vcpkg's versions/baseline.json; throws unless it is 3 or later. */
export function opensslInBaseline(baseline) {
  const version = baseline?.default?.openssl?.baseline;
  const parsed = parseVersion(version ?? "");
  if (!parsed || parsed.suffix !== "" || parsed.numbers[0] < 3)
    throw new Error(`vcpkg's baseline names OpenSSL "${version}", not a 3.x.y or later version`);
  return version;
}

/**
 * `3.6.5` -> numbers and the letter suffix of the 0.9 and 1.x releases
 * (`1.1.1w`, `1.0.2za`). Null when unreadable.
 */
function parseVersion(text) {
  // shortcut: a pre-release (`0.9.7-beta3`, only in 0.9.x records) is compared as its release; fine while the pin is 3 or later
  const m = /^(\d+)\.(\d+)\.(\d+)([a-z]{0,2})(?:-[0-9A-Za-z.]+)?$/.exec(text);
  return m && { numbers: [Number(m[1]), Number(m[2]), Number(m[3])], suffix: m[4] };
}

function compare(a, b) {
  for (let i = 0; i < 3; i++) if (a.numbers[i] !== b.numbers[i]) return a.numbers[i] - b.numbers[i];
  // a < z < za: the letters run past z by growing.
  if (a.suffix.length !== b.suffix.length) return a.suffix.length - b.suffix.length;
  return a.suffix < b.suffix ? -1 : a.suffix > b.suffix ? 1 : 0;
}

/**
 * The OpenSSL versions a CVE record (CVE JSON 5) lists as affected, as
 * `{ from, lessThan?, lessThanOrEqual? }`. Throws on a record it can't read,
 * so a format change fails the audit.
 */
function affectedEntries(record) {
  const id = record?.cveMetadata?.cveId ?? "a record";
  const read = (text) => {
    const v = parseVersion(text);
    if (!v) throw new Error(`${id}: can't read "${text}" as an OpenSSL version`);
    return v;
  };
  const entries = [];
  for (const product of record?.containers?.cna?.affected ?? []) {
    // The FIPS object module of 2007 is a product of its own, which HoploDex doesn't link.
    if (product.product !== "OpenSSL") continue;
    if (product.defaultStatus !== "unaffected")
      throw new Error(`${id}: defaultStatus "${product.defaultStatus}" isn't handled`);
    for (const entry of product.versions ?? []) {
      if (entry.status !== "affected") continue;
      if (entry.version?.startsWith("fips-")) continue;
      entries.push({
        from: read(entry.version),
        lessThan: entry.lessThan === undefined ? undefined : read(entry.lessThan),
        lessThanOrEqual:
          entry.lessThanOrEqual === undefined ? undefined : read(entry.lessThanOrEqual),
      });
    }
  }
  return entries;
}

/** Whether a CVE record lists `version` of OpenSSL as affected. */
export function affects(record, version) {
  const pinned = parseVersion(version);
  return affectedEntries(record).some(({ from, lessThan, lessThanOrEqual }) => {
    if (compare(pinned, from) < 0) return false;
    if (lessThan) return compare(pinned, lessThan) < 0;
    if (lessThanOrEqual) return compare(pinned, lessThanOrEqual) <= 0;
    return compare(pinned, from) === 0;
  });
}

/** `3.6` -> `[3, 6]`. */
const branchOf = (text) => text.split(".").slice(0, 2).map(Number);
const branchName = (b) => b.join(".");
const compareBranch = (a, b) => a[0] - b[0] || a[1] - b[1];

/** A date in data.json (`01 Nov 2026`, `Oct 2027`) or ISO, as ms; throws when unreadable. */
function day(text, what) {
  const ms = Date.parse(/^\d{4}-/.test(text ?? "") ? text : `${text} UTC`);
  if (Number.isNaN(ms)) throw new Error(`${what}: can't read "${text}" as a date`);
  return ms;
}

/**
 * The supported branches next to `branch` in data.json, released and not past
 * their end of life on `today`: the nearest below and the nearest above.
 */
function neighbours(branch, releases, now) {
  const supported = Object.entries(releases ?? {})
    .filter(
      ([name, r]) =>
        day(r.release, `data.json ${name}.release`) <= now &&
        day(r.eol, `data.json ${name}.eol`) > now,
    )
    .map(([name]) => branchOf(name))
    .sort(compareBranch);
  const below = supported.filter((b) => compareBranch(b, branch) < 0).at(-1);
  const above = supported.find((b) => compareBranch(b, branch) > 0);
  return [below, above].filter(Boolean);
}

const EXCEPTION_KEYS = ["id", "reason", "review"];

function isDate(text) {
  if (typeof text !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(text)) return false;
  const date = new Date(`${text}T00:00:00Z`);
  return !Number.isNaN(date.getTime()) && date.toISOString().slice(0, 10) === text;
}

/** What is wrong with one exception, as `field: why` strings. */
function exceptionErrors(e) {
  if (e === null || typeof e !== "object" || Array.isArray(e)) return ["must be an object"];
  const errors = Object.keys(e)
    .filter((key) => !EXCEPTION_KEYS.includes(key))
    .map((key) => `${key}: unknown key`);
  if (typeof e.id !== "string" || !/^CVE-\d{4}-\d+$/.test(e.id))
    errors.push("id: must be a CVE id, CVE-YYYY-N");
  if (typeof e.reason !== "string" || e.reason.trim() === "")
    errors.push("reason: must be a non-empty string");
  if (!isDate(e.review)) errors.push("review: must be a real date, YYYY-MM-DD");
  return errors;
}

/**
 * The audit, as of `today` (`YYYY-MM-DD`): errors and warnings for the pinned
 * `version`, the `latest` version vcpkg's master resolves to, OpenSSL's CVE
 * `records`, its `releases` (data.json) and the `exceptions` file
 * (audit-openssl.json). Every error fails it.
 */
export function checkOpenSsl({ version, latest, records, releases, exceptions, today }) {
  if (records.length < MIN_RECORDS)
    throw new Error(`only ${records.length} advisories read; has OpenSSL's repository changed?`);
  const now = day(today, "today");
  const describe = (record) => {
    const cna = record.containers.cna;
    const severity = cna.metrics?.find((m) => m.other?.content?.text)?.other.content.text;
    const advisory = cna.references?.find((r) => r.tags?.includes("vendor-advisory"))?.url;
    return `${record.cveMetadata.cveId}${severity ? ` (${severity})` : ""}${advisory ? `: ${advisory}` : ""}`;
  };
  const errors = records
    .filter((record) => affects(record, version))
    .map(
      (record) =>
        `OpenSSL ${version} is affected by ${describe(record)}; move the pin in ${MANIFEST} to a vcpkg with a fixed OpenSSL`,
    );

  if (compare(parseVersion(latest), parseVersion(version)) > 0)
    errors.push(
      `vcpkg's master resolves OpenSSL to ${latest}, newer than the pinned ${version}: take Dependabot's baseline update, or move builtin-baseline in ${MANIFEST}`,
    );

  const list = [];
  if (exceptions === null || typeof exceptions !== "object" || Array.isArray(exceptions))
    errors.push(`${EXCEPTIONS_FILE}: must hold a JSON object`);
  else {
    for (const key of Object.keys(exceptions))
      if (key !== "//" && key !== "exceptions")
        errors.push(`${EXCEPTIONS_FILE}: ${key}: unknown key`);
    if (Array.isArray(exceptions.exceptions)) list.push(...exceptions.exceptions);
    else errors.push(`${EXCEPTIONS_FILE}: exceptions: must be an array`);
  }
  const valid = [];
  list.forEach((e, index) => {
    const problems = exceptionErrors(e);
    for (const problem of problems)
      errors.push(`${EXCEPTIONS_FILE} exceptions[${index}]: ${problem}`);
    if (problems.length === 0) valid.push({ ...e, index, matched: false });
  });

  const branch = branchOf(version);
  const eol = releases?.[branchName(branch)]?.eol;
  if (eol === undefined)
    throw new Error(
      `OpenSSL's release data (data.json) has no ${branchName(branch)} branch, so its support can't be checked; has OpenSSL's repository changed?`,
    );
  const since = day(eol, `data.json ${branchName(branch)}.eol`);
  const warnings = [];
  if (since <= now) {
    const near = neighbours(branch, releases, now);
    const names = near.map(branchName).join(" and ");
    warnings.push(
      `OpenSSL ${branchName(branch)} is out of support since ${eol}: a later advisory may not list ${version}, so the audit fails on each one that affects ${names || "a supported branch next to it"}; move the vcpkg pin to a supported branch when vcpkg has one`,
    );
    for (const record of records) {
      const id = record.cveMetadata.cveId;
      if (day(record.containers.cna.datePublic, `${id}: datePublic`) < since) continue;
      if (affects(record, version)) continue;
      const hit = near.filter((b) =>
        affectedEntries(record).some(({ from }) => compareBranch(from.numbers, b) === 0),
      );
      if (hit.length === 0) continue;
      const cleared = valid.filter((e) => e.id === id);
      for (const e of cleared) e.matched = true;
      if (cleared.some((e) => e.review >= today)) continue;
      errors.push(
        `${describe(record)} affects OpenSSL ${hit.map(branchName).join(" and ")}, published after ${branchName(branch)}'s end of life: check whether ${version} is affected, then move the pin in ${MANIFEST} or record the check in ${EXCEPTIONS_FILE}${cleared.length > 0 ? ` (its review date ${cleared.map((e) => e.review).join(", ")} has passed)` : ""}`,
      );
    }
  }
  for (const e of valid)
    if (!e.matched)
      errors.push(
        `${EXCEPTIONS_FILE} exceptions[${e.index}]: ${e.id} matches no advisory the audit would fail on: remove it`,
      );
  return { errors, warnings };
}

/** The files of a .tar.gz, keyed by their path below the archive's top folder. */
export function untar(gzipped) {
  const tar = gunzipSync(gzipped);
  const files = new Map();
  for (let at = 0; at + 512 <= tar.length;) {
    const header = tar.subarray(at, at + 512);
    if (header.every((byte) => byte === 0)) break;
    const field = (start, length) =>
      header.toString("utf8", start, start + length).replace(/\0.*$/s, "");
    const size = parseInt(field(124, 12).trim() || "0", 8);
    const type = field(156, 1);
    const prefix = field(345, 155);
    const name = prefix ? `${prefix}/${field(0, 100)}` : field(0, 100);
    if (type === "0" || type === "")
      files.set(name.replace(/^[^/]*\//, ""), tar.subarray(at + 512, at + 512 + size));
    at += 512 + Math.ceil(size / 512) * 512;
  }
  return files;
}

async function fetchOk(url) {
  let response;
  try {
    response = await fetch(url);
  } catch (error) {
    throw new Error(
      `couldn't fetch ${url} (the audit needs the network): ${error.cause?.message ?? error.message}`,
    );
  }
  if (!response.ok) throw new Error(`couldn't fetch ${url}: HTTP ${response.status}`);
  return Buffer.from(await response.arrayBuffer());
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const manifest = JSON.parse(readFileSync(path.join(repoRoot, MANIFEST), "utf8"));
    const exceptions = JSON.parse(readFileSync(path.join(repoRoot, EXCEPTIONS_FILE), "utf8"));
    const commit = pinnedBaseline(manifest);
    const version = opensslInBaseline(JSON.parse(await fetchOk(BASELINE_URL(commit))));
    const latest = opensslInBaseline(JSON.parse(await fetchOk(LATEST_BASELINE_URL)));
    const files = untar(await fetchOk(ADVISORIES_URL));
    const records = [...files]
      .filter(([name]) => /^secjson\/CVE-[\d-]+\.json$/.test(name))
      .map(([, bytes]) => JSON.parse(bytes.toString("utf8")));
    const releases = JSON.parse(files.get("data.json")?.toString("utf8") ?? "null");
    const { errors, warnings } = checkOpenSsl({
      version,
      latest,
      records,
      releases,
      exceptions,
      today: new Date().toISOString().slice(0, 10),
    });
    for (const warning of warnings) console.warn(`audit:openssl: warning: ${warning}`);
    if (errors.length > 0) {
      for (const error of errors) console.error(`audit:openssl: ${error}`);
      process.exit(1);
    }
    console.log(
      `audit:openssl: OpenSSL ${version} (vcpkg ${commit.slice(0, 12)}, as on vcpkg's master): none of OpenSSL's ${records.length} advisories lists it`,
    );
  } catch (error) {
    console.error(`audit:openssl: ${error.message}`);
    process.exit(1);
  }
}
