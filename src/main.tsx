import React from "react";
import ReactDOM from "react-dom/client";
import "./styles.css";

async function installTestBridge() {
  if (import.meta.env.MODE !== "e2e") return;
  const { installE2eMock } = await import("./lib/e2eMock");
  await installE2eMock("main");
}

async function render() {
  await installTestBridge();
  const { default: App } = await import("./App");

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void render();
