import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
// Bundled with the app (never fetched), so the UI renders offline and no
// request ever leaves the device (constitution Principle V, SC-008).
import "@fontsource-variable/big-shoulders-display";
import "@fontsource-variable/atkinson-hyperlegible-next";
import "@fontsource-variable/atkinson-hyperlegible-mono";
import App from "./App";
import "./styles/global.css";

createRoot(document.getElementById("root") as HTMLElement).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
