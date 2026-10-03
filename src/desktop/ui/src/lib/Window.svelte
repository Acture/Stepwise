<script lang="ts">
	import { invoke } from "@tauri-apps/api/core";
	import { open } from "@tauri-apps/plugin-dialog";
	import App from "./App.svelte";
	import { fault } from "./fault";
	import type { Appearance } from "./protocol/Appearance";
	import type { Look } from "./protocol/Look";
	import type { Command, Host, View } from "./view";

	let view: View | null = $state(null);
	let appearance: Appearance | null = $state(null);

	// Calls reach the Rust side one at a time, in the order the student made them, and each
	// reply replaces the whole view or the whole appearance. Only a broken bridge rejects:
	// the commands themselves always answer.
	let queue: Promise<void> = Promise.resolve();

	function then(call: () => Promise<void>): void {
		queue = queue
			.then(call)
			.catch((error: unknown) =>
				fault(`窗口和程序之间的调用失败：${String(error)}`),
			);
	}

	function draw(call: () => Promise<View | null>): void {
		then(async () => {
			const next: View | null = await call();
			if (next) view = next;
		});
	}

	function dress(call: () => Promise<Appearance | null>): void {
		then(async () => {
			const next: Appearance | null = await call();
			if (next) appearance = next;
		});
	}

	/** Asks for a file with the window's own picker; null when the student cancels. */
	async function pick(
		name: string,
		extensions: string[],
	): Promise<string | null> {
		return open({
			multiple: false,
			directory: false,
			filters: [{ name, extensions }],
		});
	}

	// The first view and the look it is drawn in are asked for together, ahead of the first
	// command, and the board is drawn once both are in: it never shows in the wrong skin.
	then(async () => {
		const [first, look]: [View, Appearance] = await Promise.all([
			invoke<View>("view"),
			invoke<Appearance>("appearance"),
		]);
		view = first;
		appearance = look;
	});

	const host: Host = {
		send: (command: Command) => draw(() => invoke<View>("act", { command })),
		// Choosing a file is the window's; reading it and checking it are Rust's.
		openSet: () =>
			draw(async () => {
				const path: string | null = await pick("TOML 题集", ["toml"]);
				return path === null ? null : invoke<View>("load_set", { path });
			}),
		setLook: (look: Look) =>
			dress(() => invoke<Appearance>("set_look", { look })),
		importTheme: () =>
			dress(async () => {
				const path: string | null = await pick("VS Code 颜色主题", [
					"json",
					"jsonc",
				]);
				return path === null
					? null
					: invoke<Appearance>("import_theme", { path });
			}),
	};
</script>

{#if view && appearance}
	<App {view} {host} {appearance} />
{/if}
