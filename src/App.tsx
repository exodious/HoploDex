import { ToastProvider } from "./components";
import { AppShell } from "./features/app/AppShell";
import { CollectionProvider } from "./features/app/CollectionProvider";
import { SessionProvider } from "./features/session/SessionProvider";

/** No collection state exists outside an open database: the session shows
 * the chooser until one is open, and mounts the collection's provider and
 * shell afresh for each open (specs/003 contracts/ui-databases.md §3). */
function App() {
  return (
    <ToastProvider>
      <SessionProvider>
        <CollectionProvider>
          <AppShell />
        </CollectionProvider>
      </SessionProvider>
    </ToastProvider>
  );
}

export default App;
