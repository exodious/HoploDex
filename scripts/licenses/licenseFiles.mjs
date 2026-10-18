// Finds and places a package's or crate's license, NOTICE and attribution files
// (010 research.md §5; 010 FR-010). No fallback to a generic text: a package
// with no text for its license fails, and a file that can't be placed in a
// package with a choice of licenses fails.

import { readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { PREFERENCE, isAllowed } from "./policy.mjs";

/** A license file smaller than this that no name places is a pointer, not a text. */
const POINTER_BYTES = 400;

const LICENSE_FILE = /^(?:licen[cs]e|copying|unlicense)/i;
const NOTICE_FILE = /^notice/i;
const COPYRIGHT_FILE = /^copyright/i;
const AUTHORS_FILE = /^authors/i;
const LICENSE_FOLDER = /^licen[cs]es?$/i;

/** The opening phrases that identify a license's text (010 research.md §5), lower case. */
const PHRASES = {
  MIT: [["permission is hereby granted, free of charge"]],
  "Apache-2.0": [["apache license", "version 2.0"]],
  "BSD-3-Clause": [["redistribution and use in source and binary forms"]],
  "BSD-2-Clause": [["redistribution and use in source and binary forms"]],
  Zlib: [["this software is provided 'as-is'"]],
  "Unicode-3.0": [["unicode license"]],
  ISC: [["permission to use, copy, modify, and/or distribute this software"]],
  "BSL-1.0": [["boost software license"]],
  "MPL-2.0": [["mozilla public license", "version 2.0"]],
  Unlicense: [["this is free and unencumbered software"]],
  "CC0-1.0": [["cc0 1.0 universal"]],
};

const normalize = (text) =>
  text.toLowerCase().replace(/[‘’]/g, "'").replace(/[“”]/g, '"').replace(/\s+/g, " ");

/** The licenses an SPDX expression names, without the operators: "A OR B WITH C" -> [A, "B WITH C"]. */
function idsOf(expression) {
  return expression
    .split(/\s+(?:OR|AND)\s+|[()]/)
    .map((id) => id.trim())
    .filter(Boolean);
}

/** The first word of an identifier, which is what a file name carries: "Apache-2.0" -> "apache". */
const nameWord = (id) => id.split(/[\s-]/)[0].toLowerCase();

/** The licenses in `ids` that a file's name says it holds (LICENSE-MIT, LICENSE.Apache-2.0, MIT.txt). */
function placedByName(file, ids) {
  const tokens = path.posix
    .basename(file)
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter(Boolean);
  return ids.filter((id) => tokens.includes(nameWord(id)));
}

/** The licenses in `ids` whose opening phrases a file's text holds. */
function placedByContent(text, ids) {
  const normalized = normalize(text);
  return ids.filter((id) =>
    (PHRASES[id] ?? []).some((all) => all.every((phrase) => normalized.includes(phrase))),
  );
}

/**
 * The candidate files as forward-slash paths relative to `folder`: the files
 * in its root, and everything under a `LICENSES/` or `license/` folder there.
 */
function candidateFiles(folder) {
  const found = [];
  const walk = (prefix) => {
    for (const entry of readdirSync(path.join(folder, prefix), { withFileTypes: true })) {
      const relative = prefix === "" ? entry.name : `${prefix}/${entry.name}`;
      if (entry.isDirectory()) {
        if (prefix !== "" || LICENSE_FOLDER.test(entry.name)) walk(relative);
      } else if (entry.isFile()) {
        found.push(relative);
      }
    }
  };
  walk("");
  return found;
}

const byPath = (a, b) => (a.file < b.file ? -1 : a.file > b.file ? 1 : 0);

/**
 * The texts to ship for the package or crate in `folder`, in order: license
 * files, then NOTICE, COPYRIGHT and AUTHORS files.
 *
 * - Files that count are `LICENSE*`, `LICENCE*`, `COPYING*` and `UNLICENSE*` in
 *   the root (case ignored; `.spdx` files are metadata, not texts), every file
 *   in a `LICENSES/` or `license/` folder, `NOTICE*` always, and `COPYRIGHT*`
 *   and `AUTHORS*` only beside a license file.
 * - A package with one license (`declared` has no `OR`) takes all its license
 *   files. One with a choice takes the files for `chosen`, placed by name and
 *   then by content; a file that holds two licenses counts for both.
 * - A license file under 400 bytes that no name places only points elsewhere:
 *   it isn't a text.
 *
 * Returns `{ texts: [{ file, text }], failure }`; on a failure `texts` is empty
 * and `failure` names `label`. `file` is the path inside `folder`.
 */
export function findTexts(folder, chosen, { declared, label }) {
  const failed = (message) => ({ texts: [], failure: `${label}: ${message}` });
  const choice = /\sOR\s/.test(declared);
  const declaredIds = idsOf(declared);

  const licenseFiles = [];
  const notices = [];
  const copyrights = [];
  const authors = [];
  for (const file of candidateFiles(folder)) {
    if (/\.spdx$/i.test(file)) continue;
    const top = file.split("/")[0];
    const inFolder = file.includes("/") && LICENSE_FOLDER.test(top);
    const name = path.posix.basename(file);
    const read = () => ({ file, text: readFileSync(path.join(folder, file), "utf8") });
    if (inFolder || (!file.includes("/") && LICENSE_FILE.test(name))) {
      const entry = read();
      const small = statSync(path.join(folder, file)).size < POINTER_BYTES;
      if (small && placedByName(file, declaredIds).length === 0) continue;
      licenseFiles.push(entry);
    } else if (!file.includes("/")) {
      if (NOTICE_FILE.test(name)) notices.push(read());
      else if (COPYRIGHT_FILE.test(name)) copyrights.push(read());
      else if (AUTHORS_FILE.test(name)) authors.push(read());
    }
  }
  licenseFiles.sort(byPath);

  const missing = (id) =>
    failed(
      `no license text for ${id} in its package. Add an exception (licenses/exceptions.json) with the text from its source repository; see DEVELOPMENT.md, "License notices".`,
    );
  if (licenseFiles.length === 0) return missing(chosen);

  let chosenFiles = licenseFiles;
  if (choice) {
    // Placed by name first, then by content.
    const placed = new Map();
    for (const entry of licenseFiles) {
      const byName = placedByName(entry.file, declaredIds);
      placed.set(entry.file, byName.length > 0 ? byName : placedByContent(entry.text, declaredIds));
    }
    const unplaced = licenseFiles.filter((e) => placed.get(e.file).length === 0);
    if (unplaced.length > 0) {
      return failed(
        `can't tell which of ${unplaced.map((e) => e.file).join(", ")} is the ${chosen} text. Add a clarification (licenses/clarify.json).`,
      );
    }
    const wanted = idsOf(chosen);
    for (const id of wanted) {
      if (!licenseFiles.some((e) => placed.get(e.file).includes(id))) return missing(id);
    }
    chosenFiles = licenseFiles.filter((e) => wanted.some((id) => placed.get(e.file).includes(id)));
  }

  return {
    texts: [
      ...chosenFiles,
      ...notices.sort(byPath),
      ...copyrights.sort(byPath),
      ...authors.sort(byPath),
    ],
    failure: null,
  };
}

/**
 * The licenses to look for a text of, best first: `chosen`, then the other
 * allowed alternatives of a plain choice (`A OR B`) in the shared preference
 * order, since a package may ship only a less preferred alternative's text.
 * `name` is the package or crate, for `isAllowed`'s scoped exceptions. The
 * notices and the stale-exception check both look through this list.
 */
export function alternativesOf(declared, chosen, name) {
  if (/[()]|\sAND\s/.test(declared)) return [chosen];
  const rank = (id) => {
    const index = PREFERENCE.indexOf(id);
    return index === -1 ? PREFERENCE.length : index;
  };
  const others = declared
    .split(/\s+OR\s+/)
    .filter((id) => id !== chosen && isAllowed(id, { package: name }))
    .sort((a, b) => rank(a) - rank(b));
  return [chosen, ...others];
}

/**
 * `findTexts` for the first of `licenses` that the folder has a text for (010
 * research.md §5): a choice resolves to its most preferred allowed license, but
 * a package that ships only another alternative's text (`dunce` ships CC0's,
 * not Apache-2.0's) is used under that one. Returns `findTexts`' result and
 * the `license` it is for; with no text for any, the first license's failure.
 */
export function findTextsAmong(folder, licenses, options) {
  let first;
  for (const license of licenses) {
    const found = findTexts(folder, license, options);
    if (found.failure === null) return { ...found, license };
    first ??= { ...found, license };
  }
  return first;
}

/**
 * The first of `licenses` (best first) that some of `texts` (`[{ file, text }]`,
 * committed under licenses/texts/) is a text of, placed as `findTexts` places a
 * package's files: by the file's name, then by its content, and a text under 400
 * bytes that no name places only points elsewhere (an exception for a choice of
 * licenses names the license whose text it holds, 010 research.md §8).
 * `undefined` when none places.
 */
export function placeTexts(texts, licenses) {
  for (const license of licenses) {
    for (const { file, text } of texts) {
      if (placedByName(file, [license]).length > 0) return license;
      if (Buffer.byteLength(text) >= POINTER_BYTES && placedByContent(text, [license]).length > 0) {
        return license;
      }
    }
  }
  return undefined;
}
