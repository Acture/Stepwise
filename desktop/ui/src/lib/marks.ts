// Hand-drawn marks over laid-out text: rings, underlines, boxes, and the bars and rules of
// a Fitch proof. They are drawn on one overlay per container, never inserted among the text,
// so the text stays plain DOM rendered from the view. How rough a stroke is belongs to the
// skin (`--mark-roughness`); what a mark means belongs to its colour role.

import rough from "roughjs";
import type { Options, PathInfo } from "roughjs/bin/core";

export type Shape = "underline" | "capsule" | "box" | "bar" | "rule";

export interface Mark {
	/** Stable across redraws: it seeds the strokes and says whether a mark is new. */
	id: string;
	shape: Shape;
	color: string;
	targets: HTMLElement[];
	/** Milliseconds to wait before a new mark starts drawing. */
	delay?: number;
	width?: number;
	/** Pixels to move the mark down, for a second stroke beside a first. */
	offset?: number;
}

export interface Stroke {
	key: string;
	d: string;
	color: string;
	width: number;
	fresh: boolean;
	delay: number;
}

interface Box {
	x: number;
	y: number;
	w: number;
	h: number;
}

const generator = rough.generator();

function seed(id: string): number {
	let hash: number = 2166136261;
	for (const character of id) {
		hash = Math.imul(hash ^ character.codePointAt(0)!, 16777619);
	}
	return (hash >>> 0) % 2 ** 31 || 1;
}

/** The targets' line boxes relative to `host`, one box per visual line. */
function lines(host: HTMLElement, targets: HTMLElement[]): Box[] {
	const origin: DOMRect = host.getBoundingClientRect();
	const boxes: Box[] = [];
	for (const target of targets) {
		for (const rect of Array.from(target.getClientRects())) {
			if (rect.width === 0 && rect.height === 0) continue;
			const box: Box = {
				x: rect.left - origin.left,
				y: rect.top - origin.top,
				w: rect.width,
				h: rect.height,
			};
			const same: Box | undefined = boxes.find(
				(line) => Math.abs(line.y - box.y) < Math.max(box.h, 1) / 2,
			);
			if (same) {
				const right: number = Math.max(same.x + same.w, box.x + box.w);
				same.x = Math.min(same.x, box.x);
				same.w = right - same.x;
				same.h = Math.max(same.h, box.h);
			} else {
				boxes.push(box);
			}
		}
	}
	return boxes;
}

/** Every target together, for marks that span lines by design. */
function union(boxes: Box[]): Box | null {
	if (boxes.length === 0) return null;
	const left: number = Math.min(...boxes.map((box) => box.x));
	const top: number = Math.min(...boxes.map((box) => box.y));
	const right: number = Math.max(...boxes.map((box) => box.x + box.w));
	const bottom: number = Math.max(...boxes.map((box) => box.y + box.h));
	return { x: left, y: top, w: right - left, h: bottom - top };
}

/** A ring around text: straight sides that hug it, overlapping itself along the top. */
function capsule(box: Box): string {
	const x: number = box.x - 2;
	const y: number = box.y + 2;
	const w: number = box.w + 4;
	const h: number = box.h - 4;
	const r: number = Math.min(h / 2, 14);
	return [
		`M ${x + r} ${y}`,
		`L ${x + w - r} ${y}`,
		`Q ${x + w} ${y} ${x + w} ${y + r}`,
		`L ${x + w} ${y + h - r}`,
		`Q ${x + w} ${y + h} ${x + w - r} ${y + h}`,
		`L ${x + r} ${y + h}`,
		`Q ${x} ${y + h} ${x} ${y + h - r}`,
		`L ${x} ${y + r}`,
		`Q ${x} ${y} ${x + r} ${y}`,
		`L ${x + r + 7} ${y - 0.5}`,
	].join(" ");
}

function paths(shape: Shape, box: Box, options: Options): PathInfo[] {
	switch (shape) {
		case "underline": {
			const y: number = box.y + box.h - 1;
			return generator.toPaths(
				generator.line(box.x - 2, y, box.x + box.w + 2, y + 1, options),
			);
		}
		case "capsule":
			return generator.toPaths(
				generator.path(capsule(box), {
					...options,
					roughness: (options.roughness ?? 1) * 0.7,
				}),
			);
		case "box":
			return generator.toPaths(
				generator.rectangle(box.x, box.y, box.w, box.h, {
					...options,
					bowing: 0.6,
				}),
			);
		case "bar":
			return generator.toPaths(
				generator.line(box.x + 1, box.y + 2, box.x + 1, box.y + box.h - 2, {
					...options,
					roughness: (options.roughness ?? 1) * 0.45,
					bowing: 0.5,
				}),
			);
		case "rule": {
			const y: number = box.y + box.h;
			return generator.toPaths(
				generator.line(box.x + 1, y, box.x + box.w, y, {
					...options,
					roughness: (options.roughness ?? 1) * 0.45,
					bowing: 0.4,
				}),
			);
		}
	}
}

/** Strokes for every mark; `drawn` holds the ids already on the board, so only new marks animate. */
export function strokes(
	host: HTMLElement,
	marks: Mark[],
	drawn: Set<string>,
): Stroke[] {
	const roughness: number = Number.parseFloat(
		getComputedStyle(host).getPropertyValue("--mark-roughness") || "1",
	);
	const out: Stroke[] = [];
	for (const mark of marks) {
		const all: Box[] = lines(host, mark.targets);
		// Bars and rules follow a whole scope; a ring reads as "this one" only around a single
		// line, so a ring whose text wraps is underlined instead.
		const spanning: boolean =
			mark.shape === "bar" || mark.shape === "box" || mark.shape === "rule";
		const boxes: Box[] = spanning
			? [union(all)].filter((box): box is Box => box !== null)
			: all;
		const shape: Shape =
			mark.shape === "capsule" && boxes.length > 1 ? "underline" : mark.shape;
		const fresh: boolean = !drawn.has(mark.id);
		boxes.forEach((unmoved, line) => {
			const box: Box = { ...unmoved, y: unmoved.y + (mark.offset ?? 0) };
			const options: Options = {
				seed: seed(`${mark.id}:${line}`),
				stroke: mark.color,
				strokeWidth: mark.width ?? 2.8,
				roughness,
				bowing: 1.4,
			};
			paths(shape, box, options).forEach((path, index) => {
				out.push({
					key: `${mark.id}:${line}:${index}`,
					d: path.d,
					color: mark.color,
					width: path.strokeWidth,
					fresh,
					delay: (mark.delay ?? 0) + line * 160 + index * 90,
				});
			});
		});
	}
	return out;
}
