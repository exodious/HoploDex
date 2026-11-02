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
const record = (id: string, versions: Entry[], product = "OpenSSL") => ({
  cveMetadata: { cveId: id },
  containers: {
    cna: {
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
  const releases = { "3.6": { eol: "01 Nov 2026" }, "3.5": { eol: "08 Apr 2030" } };

  it("fails on each advisory that lists the version, naming it, its severity and its advisory", () => {
    const { errors } = checkOpenSsl(
      "3.5.4",
      [...filler, record("CVE-2026-1", [range("3.5.0", "3.5.9")])],
      releases,
      "2026-11-02",
    );
    expect(errors).toEqual([
      "OpenSSL 3.5.4 is affected by CVE-2026-1 (Low): https://openssl-library.org/news/secadv/x.txt",
    ]);
  });

  it("warns, without failing, once the version's branch is out of support", () => {
    expect(checkOpenSsl("3.6.5", filler, releases, "2026-10-31")).toEqual({
      errors: [],
      warnings: [],
    });
    const { errors, warnings } = checkOpenSsl("3.6.5", filler, releases, "2026-11-01");
    expect(errors).toEqual([]);
    expect(warnings).toEqual([
      expect.stringContaining("OpenSSL 3.6 is out of support since 01 Nov 2026"),
    ]);
    expect(checkOpenSsl("3.7.0", filler, releases, "2026-11-01").warnings).toEqual([
      "OpenSSL's release data has no 3.7 branch",
    ]);
  });

  it("fails when too few advisories were read to trust the source", () => {
    expect(() => checkOpenSsl("3.5.4", filler.slice(1), releases, "2026-11-02")).toThrow(
      "only 99 advisories read",
    );
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
