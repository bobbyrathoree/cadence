import React from "react";
import ReactDOM from "react-dom/client";
import { installE2eMock } from "./lib/e2eMock";
import "./styles.css";

async function render() {
  await installE2eMock("main");
  const { default: App } = await import("./App");

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void render();
