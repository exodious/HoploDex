// What ships in the interface (010 research.md §5, §6): one function that the
// audit and the notices share, over package-lock.json.
import { describe, expect, it } from "vitest";
import { npmPackages, npmShipped } from "./shipped.mjs";

const lock = {
  packages: {
    "": { name: "hoplodex", version: "0.1.0", license: "GPL-3.0-only" },
    "node_modules/react": { version: "18.3.1", license: "MIT" },
    "node_modules/@radix-ui/react-dialog": { version: "1.1.0", license: "MIT" },
    "node_modules/react/node_modules/loose-envify": { version: "1.4.0", license: "MIT" },
    "node_modules/typescript": { version: "5.6.0", license: "Apache-2.0", dev: true },
    "node_modules/@types/react": { version: "18.3.0", license: "MIT", devOptional: true },
    "node_modules/csstype": { version: "3.1.3", license: "MIT", devOptional: true },
    "node_modules/aliased": { name: "real-name", version: "2.0.0", license: "ISC" },
    "node_modules/odd": { version: "1.0.0", license: { type: "MIT" } },
  },
};

describe("010 research.md §6 - npmShipped()", () => {
  const shipped = npmShipped(lock);
  const names = shipped.map((p: { name: string }) => p.name);

  it("leaves out the root package", () => {
    expect(names).not.toContain("hoplodex");
  });

  it("leaves out dev packages and, since 010, devOptional ones", () => {
    expect(names).not.toContain("typescript");
    expect(names).not.toContain("@types/react");
    expect(names).not.toContain("csstype");
  });

  it("names a nested package by its last node_modules/ segment, and a scoped one whole", () => {
    expect(names).toContain("loose-envify");
    expect(names).toContain("@radix-ui/react-dialog");
  });

  it("prefers the lockfile's own name for an alias", () => {
    expect(names).toContain("real-name");
    expect(names).not.toContain("aliased");
  });

  it("returns name, version, location and the declared license", () => {
    const react = shipped.find((p: { name: string }) => p.name === "react");
    expect(react).toMatchObject({
      name: "react",
      version: "18.3.1",
      location: "node_modules/react",
      license: "MIT",
    });
    expect(shipped.map((p: { name: string }) => p.name).sort()).toEqual([
      "@radix-ui/react-dialog",
      "loose-envify",
      "odd",
      "react",
      "real-name",
    ]);
  });

  it("keeps a non-string license as it is, for the audit to report", () => {
    const odd = shipped.find((p: { name: string }) => p.name === "odd");
    expect(odd.license).toEqual({ type: "MIT" });
  });
});

describe("010 research.md §6 - npmPackages()", () => {
  it("marks development packages, devOptional ones included", () => {
    const development = npmPackages(lock)
      .filter((p: { development: boolean }) => p.development)
      .map((p: { name: string }) => p.name)
      .sort();
    expect(development).toEqual(["@types/react", "csstype", "typescript"]);
  });
});
