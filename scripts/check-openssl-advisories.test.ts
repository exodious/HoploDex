// @vitest-environment node
// The OpenSSL advisory audit (#25): fixture CVE records in OpenSSL's format,
// no network.
import { gzipSync } from "node:zlib";
import { describe, expect, it } from "vitest";
import {
  affects,
  checkOpenSsl,
  opensslInBaseline,
  pinnedBaseline,
  untar,
} from "./check-openssl-advisories.mjs";

const COMMIT = "9eec002df1cd109f8f104cd966a763daf9751489";

type Entry = Record<string, string>;
const record = (
  id: string,
  versions: Entry[],
  product = "OpenSSL",
  datePublic = "2026-09-29T16:21:57+02:00",
) => ({
  cveMetadata: { cveId: id },
  containers: {
    cna: {
      datePublic,
      affected: [{ product, defaultStatus: "unaffected", versions }],
      metrics: [{ other: { content: { text: "Low" } } }],
      references: [
        { tags: ["vendor-advisory"], url: "https://openssl-library.org/news/secadv/x.txt" },
      ],
    },
  },
});
const range = (version: string, lessThan: string): Entry => ({
  version,
  lessThan,
  status: "affected",
  versionType: "semver",
});

describe("pinnedBaseline", () => {
  it("returns the pinned commit", () => {
    expect(pinnedBaseline({ dependencies: ["openssl"], "builtin-baseline": COMMIT })).toBe(COMMIT);
  });

  it("refuses a manifest whose OpenSSL version comes from anywhere but the baseline", () => {
    expect(() => pinnedBaseline({ dependencies: [], "builtin-baseline": COMMIT })).toThrow(
      "no openssl",
    );
    expect(() =>
      pinnedBaseline({
        dependencies: [{ name: "openssl", "version>=": "3.6.5" }],
        "builtin-baseline": COMMIT,
      }),
    ).toThrow("builtin-baseline alone");
    expect(() =>
      pinnedBaseline({
        dependencies: ["openssl"],
        overrides: [{ name: "openssl", version: "3.5.0" }],
        "builtin-baseline": COMMIT,
      }),
    ).toThrow("builtin-baseline alone");
    expect(() =>
      pinnedBaseline({ dependencies: ["openssl"], "builtin-baseline": "2026.10.16" }),
    ).toThrow("full commit hash");
  });
});

describe("opensslInBaseline", () => {
  it("reads OpenSSL's version, 3 or later only", () => {
    expect(
      opensslInBaseline({ default: { openssl: { baseline: "3.6.5", "port-version": 2 } } }),
    ).toBe("3.6.5");
    expect(() => opensslInBaseline({ default: { openssl: { baseline: "1.1.1w" } } })).toThrow();
    expect(() => opensslInBaseline({ default: {} })).toThrow();
  });
});

describe("affects", () => {
  it("matches a lessThan range from its first version up to, not including, the fix", () => {
    const r = record("CVE-1", [range("3.6.0", "3.6.5"), range("3.5.0", "3.5.9")]);
    expect(affects(r, "3.6.0")).toBe(true);
    expect(affects(r, "3.6.4")).toBe(true);
    expect(affects(r, "3.6.5")).toBe(false);
    expect(affects(r, "3.5.8")).toBe(true);
    expect(affects(r, "4.0.0")).toBe(false);
  });

  it("matches lessThanOrEqual and a single version", () => {
    const r = record("CVE-2", [
      { version: "3.0.0", lessThanOrEqual: "3.0.2", status: "affected" },
      { version: "3.1.4", status: "affected" },
    ]);
    expect(affects(r, "3.0.2")).toBe(true);
    expect(affects(r, "3.0.3")).toBe(false);
    expect(affects(r, "3.1.4")).toBe(true);
    expect(affects(r, "3.1.5")).toBe(false);
  });

  it("orders 1.x letter releases, a < z < za", () => {
    const r = record("CVE-3", [{ version: "1.0.2", lessThan: "1.0.2za", status: "affected" }]);
    expect(affects(r, "1.0.2")).toBe(true);
    expect(affects(r, "1.0.2z")).toBe(true);
    expect(affects(r, "1.0.2za")).toBe(false);
  });

  it("leaves out unaffected entries, the FIPS module and other products", () => {
    expect(affects(record("CVE-4", [{ version: "3.6.5", status: "unaffected" }]), "3.6.5")).toBe(
      false,
    );
    expect(affects(record("CVE-5", [range("fips-1.1.1", "fips-1.1.2")]), "3.6.5")).toBe(false);
    expect(affects(record("CVE-6", [range("3.6.0", "3.6.9")], "Other"), "3.6.5")).toBe(false);
  });

  it("fails on a record it can't read rather than passing it", () => {
    expect(() => affects(record("CVE-7", [range("3.6", "3.6.9")]), "3.6.5")).toThrow(
      'CVE-7: can\'t read "3.6"',
    );
    const affectedByDefault = record("CVE-8", []);
    affectedByDefault.containers.cna.affected[0].defaultStatus = "affected";
    expect(() => affects(affectedByDefault, "3.6.5")).toThrow("defaultStatus");
  });
});

describe("checkOpenSsl", () => {
  const filler = Array.from({ length: 100 }, (_, i) =>
    record(`CVE-0-${i}`, [range("1.0.0", "1.0.1")]),
  );
  // As data.json had them on 2026-11-02: 3.4 is past its end of life, 4.1 is
  // newer than 4.0, and 5.0 is not out yet.
  const releases = {
    "5.0": { release: "Oct 2027", eol: "Nov 2028" },
    "4.1": { release: "23 Oct 2026", eol: "23 Nov 2027" },
    "4.0": { release: "14 Apr 2026", eol: "14 May 2027" },
    "3.6": { release: "01 Oct 2025", eol: "01 Nov 2026" },
    "3.5": { release: "08 Apr 2025", eol: "08 Apr 2030" },
    "3.4": { release: "22 Oct 2024", eol: "22 Oct 2026" },
  };
  const none = { exceptions: [] };
  const audit = (over: Partial<Parameters<typeof checkOpenSsl>[0]> = {}) =>
    checkOpenSsl({
      version: "3.6.5",
      latest: "3.6.5",
      records: filler,
      releases,
      exceptions: none,
      today: "2026-11-02",
      ...over,
    });
  // Published after 3.6's end of life, listing 4.0 and 3.5 but not 3.6.
  const later = (id: string, versions = [range("4.0.0", "4.0.2"), range("3.5.0", "3.5.7")]) =>
    record(id, versions, "OpenSSL", "2026-11-10T14:00:00Z");

  it("fails on each advisory that lists the version, naming it, its severity and its advisory", () => {
    const { errors } = audit({
      version: "3.5.4",
      latest: "3.5.4",
      records: [...filler, record("CVE-2026-1", [range("3.5.0", "3.5.9")])],
    });
    expect(errors).toEqual([
      "OpenSSL 3.5.4 is affected by CVE-2026-1 (Low): https://openssl-library.org/news/secadv/x.txt; move the pin in scripts/windows/vcpkg/vcpkg.json to a vcpkg with a fixed OpenSSL",
    ]);
  });

  it("warns, without failing, once the version's branch is out of support", () => {
    expect(audit({ today: "2026-10-31" })).toEqual({ errors: [], warnings: [] });
    const { errors, warnings } = audit({ today: "2026-11-01" });
    expect(errors).toEqual([]);
    expect(warnings).toEqual([
      expect.stringContaining(
        "OpenSSL 3.6 is out of support since 01 Nov 2026: a later advisory may not list 3.6.5, so the audit fails on each one that affects 3.5 and 4.0",
      ),
    ]);
    expect(audit({ version: "3.7.0", latest: "3.7.0" }).warnings).toEqual([
      "OpenSSL's release data has no 3.7 branch",
    ]);
  });

  it("past the branch's end of life, fails on a later advisory for the supported branch below or above", () => {
    const below = later("CVE-2026-2", [range("3.5.0", "3.5.7")]);
    const above = later("CVE-2026-3", [range("4.0.0", "4.0.2")]);
    const { errors } = audit({ records: [...filler, below, above] });
    expect(errors).toEqual([
      expect.stringMatching(
        /^CVE-2026-2 \(Low\).* affects OpenSSL 3\.5, published after 3\.6's end of life: check whether 3\.6\.5 is affected/,
      ),
      expect.stringMatching(/^CVE-2026-3 \(Low\).* affects OpenSSL 4\.0, published after/),
    ]);
  });

  it("leaves out advisories before the end of life, and those for branches further away", () => {
    const before = record("CVE-2026-4", [range("3.5.0", "3.5.7")]);
    const farther = later("CVE-2026-5", [range("4.1.0", "4.1.1"), range("3.4.0", "3.4.6")]);
    expect(audit({ records: [...filler, before, farther] }).errors).toEqual([]);
    // Before its end of life, 3.6 is listed when affected, so a later advisory needs no check.
    expect(
      audit({ records: [...filler, later("CVE-2026-6")], today: "2026-10-31" }).errors,
    ).toEqual([]);
  });

  it("clears such an advisory with a dated exception, until its review date", () => {
    const exceptions = {
      exceptions: [{ id: "CVE-2026-7", reason: "3.6 has no such code", review: "2026-12-31" }],
    };
    const records = [...filler, later("CVE-2026-7")];
    expect(audit({ records, exceptions }).errors).toEqual([]);
    expect(audit({ records, exceptions, today: "2026-12-31" }).errors).toEqual([]);
    expect(audit({ records, exceptions, today: "2027-01-01" }).errors).toEqual([
      expect.stringContaining("(its review date 2026-12-31 has passed)"),
    ]);
  });

  it("fails on an exception it can't read, or one that matches nothing", () => {
    const { errors } = audit({
      records: [...filler, record("CVE-2026-8", [range("3.6.0", "3.6.9")]), later("CVE-2026-9")],
      exceptions: {
        extra: 1,
        exceptions: [
          { id: "CVE-2026-9", reason: "", review: "2026-02-30", note: "x" },
          { id: "CVE-2026-8", reason: "lists 3.6.5 itself", review: "2027-01-01" },
          { id: "CVE-2026-99", reason: "no such advisory", review: "2027-01-01" },
        ],
      },
    });
    expect(errors).toEqual([
      expect.stringContaining("OpenSSL 3.6.5 is affected by CVE-2026-8"),
      "audit-openssl.json: extra: unknown key",
      "audit-openssl.json exceptions[0]: note: unknown key",
      "audit-openssl.json exceptions[0]: reason: must be a non-empty string",
      "audit-openssl.json exceptions[0]: review: must be a real date, YYYY-MM-DD",
      expect.stringMatching(/^CVE-2026-9 .* check whether 3\.6\.5 is affected/),
      "audit-openssl.json exceptions[1]: CVE-2026-8 matches no advisory the audit would fail on: remove it",
      "audit-openssl.json exceptions[2]: CVE-2026-99 matches no advisory the audit would fail on: remove it",
    ]);
    expect(audit({ exceptions: [] }).errors).toEqual([
      "audit-openssl.json: must hold a JSON object",
    ]);
  });

  it("fails when vcpkg's master resolves OpenSSL to a newer version than the pin", () => {
    expect(audit({ latest: "3.6.5" }).errors).toEqual([]);
    expect(audit({ latest: "3.6.6" }).errors).toEqual([
      "vcpkg's master resolves OpenSSL to 3.6.6, newer than the pinned 3.6.5: take Dependabot's baseline update, or move builtin-baseline in scripts/windows/vcpkg/vcpkg.json",
    ]);
    expect(audit({ latest: "4.0.0" }).errors).toHaveLength(1);
  });

  it("fails when too few advisories were read to trust the source", () => {
    expect(() => audit({ records: filler.slice(1) })).toThrow("only 99 advisories read");
  });

  it("fails on a date it can't read rather than skipping the check", () => {
    const bad = { ...releases, "3.5": { release: "08 Apr 2025", eol: "someday" } };
    expect(() => audit({ releases: bad })).toThrow('data.json 3.5.eol: can\'t read "someday"');
  });
});

describe("untar", () => {
  /** One ustar entry: a 512-byte header and the content padded to 512. */
  function entry(name: string, content: string, type = "0") {
    const header = Buffer.alloc(512);
    header.write(name, 0);
    header.write(content.length.toString(8).padStart(11, "0"), 124);
    header.write(type, 156);
    const body = Buffer.alloc(Math.ceil(content.length / 512) * 512);
    body.write(content);
    return Buffer.concat([header, body]);
  }

  it("reads the files below the archive's top folder, skipping pax headers and folders", () => {
    const tar = Buffer.concat([
      entry("pax_global_header", "52 comment=abc\n", "g"),
      entry("release-metadata-main/", "", "5"),
      entry("release-metadata-main/data.json", '{"3.6":{}}'),
      entry("release-metadata-main/secjson/CVE-2026-1.json", "x".repeat(600)),
      Buffer.alloc(1024),
    ]);
    const files = untar(gzipSync(tar));
    expect([...files.keys()]).toEqual(["data.json", "secjson/CVE-2026-1.json"]);
    expect(files.get("data.json")!.toString()).toBe('{"3.6":{}}');
    expect(files.get("secjson/CVE-2026-1.json")!.length).toBe(600);
  });
});
