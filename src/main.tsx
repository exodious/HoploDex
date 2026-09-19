import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
// Bundled with the app (never fetched), so the UI renders offline and no
// request ever leaves the device (constitution Principle V, SC-008).
import "@fontsource-variable/big-shoulders-display";
import "@fontsource-variable/atkinson-hyperlegible-next";
import "@fontsource-variable/atkinson-hyperlegible-mono";
import App from "./App";
import { applyTheme, loadTheme } from "./features/app/theme";
import "./styles/global.css";

// Before first render, so a chosen theme never flashes the other one.
applyTheme(loadTheme());

createRoot(document.getElementById("root") as HTMLElement).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
