// What stopped the page, shown in the window and sent to the shell, so a failure is never
// left as a blank board.

import { invoke } from "@tauri-apps/api/core";

export function fault(line: string): void {
	const shown: HTMLElement = document.createElement("div");
	shown.id = "fault";
	shown.textContent = line;
	document.body.appendChild(shown);
	// The shell may be the thing that failed; the window has said it already.
	void invoke("report", { line }).catch(() => undefined);
}
