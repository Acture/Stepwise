// Screens for the design review, written the way the Rust side will send them. Sentences in
// `feedback` are copied from what the teaching rules actually say; the proof is unfinished and
// belongs to no shipped question set.

import type { Evaluation, Proof, Run, View } from "./view";

/** Split source into runs: each maximal stretch of one owner. Test data only. */
function runs(parts: [string, number | null][]): Run[] {
	return parts.map(([text, node]) => ({ text, node }));
}

const python: Evaluation = {
	kind: "evaluation",
	course: {
		language: "Python",
		title: "代入同名变量",
		set: null,
		position: 3,
		count: null,
		back: true,
		forward: true,
	},
	bindings: ["x = 2", "y = 3"],
	history: ["x + y * x", "2 + y * 2"],
	current: runs([
		["2", 1],
		[" + ", 1],
		["3", 3],
		[" * ", 3],
		["2", 3],
	]),
	extents: { 1: [0, 4], 3: [2, 4] },
	selected: null,
	draft: {
		runs: [
			{ text: "2", node: 1 },
			{ text: " + ", node: 1 },
			{ text: "3 * 2", node: 3, blank: "input" },
		],
		input: "5",
	},
	ending: null,
	feedback: {
		text: "选对了位置，但结果不正确。两个操作数都已求值，现在应用乘法规则。 请再算一次。",
		tone: "bad",
	},
};

const logic: Evaluation = {
	kind: "evaluation",
	course: {
		language: "命题逻辑",
		title: "长公式",
		set: "内置题库",
		position: 14,
		count: 21,
		back: true,
		forward: true,
	},
	bindings: ["P = False", "Q = True", "R = False", "S = True"],
	history: [
		"(¬P ∧ Q) → (R ∨ ⊥) ↔ ¬(S ∧ Q) ∨ (R → P)",
		"(¬False ∧ Q) → (R ∨ ⊥) ↔ ¬(S ∧ Q) ∨ (R → False)",
	],
	// Literals belong to the operation around them; each variable is a node of its own.
	current: runs([
		["(", 2],
		["True", 3],
		[" ∧ ", 3],
		["Q", 4],
		[")", 2],
		[" → ", 1],
		["(", 5],
		["R", 7],
		[" ∨ ", 6],
		["⊥", 6],
		[")", 5],
		[" ↔ ", 0],
		["¬", 9],
		["(", 10],
		["S", 12],
		[" ∧ ", 11],
		["Q", 13],
		[")", 10],
		[" ∨ ", 8],
		["(", 14],
		["R", 16],
		[" → ", 15],
		["False", 15],
		[")", 14],
	]),
	extents: {
		0: [0, 23],
		1: [0, 10],
		2: [0, 4],
		3: [1, 3],
		4: [3, 3],
		5: [6, 10],
		6: [7, 9],
		7: [7, 7],
		8: [12, 23],
		9: [12, 17],
		10: [13, 17],
		11: [14, 16],
		12: [14, 14],
		13: [16, 16],
		14: [19, 23],
		15: [20, 22],
		16: [20, 20],
	},
	selected: 10,
	draft: null,
	ending: null,
	feedback: {
		text: "点一处子式，在下一行写出它的值，Enter 检查。",
		tone: "plain",
	},
};

const finished: Evaluation = {
	kind: "evaluation",
	course: {
		language: "Python",
		title: "在哪里停止",
		set: "内置题库",
		position: 10,
		count: 21,
		back: true,
		forward: true,
	},
	bindings: [],
	history: ["(1 + 2) / (3 - 3)", "(3) / (3 - 3)", "3 / (3 - 3)"],
	current: runs([
		["3", 0],
		[" / ", 0],
		["(", 2],
		["0", null],
		[")", 2],
	]),
	extents: { 0: [0, 4], 2: [2, 4] },
	selected: null,
	draft: null,
	ending: "3 / 0 引发 ZeroDivisionError",
	feedback: {
		text: "正确。除数为零时，Python 的除法引发 ZeroDivisionError。 求值在这里终止。",
		tone: "good",
	},
};

const proof: Proof = {
	kind: "proof",
	course: {
		language: "自然演绎",
		title: "条件证明",
		set: "示例题集",
		position: 5,
		count: 5,
		back: true,
		forward: false,
	},
	goal: "P → R",
	lines: [
		{
			number: 1,
			depth: 0,
			formula: "P ∧ Q → R",
			rule: "premise",
			references: [],
			premise: true,
			assumption: false,
		},
		{
			number: 2,
			depth: 0,
			formula: "Q",
			rule: "premise",
			references: [],
			premise: true,
			assumption: false,
		},
		{
			number: 3,
			depth: 1,
			formula: "P",
			rule: "assume",
			references: [],
			premise: false,
			assumption: true,
		},
		{
			number: 4,
			depth: 1,
			formula: "P ∧ Q",
			rule: "and-intro",
			references: [3, 2],
			premise: false,
			assumption: false,
		},
	],
	open: 1,
	finished: false,
	input: "R ; mp ; 1,4",
	feedback: {
		text: "正确。公式、引用行和假设作用域均符合该规则。",
		tone: "good",
	},
	rules: null,
};

export const screens: {
	name: string;
	view: View;
	width: number;
	height: number;
}[] = [
	{
		name: "入口",
		view: { kind: "entry", message: null },
		width: 920,
		height: 460,
	},
	{ name: "Python · 填错了", view: python, width: 920, height: 460 },
	{ name: "命题逻辑 · 长公式、键盘选择", view: logic, width: 920, height: 460 },
	{
		name: "同一题拖窄以后：公式自动换行",
		view: logic,
		width: 520,
		height: 600,
	},
	{ name: "求值到异常结束", view: finished, width: 920, height: 460 },
	{ name: "自然演绎", view: proof, width: 920, height: 460 },
];
