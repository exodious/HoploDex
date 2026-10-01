import { createContext, useContext } from "react";

export type Route =
  | { page: "collection" }
  | { page: "accessories" }
  | { page: "insurance" }
  | { page: "policy"; id: number }
  | { page: "firearm"; id: number; from: "collection" | "insurance" }
  // specs/006-accessory-links: `from` is where "back" returns to, which for
  // a record reached through a mount is the record it was reached from.
  | {
      page: "accessory";
      id: number;
      from: "accessories" | "collection" | "insurance" | "firearm" | "accessory";
    };

/** Dialogs the app shell owns, so any page can open them. */
export type ShellDialog = "addFirearm" | "addAccessory" | "import" | "export";

/** Where "back" goes from the current page, and what to call it. */
export interface BackTarget {
  label: string;
  go: () => void;
}

export interface Navigation {
  route: Route;
  /** Jumps to a top-level page (the tabs, the brand): forgets any back trail. */
  navigate: (route: Route) => void;
  /** Follows a link: the page being left stays reachable through `back`. */
  open: (route: Route) => void;
  /** Null when there is nothing to go back to. */
  back: BackTarget | null;
  openDialog: (dialog: ShellDialog) => void;
}

export const NavigationContext = createContext<Navigation>({
  route: { page: "collection" },
  navigate: () => {},
  open: () => {},
  back: null,
  openDialog: () => {},
});

export function useNavigation(): Navigation {
  return useContext(NavigationContext);
}
