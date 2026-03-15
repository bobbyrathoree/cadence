import React from "react";
import ReactDOM from "react-dom/client";
import "./styles.css";

function App() {
  return (
    <div style={{ padding: 40, fontFamily: '-apple-system, system-ui, sans-serif' }}>
      <h1>Cadence</h1>
      <p>Prompt library loading...</p>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
