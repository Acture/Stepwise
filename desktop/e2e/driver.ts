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

/** Windows' own tools, by path: the job's PATH puts Git's coreutils, whoami among them, first. */
function system(tool: string): string {
	return join(process.env.SystemRoot ?? "C:\\Windows", "System32", tool);
}

/** The integrity label of this process's own token, which the driver and the app inherit. */
export function integrity(): string {
	const groups: string = execFileSync(
		system("whoami.exe"),
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
				// One token: msedgedriver passes the app only what reads as a switch.
				args: [`--progress-file=${progress}`],
			},
		},
	});
	// What the driver reports of the session: on Windows the WebView2 runtime's version among it;
	// on Linux the version wry gives its web context, not WebKitGTK's.
	writeFileSync(
		join(setup.evidence, `${name}.json`),
		JSON.stringify(browser.capabilities, null, "\t") + "\n",
	);
	return browser;
}

/** The app's processes still running: on Unix found by the progress file only this suite hands
 * it, on Windows by image name, since tasklist shows no command line and nothing else on the
 * runner runs the app. */
function running(progress: string): string[] {
	if (process.platform === "win32") {
		return execFileSync(system("tasklist.exe"), ["/fo", "csv", "/nh"], {
			encoding: "utf8",
		})
			.split(/\r?\n/)
			.filter((line: string) => /^"stepwise-desktop\.exe"/i.test(line));
	}
	const pattern: string = progress.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
	return spawnSync("pgrep", ["-a", "-f", "--", pattern], { encoding: "utf8" })
		.stdout.split("\n")
		.filter(Boolean);
}

async function quit(progress: string, timeout: number): Promise<boolean> {
	const deadline: number = Date.now() + timeout;
	while (running(progress).length > 0) {
		if (Date.now() > deadline) return false;
		await sleep(250);
	}
	return true;
}

/** Ends a session and waits until the app has quit, so the next session is a relaunch and
 * not a second window over the same progress file. */
export async function close(
	browser: WebdriverIO.Browser,
	progress: string,
): Promise<void> {
	await browser.deleteSession();
	if (await quit(progress, 10_000)) return;
	// WebKitWebDriver ends the process it started, which for an AppImage is the AppImage's
	// runtime: the app that runtime started is left running, and is ended here.
	if (process.platform !== "win32") {
		for (const line of running(progress)) {
			console.log(`Ending what the driver left running: ${line}`);
			process.kill(Number(line.split(" ")[0]), "SIGTERM");
		}
		if (await quit(progress, 10_000)) return;
	}
	throw new Error(`the app is still running:\n${running(progress).join("\n")}`);
}
