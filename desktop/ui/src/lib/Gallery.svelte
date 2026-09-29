<script lang="ts">
	import App from "./App.svelte";
	import { screens } from "./fixtures";
	import type { Command } from "./view";

	function send(command: Command): void {
		console.info("command", command);
	}
</script>

<div class="gallery">
	{#each screens as screen (screen.name)}
		<figure>
			<div
				class="window"
				style:width="{screen.width}px"
				style:height="{screen.height}px"
			>
				<App view={screen.view} {send} skin="blackboard" />
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
		-webkit-user-select: text;
		user-select: text;
	}

	.gallery {
		display: flex;
		flex-wrap: wrap;
		gap: 40px;
		padding: 40px;
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
