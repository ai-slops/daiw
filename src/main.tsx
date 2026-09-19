import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles.css";

const root = document.querySelector("#root");
if (!(root instanceof HTMLElement)) throw new TypeError("Missing #root application mount.");

createRoot(root).render(<StrictMode><App /></StrictMode>);
