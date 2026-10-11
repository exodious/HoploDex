import type { SessionScope } from "../session/sessionScope";
import type { ListFirearmsInput, ListFirearmsOutput } from "./types";

export function listFirearms(
  scope: SessionScope,
  input: ListFirearmsInput,
): Promise<ListFirearmsOutput> {
  return scope.invoke<ListFirearmsOutput>("list_firearms", { input });
}
