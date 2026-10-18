// The license policy the audit and the notices share (010 research.md §5, §7):
// the allowed licenses, the exceptions scoped to a package or a hand-recorded
// entry, and the order in which a choice of licenses is resolved.
//
// `policy.test.ts` checks ALLOWED against src-tauri/deny.toml (010 research.md
// §9) and PREFERENCE against 010 research.md §5.

/** The GPLv3-compatible licenses. Keep in step with [licenses] allow in src-tauri/deny.toml. */
export const ALLOWED = new Set([
  "0BSD",
  "Apache-2.0",
  "Apache-2.0 WITH LLVM-exception",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "BSL-1.0",
  "CC0-1.0",
  "GPL-3.0-only",
  "GPL-3.0-or-later",
  "ISC",
  "LGPL-2.1-or-later",
  "LGPL-3.0-only",
  "LGPL-3.0-or-later",
  "MIT",
  "MIT-0",
  "MPL-2.0",
  "Unicode-3.0",
  "Unlicense",
  "Zlib",
]);

/**
 * The order in which a choice of licenses (`OR`) is resolved: the first allowed
 * license in this list is the one a notice names (010 research.md §5). An
 * allowed license that isn't listed ranks after all of these.
 */
export const PREFERENCE = [
  "MIT",
  "Apache-2.0",
  "BSD-3-Clause",
  "BSD-2-Clause",
  "ISC",
  "Zlib",
  "Unicode-3.0",
  "BSL-1.0",
  "0BSD",
  "MIT-0",
  "CC0-1.0",
  "MPL-2.0",
  "Apache-2.0 WITH LLVM-exception",
];

/**
 * Licenses allowed only for particular subjects, with the reason (constitution,
 * "Licensing"). A subject is an npm package or crate (`packages`, matched by
 * name) or a hand-recorded entry (`entries`, matched by the entry's name in
 * licenses/hand-recorded.json). None of the entries' licenses is allowed for a
 * package or crate in general (010 research.md §7).
 */
export const EXCEPTIONS = [
  {
    // The OFL isn't GPL-compatible for code, but fonts ship as separate
    // .woff2 files, not combined with the program, and the OFL allows bundling
    // them with any software. The OFL notice must ship with them.
    license: "OFL-1.1",
    packages: /^@fontsource(-variable)?\//,
    reason: "fonts ship as separate files, and the OFL allows bundling them with any software",
  },
  {
    license: "blessing",
    entries: ["SQLite"],
    reason: "SQLite's public-domain dedication, as its SPDX identifier names it",
  },
  {
    license: "CC-BY-4.0",
    entries: ["EFF Large Wordlist for Passphrases"],
    reason: "a word list in the strength hint, used with the attribution the license asks for",
  },
  {
    license: "ODC-By-1.0",
    entries: ["OpenSubtitles 2024 word frequencies"],
    reason:
      "data in the strength hint's dictionary, used with the attribution the license asks for",
  },
  {
    license: "LicenseRef-public-domain",
    entries: ["Moby Word Lists names", "1990 US Census surnames", "Ten million passwords"],
    reason: "public-domain data in the strength hint's dictionary, credited as its source asks",
  },
];

/**
 * Whether `license` (a single SPDX identifier) is allowed for `subject`:
 * `{ package: name }` for an npm package or crate, `{ entry: name }` for a
 * hand-recorded entry.
 */
export function isAllowed(license, subject = {}) {
  if (ALLOWED.has(license)) return true;
  return EXCEPTIONS.some(
    (e) =>
      e.license === license &&
      ((e.packages !== undefined &&
        subject.package !== undefined &&
        e.packages.test(subject.package)) ||
        (e.entries !== undefined &&
          subject.entry !== undefined &&
          e.entries.includes(subject.entry))),
  );
}

/**
 * Data files that must not be in the app (010 research.md §2): the build fails
 * naming the file when an emitted chunk contains its `marker`, a plain
 * substring of the file's opening entries as the bundle encodes them (a JS
 * string literal, so the words run together). `file` is the path inside
 * node_modules, `reason` what is wrong with it.
 */
export const WITHDRAWN = [
  {
    file: "@zxcvbn-ts/language-en/dist/wikipedia.json",
    marker: "AtheAofAandAinAwasAisAforAasAonAwithAbyAheAatAfromAhisAanAwere",
    reason: "CC BY-SA 3.0 / GFDL, 010 research.md §2",
  },
];
