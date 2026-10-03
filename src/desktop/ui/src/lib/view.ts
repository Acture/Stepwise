// What the Rust side hands the page and what the page hands back. The declarations are
// generated from src/desktop/view.rs when `cargo test` runs; this file only gathers them.
// The page draws a View and forwards Commands: it never decides what a step means, never
// computes a byte offset and never words a teaching sentence.

export type { Blank } from "./protocol/Blank";
export type { Catalog } from "./protocol/Catalog";
export type { Choice } from "./protocol/Choice";
export type { Command } from "./protocol/Command";
export type { CourseView as Course } from "./protocol/CourseView";
export type { Draft } from "./protocol/Draft";
export type { EntryView } from "./protocol/EntryView";
export type { EvaluationView } from "./protocol/EvaluationView";
export type { Feedback } from "./protocol/Feedback";
export type { Listed } from "./protocol/Listed";
export type { ProofLineView as ProofLine } from "./protocol/ProofLineView";
export type { ProofView } from "./protocol/ProofView";
export type { Run } from "./protocol/Run";
export type { Tone } from "./protocol/Tone";
export type { View } from "./protocol/View";
export type { Written } from "./protocol/Written";

import type { Command } from "./protocol/Command";
import type { Look } from "./protocol/Look";

/** What the page asks of the window around it: a command for the Rust side, or something
 * only the window itself can do, such as showing the file picker. */
export interface Host {
	send: (command: Command) => void;
	openSet: () => void;
	/** Wears a look; the shell keeps it for the next launch. */
	setLook: (look: Look) => void;
	/** Picks a VS Code colour theme file, then wears it. */
	importTheme: () => void;
}
