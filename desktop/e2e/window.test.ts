// The installed window, driven like a student would: every check reads what the page drew, and
// the app under test is the one the installer wrote, with no code of its own for testing.

import assert from "node:assert/strict";
import type { ChildProcess } from "node:child_process";
import { copyFileSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { setTimeout as sleep } from "node:timers/promises";
import type { ChainablePromiseElement } from "webdriverio";
import {
	close,
	configure,
	integrity,
	open,
	start,
	type Setup,
} from "./driver.ts";

const setup: Setup = configure();
// A path with a space and Chinese in it, as a student's home may have.
const progress: string = join(
	mkdtempSync(join(tmpdir(), "stepwise 进度 ")),
	"progress.json",
);

let driver: ChildProcess | undefined;
let browser: WebdriverIO.Browser;
let failed: boolean = false;
let shots: number = 0;

/** Text as the page shows it, with its whitespace runs folded. */
function squeeze(text: string): string {
	return text.replace(/\s+/g, " ").trim();
}

/** Polls `read` until `ok` holds, and says what it last saw when it never does. */
async function until<T>(
	what: string,
	read: () => Promise<T>,
	ok: (value: T) => boolean,
	timeout: number = 20_000,
): Promise<T> {
	const deadline: number = Date.now() + timeout;
	let last: T | undefined;
	let problem: unknown;
	for (;;) {
		try {
			last = await read();
			if (ok(last)) return last;
		} catch (error: unknown) {
			// An element replaced under the read; the next poll finds the new one.
			problem = error;
		}
		if (Date.now() > deadline)
			throw new Error(`${what}; last saw ${JSON.stringify(last)}`, {
				cause: problem,
			});
		await sleep(200);
	}
}

/** The element under `selector` whose text holds `label`. */
async function labelled(
	selector: string,
	label: string,
	exact: boolean = false,
): Promise<WebdriverIO.Element> {
	let found: WebdriverIO.Element | undefined;
	await until(
		`no ${selector} reads ${label}`,
		async () => {
			const texts: string[] = [];
			for (const element of await browser.$$(selector)) {
				const text: string = squeeze(await element.getText());
				texts.push(text);
				if (exact ? text === label : text.includes(label)) {
					found = element;
					break;
				}
			}
			return texts;
		},
		() => found !== undefined,
	);
	return found!;
}

const CURRENT: string = 'main div.line[role="group"]';

/** Waits until the line in hand reads `text`. */
async function expression(text: string): Promise<void> {
	await until(
		`the line in hand does not read ${text}`,
		async () => squeeze(await browser.$(CURRENT).getText()),
		(shown: string) => shown === text,
	);
}

async function history(): Promise<string[]> {
	const lines: string[] = [];
	for (const line of await browser.$$("main ol.history li"))
		lines.push(squeeze(await line.getText()));
	return lines;
}

async function feedback(tone: "good" | "bad"): Promise<string> {
	return until(
		`no ${tone} feedback`,
		async () => {
			const shown: ChainablePromiseElement = browser.$("main p.feedback");
			return {
				tone: (await shown.getAttribute("class")) ?? "",
				text: squeeze(await shown.getText()),
			};
		},
		(shown: { tone: string; text: string }) =>
			shown.tone.split(/\s+/).includes(tone) && shown.text !== "",
	).then((shown: { text: string }) => shown.text);
}

async function blank(): Promise<ChainablePromiseElement> {
	const field: ChainablePromiseElement = browser.$(".blank input");
	await field.waitForExist({ timeout: 20_000 });
	return field;
}

/** Points at the operation or name whose glyph in the line in hand is `glyph`. */
async function point(glyph: string): Promise<void> {
	await (await labelled(`${CURRENT} span.node`, glyph, true)).click();
}

/** Writes `answer` in the open blank and presses Enter on it. */
async function answer(text: string): Promise<void> {
	const field: ChainablePromiseElement = await blank();
	await field.setValue(text);
	assert.equal(await field.getValue(), text);
	await browser.keys("Enter");
}

async function choose(language: "Python 表达式" | "命题逻辑"): Promise<void> {
	await (await labelled(".board button.choice", language)).click();
	await browser.$("header .where .title").waitForExist({ timeout: 20_000 });
}

/** Picks a question by its title from the question list. */
async function pick(title: string): Promise<void> {
	await browser.$('button[title="题目与题集"]').click();
	await (await labelled("section.sheet .questions button", title)).click();
	await until(
		`the header does not name ${title}`,
		async () => squeeze(await browser.$("header .where .title").getText()),
		(shown: string) => shown === title,
	);
}

/** The board itself, found positively: the page's watchdog is cleared the moment its script
 * starts, so a page with no fault shown has not thereby drawn anything. */
async function drawn(): Promise<void> {
	const names: string[] = await until(
		"the language choice is not drawn",
		async () => {
			const shown: string[] = [];
			for (const name of await browser.$$(".board button.choice .name"))
				shown.push(squeeze(await name.getText()));
			return shown;
		},
		(shown: string[]) => shown.length === 2,
		60_000,
	);
	assert.deepEqual(names, ["Python 表达式", "命题逻辑"]);
	assert.equal(await browser.$("#fault").isExisting(), false);
}

async function shoot(name: string): Promise<void> {
	shots += 1;
	await browser.saveScreenshot(
		join(setup.evidence, `${String(shots).padStart(2, "0")}-${name}.png`),
	);
}

/** One step of the walk, in order: a step after a failed one is skipped, and every step ends
 * with a screenshot, a failed one too. */
function step(name: string, title: string, body: () => Promise<void>): void {
	test(title, async (t: { skip: (message: string) => void }) => {
		if (failed) {
			t.skip("an earlier step failed");
			return;
		}
		let problem: unknown;
		try {
			await body();
		} catch (error: unknown) {
			failed = true;
			problem = error;
		}
		try {
			await shoot(problem === undefined ? name : `${name}-failed`);
		} catch (error: unknown) {
			if (problem === undefined) throw error;
			throw new AggregateError(
				[problem, error],
				"the step failed, and so did its screenshot",
			);
		}
		if (problem !== undefined) throw problem;
	});
}

before(async () => {
	if (process.platform === "win32") {
		// WebView2 ignores the debugging arguments msedgedriver passes to a program running
		// elevated, and the runner runs everything as administrator; the job drops this whole
		// process tree to Medium, which is also how a student's window runs.
		const label: string = integrity();
		console.log(`Integrity: ${label}`);
		assert.match(label, /S-1-16-8192/);
	}
	driver = await start(setup);
	browser = await open(setup, progress, "session-1");
});

after(async () => {
	try {
		if (browser) await close(browser);
	} finally {
		driver?.kill();
		try {
			copyFileSync(progress, join(setup.evidence, "progress.json"));
		} catch (error: unknown) {
			if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
		}
	}
});

step("drawn", "the window draws its board", drawn);

step(
	"picked",
	"a question picked from the list opens on the board",
	async () => {
		await choose("Python 表达式");
		await pick("先乘后加");
		await expression("2 + (3 * 4)");
	},
);

step(
	"refused",
	"a click opens a blank, and a Chinese answer is refused and kept",
	async () => {
		await point("*");
		await answer("十二");
		await feedback("bad");
		assert.equal(await (await blank()).getValue(), "十二");
		await expression("2 + (3 * 4)");
	},
);

step(
	"composed",
	"Enter while composing does not submit, and a plain Enter does",
	async () => {
		// The rules hold 12 once it is typed, so an Enter that got through would be accepted
		// and move the line on; an edit also stops calling the last answer a mistake.
		const field: ChainablePromiseElement = await blank();
		await field.setValue("12");
		await until(
			"the edit still reads as a mistake",
			async () => browser.$("main p.feedback.bad").isExisting(),
			(bad: boolean) => !bad,
		);
		// Script-dispatched events reach only the guard code, not a real input method. WebKit
		// confirms a candidate with an Enter after compositionend, isComposing false and
		// keyCode 229; both kinds must pass through untaken.
		const composed: {
			composing: boolean;
			keyCode: number;
			prevented: boolean[];
		} = await browser.execute(() => {
			const blank: HTMLInputElement | null =
				document.querySelector(".blank input");
			if (!blank) throw new Error("no open blank");
			blank.dispatchEvent(
				new CompositionEvent("compositionstart", { bubbles: true, data: "" }),
			);
			blank.dispatchEvent(
				new InputEvent("input", {
					bubbles: true,
					data: "12",
					inputType: "insertCompositionText",
					isComposing: true,
				}),
			);
			const composing: KeyboardEvent = new KeyboardEvent("keydown", {
				bubbles: true,
				cancelable: true,
				key: "Enter",
				code: "Enter",
				isComposing: true,
			});
			blank.dispatchEvent(composing);
			blank.dispatchEvent(
				new CompositionEvent("compositionend", { bubbles: true, data: "12" }),
			);
			const confirming: KeyboardEvent = new KeyboardEvent("keydown", {
				bubbles: true,
				cancelable: true,
				key: "Enter",
				code: "Enter",
			});
			Object.defineProperty(confirming, "keyCode", { value: 229 });
			blank.dispatchEvent(confirming);
			return {
				composing: composing.isComposing,
				keyCode: confirming.keyCode,
				prevented: [composing.defaultPrevented, confirming.defaultPrevented],
			};
		});
		assert.deepEqual(composed, {
			composing: true,
			keyCode: 229,
			prevented: [false, false],
		});
		// A submit would answer within a command; give one time to arrive.
		await sleep(1500);
		await expression("2 + (3 * 4)");
		await blank();
		// The same dispatch, as a plain Enter, is taken: the events above did reach the guard.
		const plain: boolean = await browser.execute(() => {
			const blank: HTMLInputElement | null =
				document.querySelector(".blank input");
			if (!blank) throw new Error("no open blank");
			const enter: KeyboardEvent = new KeyboardEvent("keydown", {
				bubbles: true,
				cancelable: true,
				key: "Enter",
				code: "Enter",
			});
			blank.dispatchEvent(enter);
			return enter.defaultPrevented;
		});
		assert.equal(plain, true);
		await expression("2 + (12)");
		await feedback("good");
		assert.deepEqual(await history(), ["2 + (3 * 4)"]);
	},
);

step("saved", "the step is saved in a version-3 progress file", async () => {
	const saved: {
		version: number;
		current_set: string;
		current: string;
		sessions: Record<string, unknown[]>;
	} = JSON.parse(readFileSync(progress, "utf8"));
	assert.equal(saved.version, 3);
	assert.equal(saved.current_set, "builtin");
	assert.equal(saved.current, "precedence");
	const records: [string, unknown[]][] = Object.entries(saved.sessions).filter(
		([key]: [string, unknown[]]) => key.includes("2 + (3 * 4)"),
	);
	assert.equal(records.length, 1, Object.keys(saved.sessions).join("\n"));
	assert.ok(records[0]![1].length > 0);
});

step(
	"resumed",
	"the app reopened on the same progress returns to the same question",
	async () => {
		await close(browser);
		browser = await open(setup, progress, "session-2");
		await drawn();
		await choose("Python 表达式");
		assert.equal(
			squeeze(await browser.$("header .where .title").getText()),
			"先乘后加",
		);
		await expression("2 + (12)");
		assert.deepEqual(await history(), ["2 + (3 * 4)"]);
	},
);

step("symbols", "a logic question takes 真 and ⊥ as answers", async () => {
	await browser.$('button[title="题目与题集"]').click();
	await (await labelled("section.sheet .actions button", "换一种语言")).click();
	await choose("命题逻辑");
	await pick("合取与否定");
	await expression("P & ~Q");
	await point("P");
	await answer("真");
	await feedback("good");
	await expression("True & ~Q");
	await point("Q");
	await answer("⊥");
	await feedback("good");
	await expression("True & ~False");
});
