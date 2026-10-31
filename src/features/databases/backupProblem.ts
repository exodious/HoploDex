import { CommandFailure } from "../../services/tauriClient";

/** Why the backup before an upgrade couldn't be saved where it was meant to
 * be (009 contracts/ui.md §3). */
export interface BackupProblem {
  /** The folder it couldn't be saved in. */
  folder: string;
  reason: "missing" | "notWritable" | "insufficientSpace" | "outOfReach" | "failed";
  /** With `insufficientSpace` from `INSUFFICIENT_SPACE`, which knows them. */
  bytesNeeded?: number;
  bytesAvailable?: number;
}

/** What an `upgrade_database` refusal says about the backup's folder, or
 * `null` for any other failure. `folder` is the one that was tried when the
 * refusal doesn't name it. */
export function backupProblemOf(error: unknown, folder: string): BackupProblem | null {
  if (!(error instanceof CommandFailure)) return null;
  const details = error.details ?? {};
  const named = (value: unknown) => (typeof value === "string" && value ? value : folder);
  switch (error.code) {
    case "BACKUP_LOCATION_UNAVAILABLE": {
      const reason = details.reason;
      return {
        folder: named(details.path),
        reason:
          reason === "notWritable" ||
          reason === "insufficientSpace" ||
          reason === "missing" ||
          reason === "outOfReach"
            ? reason
            : "missing",
      };
    }
    case "INSUFFICIENT_SPACE":
      return {
        folder: named(details.path),
        reason: "insufficientSpace",
        bytesNeeded: typeof details.bytesNeeded === "number" ? details.bytesNeeded : undefined,
        bytesAvailable:
          typeof details.bytesAvailable === "number" ? details.bytesAvailable : undefined,
      };
    case "UPGRADE_BACKUP_FAILED":
      return { folder: named(details.folder), reason: "failed" };
    default:
      return null;
  }
}
