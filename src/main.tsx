import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./frontend/App";
import "./frontend/styles.css";

const root = document.getElementById("root");

if (!root) {
  throw new Error("无法启动：缺少应用根节点。");
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
