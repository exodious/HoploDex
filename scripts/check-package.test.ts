// @vitest-environment node
// scripts/check-package.mjs (#19): each tool's listing, read as the paths a
// package must hold.
import { describe, expect, it } from "vitest";
import { listing, missing } from "./check-package.mjs";

describe("package listings", () => {
  it("reads dpkg-deb -c, with or without the leading ./", () => {
    const out = [
      "-rwxr-xr-x 0/0 14148360 2026-11-08 06:40 usr/bin/hoplodex",
      "-rw-r--r-- 0/0   675886 2026-11-08 06:40 ./usr/lib/HoploDex/NOTICES.txt",
      "-rw-r--r-- 0/0      169 2026-11-08 06:40 etc/apport/blacklist.d/hoplodex",
    ].join("\n");
    expect(missing("deb", listing("deb", out))).toEqual([]);
    expect(missing("deb", listing("deb", out.split("\n")[0]))).toEqual([
      "usr/lib/HoploDex/NOTICES.txt",
      "etc/apport/blacklist.d/hoplodex",
    ]);
  });

  it("reads rpm -qlp", () => {
    const out =
      "/usr/bin/hoplodex\n/usr/lib/HoploDex/NOTICES.txt\n/usr/share/applications/HoploDex.desktop\n";
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
