// Line breaking for formulas. A long formula may wrap between tokens but not inside a short
// pair of brackets: a group the student might pick stays on one line. This is typesetting
// only — brackets group in both languages — and decides nothing about what a step means.

/** A piece of text, or a bracket group of pieces; `keep` groups never break inside. */
export type Piece =
	| { kind: "piece"; index: number }
	| { kind: "group"; keep: boolean; children: Piece[] };

/** A group longer than this may still break inside, between its own groups. */
const KEEP_UNDER: number = 28;

function count(text: string, character: string): number {
	return text.split(character).length - 1;
}

/** Arrange pieces into bracket groups. A piece may hold a whole group, but never half of one
 * together with text outside it — runs split at every bracket a group node owns. */
export function group(texts: string[]): Piece[] {
	const root: Piece[] = [];
	const frames: { children: Piece[]; length: number }[] = [];
	const current = (): Piece[] => frames.at(-1)?.children ?? root;
	texts.forEach((text, index) => {
		const opens: number = count(text, "(");
		const closes: number = count(text, ")");
		for (let open: number = 0; open < opens - closes; open++)
			frames.push({ children: [], length: 0 });
		current().push({ kind: "piece", index });
		for (const frame of frames) frame.length += text.length;
		for (let close: number = 0; close < closes - opens; close++) {
			const frame = frames.pop();
			if (!frame) break;
			current().push({
				kind: "group",
				keep: frame.length <= KEEP_UNDER,
				children: frame.children,
			});
		}
	});
	// An unbalanced tail (never produced by a parser) is laid out as it came.
	while (frames.length > 0) {
		const frame = frames.pop()!;
		current().push(...frame.children);
	}
	return root;
}

/** Split plain formula text so every bracket is its own piece. */
export function pieces(text: string): string[] {
	return text.match(/[()]|[^()]+/g) ?? [];
}
