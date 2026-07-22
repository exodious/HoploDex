import { invoke } from "../../services/tauriClient";
import type { ListFirearmsInput, ListFirearmsOutput } from "./types";

export function listFirearms(input: ListFirearmsInput): Promise<ListFirearmsOutput> {
  return invoke<ListFirearmsOutput>("list_firearms", { input });
}
