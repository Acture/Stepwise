// The skins a board can wear, and how a look becomes what the board wears. A built-in skin is
// one stylesheet that fills the tokens base.css names; adding one adds a file and an entry
// here, and touches no component. A colour theme from an editor is worn through the `theme`
// rule, with its colours set on the board. The shell keeps the look and knows none of these
// ids: a look naming a skin this page no longer has is worn as the blackboard.

import { createContext } from "svelte";
import type { Attachment } from "svelte/attachments";
import type { Appearance } from "../lib/protocol/Appearance";
import type { FontPick } from "../lib/protocol/FontPick";
import type { Theme } from "../lib/protocol/Theme";
import "./blackboard.css";
import "./native.css";
import "./notebook.css";
import "./terminal.css";
import "./theme.css";

export interface Skin {
	id: string;
	name: string;
}

export const skins: readonly Skin[] = [
	{ id: "blackboard", name: "黑板" },
	{ id: "notebook", name: "练习本" },
	{ id: "terminal", name: "终端" },
	{ id: "native", name: "原生" },
];

/** The faces a look can pick, by the custom property that holds each. */
export type Role = "prose" | "formula";

/** The role tokens a theme may set. Anything else in its tokens is not the page's to set. */
const ROLES: readonly string[] = [
	"surface",
	"ink",
	"ink-faded",
	"ink-bright",
	"mark-write",
	"mark-wrong",
	"mark-point",
	"mark-good",
	"quiet-mark",
	"tray",
	"scrollbar",
	"focus-ring",
	"ink-glow",
];

/** What a board wears: the skin its stylesheet is keyed by, and properties set on it. */
export interface Dress {
	skin: string;
	properties: Record<string, string>;
}

/** A theme's colours as the properties a board or a preview of it wears. */
export function themed(theme: Theme): Dress {
	const properties: Record<string, string> = {
		"color-scheme": theme.dark ? "dark" : "light",
	};
	for (const role of ROLES) {
		const value: string | undefined = theme.tokens[role];
		if (value !== undefined) properties[`--${role}`] = value;
	}
	return { skin: "theme", properties };
}

/** The family a pick names, or null where the skin's own faces stand: an editor font that
 * was not found is the skin's. */
export function face(pick: FontPick, editorFont: string | null): string | null {
	switch (pick.kind) {
		case "skin":
			return null;
		case "editor":
			return editorFont;
		case "family":
			return pick.family;
	}
}

/** A family name as a CSS string. */
export function quoted(family: string): string {
	return `"${family.replace(/["\\]/g, "\\$&")}"`;
}

/** A picked family in front of the skin's own stack for that role. */
export function stack(family: string, role: Role): string {
	return `${quoted(family)}, var(--skin-${role})`;
}

/** The look in force, as the board wears it. */
export function dress(appearance: Appearance): Dress {
	const { look, themes, editorFont } = appearance;
	const theme: Theme | undefined = themes.find((each) => each.id === look.skin);
	const worn: Dress = skins.some((skin) => skin.id === look.skin)
		? { skin: look.skin, properties: {} }
		: theme
			? themed(theme)
			: { skin: "blackboard", properties: {} };
	const properties: Record<string, string> = { ...worn.properties };
	for (const role of ["prose", "formula"] as const) {
		const family: string | null = face(look[role], editorFont);
		if (family !== null) properties[`--chosen-${role}`] = stack(family, role);
	}
	return { skin: worn.skin, properties };
}

/** What the board wears, for the marks drawn on it: they measure the text they mark, and a
 * new skin or face moves that text and sets how rough a stroke is. */
export const [wornDress, wearDress] = createContext<() => Dress>();

/** Sets `properties` on the element one by one, so no value can reach past its own property
 * the way a style string would let it; they are taken off again when the dress changes. */
export function wear(
	properties: Record<string, string>,
): Attachment<HTMLElement> {
	return (element: HTMLElement): (() => void) => {
		for (const [name, value] of Object.entries(properties))
			element.style.setProperty(name, value);
		return () => {
			for (const name of Object.keys(properties))
				element.style.removeProperty(name);
		};
	};
}
