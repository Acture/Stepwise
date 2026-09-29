<script lang="ts">
	import { skins, type Skin } from "../skins";
	import App from "./App.svelte";
	import { screens } from "./fixtures";
	import type { Command, Host } from "./view";

	// The review page talks to no Rust side: commands are logged and the frames stay still.
	const host: Host = {
		send: (command: Command) => console.info("command", command),
		openSet: () => console.info("open a set"),
	};

	let skin: Skin = $state(
		skins.find((choice) => location.hash === `#${choice.id}`)?.id ??
			"blackboard",
	);
</script>

<nav class="skins">
	{#each skins as choice (choice.id)}
		<a
			href="#{choice.id}"
			class:chosen={skin === choice.id}
			onclick={() => (skin = choice.id)}>{choice.name}</a
		>
	{/each}
</nav>

<div class="gallery">
	{#each screens as screen (screen.name)}
		<figure>
			<div
				class="window"
				style:width="{screen.width}px"
				style:height="{screen.height}px"
			>
				<App
					view={screen.view}
					{host}
					{skin}
					onskin={(next: Skin) => (skin = next)}
				/>
				<span class="lights" aria-hidden="true"><i></i><i></i><i></i></span>
			</div>
			<figcaption>{screen.name} · {screen.width}×{screen.height}</figcaption>
		</figure>
	{/each}
</div>

<style>
	:global(body) {
		background: #d9d6cf;
		height: auto;
		font-family: system-ui, sans-serif;
	}

	.skins {
		display: flex;
		gap: 18px;
		padding: 24px 40px 0;
		font-size: 15px;
	}

	.skins a {
		color: #3d3a35;
		text-decoration: none;
	}

	.skins a.chosen {
		font-weight: 600;
		text-decoration: underline;
	}

	.gallery {
		display: flex;
		flex-wrap: wrap;
		gap: 40px;
		padding: 24px 40px 40px;
		align-items: flex-start;
	}

	figure {
		margin: 0;
	}

	.window {
		position: relative;
		border-radius: 11px;
		overflow: hidden;
		box-shadow:
			0 0 0 0.5px rgba(0, 0, 0, 0.45),
			0 22px 50px rgba(0, 0, 0, 0.35);
	}

	/* Stand-ins for the macOS window buttons the real title bar draws over the board. */
	.lights {
		position: absolute;
		top: 16px;
		left: 14px;
		display: flex;
		gap: 8px;
	}

	.lights i {
		width: 12px;
		height: 12px;
		border-radius: 50%;
		background: #ff5f57;
	}

	.lights i:nth-child(2) {
		background: #febc2e;
	}

	.lights i:nth-child(3) {
		background: #28c840;
	}

	figcaption {
		margin-top: 12px;
		font-size: 13px;
		color: #3d3a35;
	}
</style>
