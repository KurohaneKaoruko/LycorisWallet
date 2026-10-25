/** App entry: mounts React into #root and bootstraps the session store. */

import { createRoot } from "react-dom/client";

import { App } from "./shell/App";
import "./boot";

const container = document.getElementById("root");
if (!container) throw new Error("#root missing");
createRoot(container).render(<App />);
