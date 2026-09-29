// Screens for the design review, typed as the Rust side sends them. Sentences in `feedback`
// are the ones the teaching rules and this window actually say; the proof is unfinished and
// belongs to no shipped question set. Node ownership follows node_owners: a literal belongs
// to the operation around it, and each variable is a node of its own.

import type {
	Catalog,
	Course,
	EvaluationView,
	ProofView,
	Run,
	View,
} from "./view";

function runs(parts: [string, number | null][]): Run[] {
	return parts.map(([text, node]) => ({ text, node, blank: null }));
}

function catalog(title: string, current: number | null): Catalog {
	const titles: [string, boolean][] = [
		["先乘后加", false],
		["同优先级任选", false],
		["跳过的右边", false],
		["在哪里停止", false],
		["长公式", false],
		["肯定前件", true],
	];
	return {
		title,
		current,
		questions: titles.map(([title, proof], index) => ({
			name: `q${index}`,
			title,
			proof,
		})),
	};
}

function course(
	fields: Partial<Course> & Pick<Course, "language" | "title">,
): Course {
	return {
		set: null,
		position: 1,
		count: null,
		back: true,
		forward: true,
		...fields,
	};
}

const python: EvaluationView = {
	course: course({ language: "Python", title: "代入同名变量", position: 3 }),
	catalog: catalog("内置题库", null),
	bindings: ["x = 2", "y = 3"],
	history: [
		{ kind: "expression", text: "x + y * x" },
		{ kind: "expression", text: "2 + y * 2" },
	],
	current: runs([
		["2 + ", 1],
		["3 * 2", 3],
	]),
	extents: { 1: [0, 1], 3: [1, 1] },
	selected: null,
	draft: {
		runs: [
			{ text: "2 + ", node: 1, blank: null },
			{ text: "3 * 2", node: 3, blank: "input" },
		],
		input: "5",
	},
	ending: null,
	feedback: {
		text: "选对了位置，但结果不正确。两个操作数都已求值，现在应用乘法规则。 请再算一次。",
		tone: "bad",
	},
	message: null,
};

const logic: EvaluationView = {
	course: course({
		language: "命题逻辑",
		title: "长公式",
		set: "示例题集",
		position: 2,
		count: 3,
	}),
	catalog: catalog("示例题集", 4),
	bindings: ["P = False", "Q = True", "R = False", "S = True"],
	history: [
		{ kind: "expression", text: "(¬P ∧ Q) → (R ∨ ⊥) ↔ ¬(S ∧ Q) ∨ (R → P)" },
		{
			kind: "expression",
			text: "(¬False ∧ Q) → (R ∨ ⊥) ↔ ¬(S ∧ Q) ∨ (R → False)",
		},
	],
	current: runs([
		["(", 2],
		["True ∧ ", 3],
		["Q", 4],
		[")", 2],
		[" → ", 1],
		["(", 5],
		["R", 7],
		[" ∨ ⊥", 6],
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
		[" → False", 15],
		[")", 14],
	]),
	extents: {
		0: [0, 20],
		1: [0, 8],
		2: [0, 3],
		3: [1, 2],
		4: [2, 2],
		5: [5, 8],
		6: [6, 7],
		7: [6, 6],
		8: [10, 20],
		9: [10, 15],
		10: [11, 15],
		11: [12, 14],
		12: [12, 12],
		13: [14, 14],
		14: [17, 20],
		15: [18, 19],
		16: [18, 18],
	},
	selected: 10,
	draft: null,
	ending: null,
	feedback: {
		text: "点一处子式，在下一行写出它的值，Enter 检查。",
		tone: "plain",
	},
	message: null,
};

const finished: EvaluationView = {
	course: course({ language: "Python", title: "在哪里停止", position: 4 }),
	catalog: catalog("内置题库", 3),
	bindings: [],
	history: [
		{ kind: "expression", text: "(1 + 2) / (3 - 3)" },
		{ kind: "expression", text: "(3) / (3 - 3)" },
		{ kind: "expression", text: "3 / (3 - 3)" },
		{ kind: "expression", text: "3 / (0)" },
	],
	current: runs([["3 / 0", 0]]),
	extents: {},
	selected: null,
	draft: null,
	ending: "3 / 0 引发 ZeroDivisionError",
	feedback: {
		text: "正确。ZeroDivisionError: 除数为零；这一步引发异常，不会得到一个数值。 求值在这里终止。",
		tone: "good",
	},
	message: null,
};

const proof: ProofView = {
	course: course({
		language: "自然演绎",
		title: "条件证明",
		set: "示例题集",
		position: 3,
		count: 3,
		forward: false,
	}),
	catalog: catalog("示例题集", 5),
	goal: "P → R",
	lines: [
		{
			number: 1,
			depth: 0,
			formula: "(P ∧ Q) → R",
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
	input: "",
	feedback: {
		text: "正确。公式、引用行和假设作用域均符合该规则。",
		tone: "good",
	},
	judged: 4,
	rules: null,
	message: null,
};

export const screens: {
	name: string;
	view: View;
	width: number;
	height: number;
}[] = [
	{
		name: "入口",
		view: { kind: "entry", set: null, message: null },
		width: 920,
		height: 460,
	},
	{
		name: "Python · 填错了",
		view: { kind: "evaluation", ...python },
		width: 920,
		height: 460,
	},
	{
		name: "命题逻辑 · 长公式、键盘选择",
		view: { kind: "evaluation", ...logic },
		width: 920,
		height: 460,
	},
	{
		name: "同一题拖窄以后：公式自动换行",
		view: { kind: "evaluation", ...logic },
		width: 520,
		height: 600,
	},
	{
		name: "求值到异常结束",
		view: { kind: "evaluation", ...finished },
		width: 920,
		height: 460,
	},
	{
		name: "自然演绎",
		view: { kind: "proof", ...proof },
		width: 920,
		height: 460,
	},
];
