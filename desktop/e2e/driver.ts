// The installed window under tauri-driver. The job installs the app; this starts the driver and
// opens sessions on the executable the installer wrote. Nothing here builds or changes the app.

import {
	execFileSync,
	spawn,
	spawnSync,
	type ChildProcess,
} from "node:child_process";
import { closeSync, mkdirSync, openSync, writeFileSync } from "node:fs";
import { connect, type Socket } from "node:net";
import { join } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { remote } from "webdriverio";

/** What the job hands the suite. */
export interface Setup {
	/** The executable the installer wrote, or the AppImage. */
	app: string;
	/** Where the screenshots and the sessions' descriptions go; the job uploads it. */
	evidence: string;
	/** tauri-driver, and on Windows the msedgedriver matching the WebView2 runtime. */
	driver: string;
	native: string | null;
}

export function configure(): Setup {
	const app: string | undefined = process.env.STEPWISE_APP;
	if (!app) throw new Error("STEPWISE_APP must name the installed executable");
	const evidence: string = process.env.STEPWISE_EVIDENCE ?? "evidence";
	mkdirSync(evidence, { recursive: true });
	return {
		app,
		evidence,
		driver: process.env.TAURI_DRIVER ?? "tauri-driver",
		native: process.env.STEPWISE_NATIVE_DRIVER ?? null,
	};
}

/** The integrity label of this process's own token, which the driver and the app inherit. */
export function integrity(): string {
	const groups: string = execFileSync(
		"whoami",
		["/groups", "/fo", "csv", "/nh"],
		{ encoding: "utf8" },
	);
	const label: string | undefined = groups
		.split(/\r?\n/)
		.find((line: string) => line.includes("S-1-16-"));
	if (!label)
		throw new Error(`whoami /groups names no integrity label:\n${groups}`);
	return label;
}

const PORT: number = 4444;

function reachable(port: number): Promise<boolean> {
	return new Promise((resolve: (open: boolean) => void) => {
		const socket: Socket = connect({ host: "127.0.0.1", port });
		socket.once("connect", () => {
			socket.destroy();
			resolve(true);
		});
		socket.once("error", () => {
			socket.destroy();
			resolve(false);
		});
	});
}

/** Starts tauri-driver and waits until it listens. Its output, and the native driver's, goes
 * to a file: on Windows this process may have no way to write to the job's log. */
export async function start(setup: Setup): Promise<ChildProcess> {
	const log: number = openSync(join(setup.evidence, "tauri-driver.log"), "a");
	const driver: ChildProcess = spawn(
		setup.driver,
		setup.native ? ["--native-driver", setup.native] : [],
		{ stdio: ["ignore", log, log] },
	);
	closeSync(log);
	const failed: Promise<never> = new Promise(
		(_: unknown, reject: (error: Error) => void) => {
			driver.once("error", reject);
			driver.once("exit", (code: number | null) =>
				reject(new Error(`tauri-driver exited with ${code}`)),
			);
		},
	);
	const listening: Promise<void> = (async () => {
		const deadline: number = Date.now() + 30_000;
		while (!(await reachable(PORT))) {
			if (Date.now() > deadline)
				throw new Error(`tauri-driver is not listening on ${PORT}`);
			await sleep(250);
		}
	})();
	await Promise.race([listening, failed]);
	return driver;
}

/** A session on the installed app, launched with its progress written to `progress`. */
export async function open(
	setup: Setup,
	progress: string,
	name: string,
): Promise<WebdriverIO.Browser> {
	const browser: WebdriverIO.Browser = await remote({
		hostname: "127.0.0.1",
		port: PORT,
		logLevel: "warn",
		// Creating the session waits for the app, its WebView and the page. A retry would
		// launch a second app, so there is none.
		connectionRetryTimeout: 180_000,
		connectionRetryCount: 0,
		capabilities: {
			"tauri:options": {
				application: setup.app,
				args: ["--progress-file", progress],
			},
		},
	});
	// What the driver reports of the session: the WebView's own version among it.
	writeFileSync(
		join(setup.evidence, `${name}.json`),
		JSON.stringify(browser.capabilities, null, "\t") + "\n",
	);
	return browser;
}

/** The app's processes still running. */
function running(): string[] {
	if (process.platform === "win32") {
		return execFileSync("tasklist", ["/fo", "csv", "/nh"], { encoding: "utf8" })
			.split(/\r?\n/)
			.filter((line: string) => /^"stepwise-desktop\.exe"/i.test(line));
	}
	return spawnSync("pgrep", ["-a", "-f", "stepwise-desktop"], {
		encoding: "utf8",
	})
		.stdout.split("\n")
		.filter(Boolean);
}

/** Ends a session and waits until the app has quit, so the next session is a relaunch and
 * not a second window over the same progress file. */
export async function close(browser: WebdriverIO.Browser): Promise<void> {
	await browser.deleteSession();
	const deadline: number = Date.now() + 30_000;
	for (;;) {
		const left: string[] = running();
		if (left.length === 0) return;
		if (Date.now() > deadline)
			throw new Error(`the app is still running:\n${left.join("\n")}`);
		await sleep(250);
	}
}
