// Entry point. Loads fonts (Fontsource + Material Symbols), then the two
// theme stylesheets (tokens first, kid layer second), then mounts <App/>.
import "@fontsource-variable/inter";
import "@fontsource/gochi-hand";
import "@fontsource/patrick-hand";
import "material-symbols/outlined.css";
import "@/styles/theme.css";
import "@/styles/kiddraw.css";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";

const root = document.getElementById("root");
if (!root) throw new Error("#root not found");

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>
);
