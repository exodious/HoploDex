// @vitest-environment node
// scripts/check-package.mjs (#19): each tool's listing, read as the paths a
// package must hold.
import { describe, expect, it } from "vitest";
import { listing, missing, releaseName } from "./check-package.mjs";

describe("package listings", () => {
  it("reads dpkg-deb -c, with or without the leading ./", () => {
    const out = [
      "-rwxr-xr-x 0/0 14148360 2026-11-08 06:40 usr/bin/hoplodex",
      "-rw-r--r-- 0/0   675886 2026-11-08 06:40 ./usr/lib/hoplodex/NOTICES.txt",
      "-rw-r--r-- 0/0      259 2026-11-08 06:40 ./usr/share/applications/hoplodex.desktop",
      "-rw-r--r-- 0/0      169 2026-11-08 06:40 etc/apport/blacklist.d/hoplodex",
    ].join("\n");
    expect(missing("deb", listing("deb", out))).toEqual([]);
    expect(missing("deb", listing("deb", out.split("\n")[0]))).toEqual([
      "usr/lib/hoplodex/NOTICES.txt",
      "usr/share/applications/hoplodex.desktop",
      "etc/apport/blacklist.d/hoplodex",
    ]);
  });

  it("reads rpm -qlp", () => {
    const out =
      "/usr/bin/hoplodex\n/usr/lib/hoplodex/NOTICES.txt\n/usr/share/applications/hoplodex.desktop\n";
    expect(missing("rpm", listing("rpm", out))).toEqual([]);
  });

  it("reads 7z l -slt of an NSIS installer, CRLF and $INSTDIR included", () => {
    const out = [
      "Path = HoploDex_0.1.0_x64-setup.exe",
      "Type = Nsis",
      "",
      "Path = $INSTDIR\\hoplodex.exe",
      "Size = 1",
      "",
      "Path = NOTICES.txt",
    ].join("\r\n");
    expect(missing("nsis", listing("nsis", out))).toEqual([]);
    expect(missing("nsis", listing("nsis", "Path = hoplodex.exe"))).toEqual(["NOTICES.txt"]);
  });
});

describe("release names (#209)", () => {
  it("gives every Tauri package one pattern", () => {
    expect(releaseName("deb", "bundle/deb/hoplodex_0.2.0_amd64.deb", "0.2.0")).toBe(
      "HoploDex-0.2.0-x86_64.deb",
    );
    expect(releaseName("rpm", "hoplodex-0.2.0-1.x86_64.rpm", "0.2.0")).toBe(
      "HoploDex-0.2.0-x86_64.rpm",
    );
    expect(releaseName("nsis", "HoploDex_0.2.0_x64-setup.exe", "0.2.0")).toBe(
      "HoploDex-0.2.0-x86_64-setup.exe",
    );
    expect(releaseName("dmg", "HoploDex_0.2.0_aarch64.dmg", "0.2.0")).toBe(
      "HoploDex-0.2.0-aarch64.dmg",
    );
  });

  it("refuses an architecture a release doesn't ship that kind for", () => {
    expect(() => releaseName("deb", "hoplodex_0.2.0_arm64.deb", "0.2.0")).toThrow();
    expect(() => releaseName("rpm", "hoplodex-0.2.0-1.aarch64.rpm", "0.2.0")).toThrow();
    expect(() => releaseName("nsis", "HoploDex_0.2.0_arm64-setup.exe", "0.2.0")).toThrow();
    expect(() => releaseName("dmg", "HoploDex_0.2.0_x64.dmg", "0.2.0")).toThrow();
  });

  it("refuses another version, another kind or a name it doesn't know", () => {
    expect(() => releaseName("deb", "hoplodex_0.1.0_amd64.deb", "0.2.0")).toThrow();
    expect(() => releaseName("rpm", "hoplodex_0.2.0_amd64.deb", "0.2.0")).toThrow();
    expect(() => releaseName("dmg", "HoploDex-0.2.0-aarch64.dmg", "0.2.0")).toThrow();
  });
});
