<script lang="ts">
	import { invoke } from "@tauri-apps/api/core";
	import { open } from "@tauri-apps/plugin-dialog";
	import { remember, remembered, type Skin } from "../skins";
	import App from "./App.svelte";
	import { fault } from "./fault";
	import type { Command, Host, View } from "./view";

	let view: View | null = $state(null);
	let skin: Skin = $state(remembered());

	// Commands reach the Rust side one at a time, in the order the student gave them, and
	// each reply replaces the whole view. Only a broken bridge rejects: the commands
	// themselves always answer with a view.
	let queue: Promise<void> = Promise.resolve();

	function then(call: () => Promise<View | null>): void {
		queue = queue
			.then(async () => {
				const next: View | null = await call();
				if (next) view = next;
			})
			.catch((error: unknown) =>
				fault(`窗口和程序之间的调用失败：${String(error)}`),
			);
	}

	// The first view is asked for like any other, so it queues ahead of the first command.
	then(() => invoke<View>("view"));

	const host: Host = {
		send: (command: Command) => then(() => invoke<View>("act", { command })),
		// Choosing a file is the window's; reading it and checking the set are Rust's.
		openSet: () =>
			then(async () => {
				const path: string | null = await open({
					multiple: false,
					directory: false,
					filters: [{ name: "TOML 题集", extensions: ["toml"] }],
				});
				return path === null ? null : invoke<View>("load_set", { path });
			}),
	};
</script>

{#if view}
	<App
		{view}
		{host}
		{skin}
		onskin={(next: Skin) => {
			skin = next;
			remember(next);
		}}
	/>
{/if}
