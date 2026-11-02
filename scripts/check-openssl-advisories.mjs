// OpenSSL advisory audit: `npm run audit:openssl`. See "Dependency audit" in
// DEVELOPMENT.md.
//
// The Windows build links OpenSSL statically from vcpkg, pinned by the
// `builtin-baseline` (a microsoft/vcpkg commit) in
// scripts/windows/vcpkg/vcpkg.json (#25). Dependabot moves that pin but raises
// no security alerts for vcpkg, so this reads the OpenSSL version the pin
// resolves to (vcpkg's versions/baseline.json at that commit) and fails when
// any of OpenSSL's own advisories lists it as affected, whatever the severity.
// The advisories are the CVE records OpenSSL publishes for its website, in
// github.com/openssl/release-metadata (secjson/), read from one tarball of
// that repository. It also warns when the version's release branch is past
// its end of life there (data.json): OpenSSL lists only the versions it
// supports in a new advisory, so one published after that date may not name
// the pinned version.
//
// It needs the network for both, like the other audits; without it, it fails
// saying what it couldn't fetch, never passes unchecked. No dependencies of
// its own.

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const MANIFEST = "scripts/windows/vcpkg/vcpkg.json";
const BASELINE_URL = (commit) =>
  `https://raw.githubusercontent.com/microsoft/vcpkg/${commit}/versions/baseline.json`;
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
 * Whether a CVE record (CVE JSON 5) lists `version` of OpenSSL as affected.
 * Throws on a record it can't read, so a format change fails the audit.
 */
export function affects(record, version) {
  const id = record?.cveMetadata?.cveId ?? "a record";
  const pinned = parseVersion(version);
  for (const product of record?.containers?.cna?.affected ?? []) {
    // The FIPS object module of 2007 is a product of its own, which HoploDex doesn't link.
    if (product.product !== "OpenSSL") continue;
    if (product.defaultStatus !== "unaffected")
      throw new Error(`${id}: defaultStatus "${product.defaultStatus}" isn't handled`);
    for (const entry of product.versions ?? []) {
      if (entry.status !== "affected") continue;
      if (entry.version?.startsWith("fips-")) continue;
      const read = (text) => {
        const v = parseVersion(text);
        if (!v) throw new Error(`${id}: can't read "${text}" as an OpenSSL version`);
        return v;
      };
      const from = read(entry.version);
      if (compare(pinned, from) < 0) continue;
      if (entry.lessThan !== undefined) {
        if (compare(pinned, read(entry.lessThan)) < 0) return true;
      } else if (entry.lessThanOrEqual !== undefined) {
        if (compare(pinned, read(entry.lessThanOrEqual)) <= 0) return true;
      } else if (compare(pinned, from) === 0) return true;
    }
  }
  return false;
}

/**
 * The audit: errors (each advisory that lists `version`) and warnings (an
 * out-of-support branch) for the records and data.json, as of `today`
 * (`YYYY-MM-DD`).
 */
export function checkOpenSsl(version, records, releases, today) {
  if (records.length < MIN_RECORDS)
    throw new Error(`only ${records.length} advisories read; has OpenSSL's repository changed?`);
  const errors = records
    .filter((record) => affects(record, version))
    .map((record) => {
      const cna = record.containers.cna;
      const severity = cna.metrics?.find((m) => m.other?.content?.text)?.other.content.text;
      const advisory = cna.references?.find((r) => r.tags?.includes("vendor-advisory"))?.url;
      return `OpenSSL ${version} is affected by ${record.cveMetadata.cveId}${severity ? ` (${severity})` : ""}${advisory ? `: ${advisory}` : ""}`;
    });
  const branch = version.split(".").slice(0, 2).join(".");
  const eol = releases?.[branch]?.eol;
  const warnings = [];
  if (eol === undefined) warnings.push(`OpenSSL's release data has no ${branch} branch`);
  else if (!(Date.parse(`${eol} UTC`) > Date.parse(`${today}T00:00:00Z`)))
    warnings.push(
      `OpenSSL ${branch} is out of support since ${eol}: a later advisory may not list ${version}; move the vcpkg pin to a supported branch when vcpkg has one`,
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
    const commit = pinnedBaseline(manifest);
    const version = opensslInBaseline(JSON.parse(await fetchOk(BASELINE_URL(commit))));
    const files = untar(await fetchOk(ADVISORIES_URL));
    const records = [...files]
      .filter(([name]) => /^secjson\/CVE-[\d-]+\.json$/.test(name))
      .map(([, bytes]) => JSON.parse(bytes.toString("utf8")));
    const releases = JSON.parse(files.get("data.json")?.toString("utf8") ?? "null");
    const { errors, warnings } = checkOpenSsl(
      version,
      records,
      releases,
      new Date().toISOString().slice(0, 10),
    );
    for (const warning of warnings) console.warn(`audit:openssl: warning: ${warning}`);
    if (errors.length > 0) {
      for (const error of errors) console.error(`audit:openssl: ${error}`);
      console.error(`audit:openssl: move the pin in ${MANIFEST} to a vcpkg with a fixed OpenSSL`);
      process.exit(1);
    }
    console.log(
      `audit:openssl: OpenSSL ${version} (vcpkg ${commit.slice(0, 12)}): none of OpenSSL's ${records.length} advisories lists it`,
    );
  } catch (error) {
    console.error(`audit:openssl: ${error.message}`);
    process.exit(1);
  }
}
