// The page the desktop shell loads: the window's board, which asks the Rust side for what to
// draw.

import { mount } from "svelte";
import Window from "./lib/Window.svelte";
import { fault } from "./lib/fault";
import "./skins";
import "./styles/base.css";

declare global {
	interface Window {
		__stepwiseWatchdog?: number;
	}
}

window.addEventListener("error", (event: ErrorEvent) =>
	fault(`${event.message}\n${event.filename}:${event.lineno}`),
);
window.addEventListener("unhandledrejection", (event: PromiseRejectionEvent) =>
	fault(String(event.reason)),
);

clearTimeout(window.__stepwiseWatchdog);
mount(Window, { target: document.getElementById("app")! });
