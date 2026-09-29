// What the Rust side hands the page. The page draws it and forwards input; it never decides
// what a step means, never computes a byte offset and never words a teaching sentence.

export type Tone = "plain" | "good" | "bad";

/** One stretch of the rendered expression owned by one node, or by none (spacing). */
export interface Run {
	text: string;
	node: number | null;
	/** The open blank takes the typed answer; a mirror repeats it at another occurrence. */
	blank?: "input" | "mirror";
}

export interface Feedback {
	text: string;
	tone: Tone;
}

/** Where the question in hand sits in the course. */
export interface Course {
	language: string;
	title: string;
	/** The set title when an ordered set is being practised; random practice has none. */
	set: string | null;
	position: number;
	count: number | null;
	back: boolean;
	forward: boolean;
}

export interface Evaluation {
	kind: "evaluation";
	course: Course;
	bindings: string[];
	/** Earlier displayed states of this question, oldest first. */
	history: string[];
	current: Run[];
	/** Run index range each selectable node covers, inclusive. */
	extents: Record<number, [number, number]>;
	/** The node the keyboard has selected, when no blank is open. */
	selected: number | null;
	/** The next line being written: the current one with the selected node blanked out. */
	draft: { runs: Run[]; input: string } | null;
	/** How the question ended: its value, or the sub-expression that raised and what. */
	ending: string | null;
	feedback: Feedback;
}

export interface ProofLine {
	number: number;
	depth: number;
	formula: string;
	rule: string;
	references: number[];
	premise: boolean;
	assumption: boolean;
}

export interface Proof {
	kind: "proof";
	course: Course;
	goal: string;
	lines: ProofLine[];
	/** How many assumptions are open: the depth the next line is written at. */
	open: number;
	finished: boolean;
	input: string;
	feedback: Feedback;
	rules: string | null;
}

export interface Entry {
	kind: "entry";
	message: string | null;
}

export type View = Entry | Evaluation | Proof;

/** Every input the page forwards. The Rust side maps each to one app-layer operation. */
export type Command =
	| { kind: "choose"; language: "python" | "logic" }
	| { kind: "select"; node: number }
	| { kind: "draft"; text: string }
	| { kind: "submit" }
	| { kind: "cancel" }
	| { kind: "step"; forward: boolean }
	| { kind: "undo" }
	| { kind: "reset" }
	| { kind: "hint" }
	| { kind: "help" }
	| { kind: "next" }
	| { kind: "previous" }
	| { kind: "rules" }
	| { kind: "open-set" }
	| { kind: "questions" };
