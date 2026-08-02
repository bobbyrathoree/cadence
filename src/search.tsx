import React from "react";
import ReactDOM from "react-dom/client";
import { installE2eMock } from "./lib/e2eMock";
import "./styles.css";

async function render() {
  await installE2eMock("search");
  const { SearchApp } = await import("./SearchApp");

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <SearchApp />
    </React.StrictMode>,
  );
}

void render();
