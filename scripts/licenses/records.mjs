// Reads and checks the committed records under licenses/: hand-recorded,
// exceptions, clarifications and native code (010 research.md §7, §8).
//
// The checks take what a target ships as a parameter (`ctx`), so the tests need
// neither `cargo` nor the real `node_modules`:
//
//   ctx = { root, triple, allTargets?, openSslDir?,
//           crates: [{ name, version, folder, license, features?, nativeBuild? }],
//           npm:    [{ name, version, folder, license }] }
//
// `root` is the repository root (a `licenses/texts/...` path resolves against
// it), the operating system comes from `triple`, `features` are the carrier's
// enabled features, and `nativeBuild` lists the `cc`/`cmake` build dependencies
// enabled for the triple.

import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { isAllowed } from "./policy.mjs";
import { findTexts } from "./licenseFiles.mjs";
import { choose } from "./spdx.mjs";

const FILES = {
  handRecorded: "hand-recorded.json",
  exceptions: "exceptions.json",
  clarifications: "clarify.json",
  nativeCode: "native-code.json",
};

/** Per file: the first field (the sort key, and the entry's name in a message) and the fields an entry may have. */
const SHAPES = {
  handRecorded: {
    key: "name",
    fields: ["name", "version", "license", "kind", "os", "carrier", "texts", "reason"],
  },
  exceptions: {
    key: "component",
    fields: ["component", "checkedVersion", "declared", "texts", "from", "reason"],
  },
  clarifications: {
    key: "component",
    fields: ["component", "version", "files", "reason"],
  },
  nativeCode: { key: "crate", fields: ["crate", "what", "note"] },
};

/** The fields an entry must have (010 data-model.md); a hand-recorded carrier also needs `crate` or `npm`. */
const REQUIRED = {
  handRecorded: ["name", "version", "license", "kind", "os", "carrier", "texts", "reason"],
  exceptions: ["component", "checkedVersion", "declared", "texts", "from", "reason"],
  clarifications: ["component", "version", "files", "reason"],
  nativeCode: ["crate", "what", "note"],
};

const CARRIER_FIELDS = ["crate", "npm", "version", "feature"];
const FILE_FIELDS = ["path", "sha256", "text"];
const WHAT = /^(?:hand-recorded:.+|own-code|import-libraries|system-library)$/;
const PREBUILT = /\.(?:lib|a|dll|dylib|so|o)$/i;
const SOURCE_LINE = /^Source: \S/;

const OPENSSL_MIN_MAJOR = 3;
const OPENSSL_ALL_TARGETS = "3.x (the version the Windows build links)";

/** The operating system of a target triple: `linux`, `windows` or `macos`. */
function osOf(triple) {
  if (triple.includes("windows")) return "windows";
  if (triple.includes("apple-darwin")) return "macos";
  return "linux";
}

function readJson(file, name, failures) {
  if (!existsSync(file)) return [];
  let value;
  try {
    value = JSON.parse(readFileSync(file, "utf8"));
  } catch (error) {
    failures.push(`${name}: can't be read as JSON (${error.message}).`);
    return [];
  }
  if (!Array.isArray(value)) {
    failures.push(`${name}: must hold a list of entries.`);
    return [];
  }
  return value;
}

/**
 * The four record files as written. An absent file is `[]`. A file not sorted
 * by its first field, or an entry with a field it doesn't have, is a failure
 * naming the file and the entry (the data model: "The JSON files are sorted by
 * their first field"), so a hand edit can't quietly misspell a key.
 */
export function readRecords(licensesDir) {
  const failures = [];
  const records = {};
  for (const [which, name] of Object.entries(FILES)) {
    const entries = readJson(path.join(licensesDir, name), name, failures);
    records[which] = entries;
    const { key, fields } = SHAPES[which];
    let previous;
    for (const entry of entries) {
      const label = String(entry?.[key] ?? "(no " + key + ")");
      if (entry === null || typeof entry !== "object" || Array.isArray(entry)) {
        failures.push(`${name}: an entry isn't an object.`);
        continue;
      }
      for (const field of Object.keys(entry)) {
        if (!fields.includes(field)) {
          failures.push(`${name}: entry "${label}" has an unknown field "${field}".`);
        }
      }
      for (const field of REQUIRED[which]) {
        if (entry[field] === undefined) {
          failures.push(`${name}: entry "${label}" has no "${field}".`);
        }
      }
      if (which === "handRecorded" && entry.carrier && typeof entry.carrier === "object") {
        const named = ["crate", "npm"].filter((f) => entry.carrier[f] !== undefined);
        if (named.length !== 1) {
          failures.push(
            `${name}: entry "${label}" carrier must name exactly one of crate and npm.`,
          );
        }
        if (entry.carrier.version === undefined) {
          failures.push(`${name}: entry "${label}" carrier has no "version".`);
        }
      }
      if (which === "handRecorded" && entry.carrier && typeof entry.carrier === "object") {
        for (const field of Object.keys(entry.carrier)) {
          if (!CARRIER_FIELDS.includes(field)) {
            failures.push(`${name}: entry "${label}" has an unknown carrier field "${field}".`);
          }
        }
      }
      if (which === "clarifications" && Array.isArray(entry.files)) {
        for (const file of entry.files) {
          for (const field of Object.keys(file ?? {})) {
            if (!FILE_FIELDS.includes(field)) {
              failures.push(`${name}: entry "${label}" has an unknown file field "${field}".`);
            }
          }
        }
      }
      if (which === "nativeCode" && !WHAT.test(String(entry.what))) {
        failures.push(
          `${name}: entry "${label}" has what "${entry.what}"; it must be hand-recorded:<entry name>, own-code, import-libraries or system-library.`,
        );
      }
      if (previous !== undefined) {
        if (label === previous) {
          failures.push(`${name}: entry "${label}" appears twice.`);
        } else if (label < previous) {
          failures.push(
            `${name}: entry "${label}" is out of order after "${previous}"; sort the file by ${key}.`,
          );
        }
      }
      previous = label;
    }
  }
  return { ...records, failures };
}

/**
 * A committed text (under licenses/texts/) less its first line, `Source: <url>`,
 * which says where it was taken from and isn't part of the notice. Throws when
 * that line is missing.
 */
export function readCommittedText(file) {
  const text = readFileSync(file, "utf8");
  const end = text.indexOf("\n");
  const first = end === -1 ? text : text.slice(0, end);
  if (!SOURCE_LINE.test(first)) {
    throw new Error(`${file}: the first line must be "Source: <url>".`);
  }
  return end === -1 ? "" : text.slice(end + 1);
}

/** Reads a committed text for `subject`'s message: `{ text }` or `{ failure }`. */
function committed(root, relative, subject) {
  const file = path.join(root, relative);
  if (!existsSync(file)) return { failure: `${subject}: text file ${relative} is missing.` };
  try {
    return { text: readCommittedText(file) };
  } catch (error) {
    return { failure: `${subject}: ${error.message}` };
  }
}

/** The shipped crate or npm package an entry's `crate:<name>`/`npm:<name>` id names. */
function findComponent(id, ctx) {
  const colon = id.indexOf(":");
  const kind = id.slice(0, colon);
  const name = id.slice(colon + 1);
  const list = kind === "crate" ? ctx.crates : ctx.npm;
  const found = list.find((c) => c.name === name);
  return found ? { ...found, kind, id } : undefined;
}

/** The one license (or conjunction) a component's notice names, falling back to what it declares. */
function chosenLicense(component) {
  try {
    return (
      choose(component.license, (id) => isAllowed(id, { package: component.name })) ??
      component.license
    );
  } catch {
    return component.license;
  }
}

function readOpenSslVersion(ctx, subject) {
  if (!ctx.openSslDir) {
    return {
      failure: `${subject}: OPENSSL_DIR isn't set, so the OpenSSL version the Windows build links can't be read.`,
    };
  }
  const header = path.join(ctx.openSslDir, "include", "openssl", "opensslv.h");
  if (!existsSync(header)) {
    return { failure: `${subject}: ${header} (opensslv.h) doesn't exist.` };
  }
  const source = readFileSync(header, "utf8");
  // OpenSSL 3 has OPENSSL_VERSION_STR; 1.1.x only OPENSSL_VERSION_TEXT ("OpenSSL 1.1.1w  11 Sep 2023").
  const version =
    source.match(/^\s*#\s*define\s+OPENSSL_VERSION_STR\s+"([^"]+)"/m)?.[1] ??
    source.match(/^\s*#\s*define\s+OPENSSL_VERSION_TEXT\s+"OpenSSL\s+(\S+)/m)?.[1];
  if (version === undefined) {
    return { failure: `${subject}: opensslv.h (${header}) names no OpenSSL version.` };
  }
  if (Number.parseInt(version, 10) < OPENSSL_MIN_MAJOR) {
    return {
      failure: `${subject}: the Windows build links OpenSSL ${version}; HoploDex needs OpenSSL 3 (its license is compatible with GPLv3, 1.1's isn't).`,
    };
  }
  return { version };
}

/**
 * The hand-recorded entries for the target's OS (010 research.md §7): each with its
 * texts read and its carrier checked, as `components` ready for the notices.
 * A failure names the entry.
 */
export function checkHandRecorded(entries, ctx) {
  const failures = [];
  const components = [];
  const os = osOf(ctx.triple);
  for (const entry of entries) {
    if (!(entry.os ?? []).includes(os)) continue;
    const subject = `hand-recorded "${entry.name}"`;
    const before = failures.length;
    const carrierIsCrate = entry.carrier?.crate !== undefined;
    const carrierName = carrierIsCrate ? entry.carrier.crate : entry.carrier?.npm;
    const carrierWord = carrierIsCrate ? "crate" : "package";
    const shippedCarrier = (carrierIsCrate ? ctx.crates : ctx.npm).find(
      (c) => c.name === carrierName,
    );
    if (!shippedCarrier) {
      failures.push(
        `${subject}: its carrier ${carrierWord} ${carrierName} isn't shipped on ${os}.`,
      );
    } else {
      if (shippedCarrier.version !== entry.carrier.version) {
        failures.push(
          `${subject}: its carrier ${carrierWord} ${carrierName} is ${shippedCarrier.version}, recorded ${entry.carrier.version}. Review the entry (its version, text and carrier) and update it.`,
        );
      }
      const feature = entry.carrier.feature;
      if (feature !== undefined && !(shippedCarrier.features ?? []).includes(feature)) {
        failures.push(`${subject}: carrier ${carrierName} is shipped without feature ${feature}.`);
      }
    }

    let version = entry.version;
    if (version === "from-build") {
      if (ctx.allTargets || os !== "windows") {
        version = OPENSSL_ALL_TARGETS;
      } else {
        const read = readOpenSslVersion(ctx, subject);
        if (read.failure) failures.push(read.failure);
        else version = read.version;
      }
    }

    const texts = [];
    for (const recorded of entry.texts ?? []) {
      if (recorded.startsWith("package:")) {
        if (!shippedCarrier) continue;
        const relative = recorded.slice("package:".length);
        const file = path.join(shippedCarrier.folder, relative);
        if (!existsSync(file)) {
          failures.push(`${subject}: text file ${relative} is missing from ${carrierName}.`);
          continue;
        }
        texts.push({ file: relative, text: readFileSync(file, "utf8") });
      } else {
        const read = committed(ctx.root, recorded, subject);
        if (read.failure) failures.push(read.failure);
        else texts.push({ file: recorded, text: read.text });
      }
    }

    if (failures.length > before) continue;
    components.push({
      name: entry.name,
      version,
      kind: entry.kind,
      license: entry.license,
      texts,
      source: "hand-recorded",
      carrier: {
        kind: carrierIsCrate ? "crate" : "npm",
        name: carrierName,
        version: entry.carrier.version,
      },
    });
  }
  return { failures, components };
}

/**
 * The exceptions (010 research.md §8): a shipped component whose package leaves out
 * its license text, with the text committed. `used` names the entries this
 * target ships; an entry it doesn't ship is only a warning, since the same
 * exception covers several operating systems. `resolved` maps a component to
 * its committed texts.
 */
export function checkExceptions(entries, ctx) {
  const failures = [];
  const warnings = [];
  const used = [];
  const resolved = {};
  for (const entry of entries) {
    const id = entry.component;
    const subject = `exception ${id}`;
    const component = findComponent(id, ctx);
    if (!component) {
      warnings.push(`${subject}: this target (${ctx.triple}) doesn't ship it.`);
      continue;
    }
    used.push(id);
    if (component.license !== entry.declared) {
      failures.push(
        `${subject}: it now declares "${component.license}", recorded "${entry.declared}". Review the exception.`,
      );
    }
    const found = findTexts(component.folder, chosenLicense(component), {
      declared: component.license,
      label: `${component.kind} ${component.name} ${component.version}`,
    });
    if (!found.failure && found.texts.length > 0) {
      const word = component.kind === "crate" ? "crate" : "package";
      failures.push(
        `${subject}: the ${word} now ships ${found.texts[0].file}; remove the exception.`,
      );
    }
    const texts = [];
    for (const recorded of entry.texts ?? []) {
      const read = committed(ctx.root, recorded, subject);
      if (read.failure) failures.push(read.failure);
      else texts.push({ file: recorded, text: read.text });
    }
    resolved[id] = texts;
  }
  return { failures, warnings, used, resolved };
}

/**
 * The clarifications (010 research.md §8): the files to use in place of what the
 * matcher finds, each with its SHA-256. A file that changed, or is missing,
 * fails so it is reviewed. `resolved` maps a component to its texts, in order.
 */
export function checkClarifications(entries, ctx) {
  const failures = [];
  const warnings = [];
  const used = [];
  const resolved = {};
  for (const entry of entries) {
    const id = entry.component;
    const subject = `clarification ${id}`;
    const component = findComponent(id, ctx);
    if (!component) {
      warnings.push(`${subject}: this target (${ctx.triple}) doesn't ship it.`);
      continue;
    }
    used.push(id);
    const texts = [];
    let ok = true;
    for (const item of entry.files ?? []) {
      if (item.text !== undefined) {
        texts.push({ text: item.text });
        continue;
      }
      // A path under licenses/ is a committed text; any other is inside the package.
      const isCommitted = item.path.startsWith("licenses/");
      const file = isCommitted
        ? path.join(ctx.root, item.path)
        : path.join(component.folder, item.path);
      if (!existsSync(file)) {
        failures.push(`${subject}: ${item.path} is missing.`);
        ok = false;
        continue;
      }
      const actual = createHash("sha256").update(readFileSync(file)).digest("hex");
      if (actual !== item.sha256) {
        failures.push(`${subject}: ${item.path} changed (sha256 ${actual}). Review it.`);
        ok = false;
        continue;
      }
      if (isCommitted) {
        const read = committed(ctx.root, item.path, subject);
        if (read.failure) {
          failures.push(read.failure);
          ok = false;
        } else texts.push({ file: item.path, text: read.text });
      } else {
        texts.push({ file: item.path, text: readFileSync(file, "utf8") });
      }
    }
    if (ok) resolved[id] = texts;
  }
  return { failures, warnings, used, resolved };
}

/** The first `*.lib`, `*.a`, `*.dll`, `*.dylib`, `*.so` or `*.o` file under a folder, as a relative path. */
function findPrebuilt(folder, relative = "") {
  let items;
  try {
    items = readdirSync(path.join(folder, relative), { withFileTypes: true });
  } catch {
    return undefined;
  }
  items.sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
  for (const item of items) {
    const next = relative === "" ? item.name : `${relative}/${item.name}`;
    if (item.isDirectory()) {
      const found = findPrebuilt(folder, next);
      if (found) return found;
    } else if (item.isFile() && PREBUILT.test(item.name)) {
      return next;
    }
  }
  return undefined;
}

/**
 * The native-code review (010 research.md §7): a shipped crate with an enabled `cc`
 * or `cmake` build dependency, or carrying a prebuilt library, must be listed.
 * A listed crate this target doesn't ship is a warning; `used` names the listed
 * crates it does ship.
 */
export function checkNativeCode(entries, ctx) {
  const failures = [];
  const warnings = [];
  const used = [];
  const listed = new Set(entries.map((e) => e.crate));
  for (const crate of ctx.crates) {
    if (listed.has(crate.name)) {
      used.push(crate.name);
      continue;
    }
    const reasons = [];
    if ((crate.nativeBuild ?? []).length > 0) {
      reasons.push(`builds C (${crate.nativeBuild.join(", ")})`);
    }
    const prebuilt = findPrebuilt(crate.folder);
    if (prebuilt) reasons.push(`carries a prebuilt library (${prebuilt})`);
    if (reasons.length > 0) {
      failures.push(
        `native code: crate ${crate.name} ${crate.version} (${ctx.triple}) ${reasons.join(" and ")} and isn't in licenses/native-code.json. Review what it compiles or links and record it.`,
      );
    }
  }
  const shippedNames = new Set(ctx.crates.map((c) => c.name));
  for (const entry of entries) {
    if (!shippedNames.has(entry.crate)) {
      warnings.push(
        `native code: crate ${entry.crate}: this target (${ctx.triple}) doesn't ship it.`,
      );
    }
  }
  return { failures, warnings, used };
}

/**
 * The `--all-targets` form: an exception, clarification or native-code entry
 * that no target uses fails (010 research.md §9). `perTarget` holds each target's
 * `used` lists.
 */
export function unusedAcrossTargets(records, perTarget) {
  const failures = [];
  const usedBy = (which, name) => perTarget.some((target) => (target[which] ?? []).includes(name));
  for (const entry of records.exceptions ?? []) {
    if (!usedBy("exceptions", entry.component)) {
      failures.push(
        `exception ${entry.component}: no target ships it; remove the exception (licenses/exceptions.json).`,
      );
    }
  }
  for (const entry of records.clarifications ?? []) {
    if (!usedBy("clarifications", entry.component)) {
      failures.push(
        `clarification ${entry.component}: no target ships it; remove the clarification (licenses/clarify.json).`,
      );
    }
  }
  for (const entry of records.nativeCode ?? []) {
    if (!usedBy("nativeCode", entry.crate)) {
      failures.push(
        `native code: crate ${entry.crate}: no target ships it; remove the entry (licenses/native-code.json).`,
      );
    }
  }
  return failures;
}
