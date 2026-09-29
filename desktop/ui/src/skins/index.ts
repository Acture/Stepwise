// The skins a board can wear. Each is one stylesheet that fills the tokens base.css names;
// adding a skin adds a file and an entry here, and touches no component.

import "./blackboard.css";
import "./terminal.css";

export const skins = [
	{ id: "blackboard", name: "黑板" },
	{ id: "terminal", name: "终端" },
] as const;

export type Skin = (typeof skins)[number]["id"];

const KEY: string = "stepwise.skin";

function known(value: string | null): value is Skin {
	return skins.some((skin) => skin.id === value);
}

/** The skin this viewer chose last time; the blackboard when nothing was kept. */
export function remembered(): Skin {
	try {
		const value: string | null = localStorage.getItem(KEY);
		return known(value) ? value : "blackboard";
	} catch {
		// Storage can be unavailable; the choice is a convenience, not state worth failing over.
		return "blackboard";
	}
}

export function remember(skin: Skin): void {
	try {
		localStorage.setItem(KEY, skin);
	} catch {
		// As above: forgetting the skin costs one click next launch.
	}
}
