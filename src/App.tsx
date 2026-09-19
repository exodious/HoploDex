import { ToastProvider } from "./components";
import { AppShell } from "./features/app/AppShell";
import { CollectionProvider } from "./features/app/CollectionProvider";

function App() {
  return (
    <ToastProvider>
      <CollectionProvider>
        <AppShell />
      </CollectionProvider>
    </ToastProvider>
  );
}

export default App;
