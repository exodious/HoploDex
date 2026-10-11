import { beforeEach, describe, expect, it, vi } from "vitest";
import { createSessionScope } from "../session/sessionScope";
import * as importExportService from "./importExportService";

// specs/008-hardening-batch contracts/tauri-commands.md "Every request names
// its session": the import and export commands are scoped.

const backend = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  invoke: backend.invoke,
}));

describe("importExportService", () => {
  const scope = createSessionScope(17);

  beforeEach(() => {
    backend.invoke.mockReset().mockResolvedValue(undefined);
  });

  it("sends each command with the session", async () => {
    await importExportService.getExportScope(scope, { scope: "all" });
    await importExportService.exportCollection(scope, {
      format: "csv",
      destinationFolder: "/tmp",
      scope: "all",
    });
    await importExportService.importCollection(scope, {
      files: [{ filePath: "/a.csv", format: "csv" }],
    });
    await importExportService.resolveImportConflicts(scope, {
      importSessionId: "import-1",
      resolutions: [],
    });

    expect(backend.invoke.mock.calls).toEqual([
      ["get_export_scope", { input: { scope: "all" } }, { session: 17 }],
      [
        "export_collection",
        { input: { format: "csv", destinationFolder: "/tmp", scope: "all" } },
        { session: 17 },
      ],
      [
        "import_collection",
        { input: { files: [{ filePath: "/a.csv", format: "csv" }] } },
        { session: 17 },
      ],
      [
        "resolve_import_conflicts",
        { input: { importSessionId: "import-1", resolutions: [] } },
        { session: 17 },
      ],
    ]);
  });

  it("refuses once the scope has ended, without sending", async () => {
    const ended = createSessionScope(18);
    ended.end();
    await expect(importExportService.importCollection(ended, { files: [] })).rejects.toMatchObject({
      code: "DATABASE_CLOSED",
    });
    expect(backend.invoke).not.toHaveBeenCalled();
  });
});
