import React from "react";
import ReactDOM from "react-dom/client";
import "./styles.css";
import { SearchApp } from "./SearchApp";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SearchApp />
  </React.StrictMode>,
);
