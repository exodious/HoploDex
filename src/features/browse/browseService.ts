import { invoke } from "../../services/tauriClient";
import type { DatabaseStatus } from "../databases/types";
import type { ListFirearmsInput, ListFirearmsOutput } from "./types";

export async function listFirearms(input: ListFirearmsInput): Promise<ListFirearmsOutput> {
  // TEMPORARY (specs/008 T005; removed by T019, which passes the caller's
  // SessionScope instead): read the open session's id from the status so the
  // E2E suite exercises the HoploDex-Session header on one scoped command.
  const { sessionId } = await invoke<DatabaseStatus>("get_database_status");
  return invoke<ListFirearmsOutput>("list_firearms", { input }, { session: sessionId });
}
