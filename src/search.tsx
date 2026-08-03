import React from "react";
import ReactDOM from "react-dom/client";
import "./styles.css";

async function installTestBridge() {
  if (import.meta.env.MODE !== "e2e") return;
  const { installE2eMock } = await import("./lib/e2eMock");
  await installE2eMock("search");
}

async function render() {
  await installTestBridge();
  const { SearchApp } = await import("./SearchApp");

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <SearchApp />
    </React.StrictMode>,
  );
}

void render();
