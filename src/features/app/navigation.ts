import { createContext, useContext } from "react";

export type Route =
  | { page: "collection" }
  | { page: "insurance" }
  | { page: "firearm"; id: number; from: "collection" | "insurance" };

/** Dialogs the app shell owns, so any page can open them. */
export type ShellDialog = "addFirearm" | "import" | "export";

export interface Navigation {
  route: Route;
  navigate: (route: Route) => void;
  openDialog: (dialog: ShellDialog) => void;
}

export const NavigationContext = createContext<Navigation>({
  route: { page: "collection" },
  navigate: () => {},
  openDialog: () => {},
});

export function useNavigation(): Navigation {
  return useContext(NavigationContext);
}
