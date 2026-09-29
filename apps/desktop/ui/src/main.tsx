import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "@/app/App";
import "@/app/styles.css";
import { initI18n } from "@/shared/i18n";

const container = document.getElementById("root");
if (container === null) {
  throw new Error("index.html is missing #root");
}

await initI18n();
createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
