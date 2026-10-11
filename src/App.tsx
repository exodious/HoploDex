import { AppShell } from "./features/app/AppShell";
import { CollectionProvider } from "./features/app/CollectionProvider";
import { ApplicationFault, FaultBoundary } from "./features/session/FaultScreen";
import { SessionProvider } from "./features/session/SessionProvider";

/** No collection state exists outside an open database: the session shows
 * the chooser until one is open, and mounts the collection's provider and
 * shell afresh for each open (specs/003 contracts/ui-databases.md §3). A
 * screen that fails to render is replaced by a way out, never a blank
 * window. Toasts belong to the open database's tree, in the session
 * (specs/008 research.md §4). */
function App() {
  return (
    <FaultBoundary fallback={<ApplicationFault />}>
      <SessionProvider>
        <CollectionProvider>
          <AppShell />
        </CollectionProvider>
      </SessionProvider>
    </FaultBoundary>
  );
}

export default App;
