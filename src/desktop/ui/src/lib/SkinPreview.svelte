<script lang="ts">
	import { wear, type Dress } from "../skins";
	import MarkLayer from "./MarkLayer.svelte";
	import type { Mark } from "./marks";

	/** The skin or theme to show, worn as the board would wear it. */
	let { dress }: { dress: Dress } = $props();

	let host: HTMLElement | undefined = $state();
	let pointed: HTMLElement | undefined = $state();
	let wrong: HTMLElement | undefined = $state();
	let blank: HTMLElement | undefined = $state();

	// The board's own marks at the size of a card: what is pointed at, a mistake underlined
	// twice, and the blank being written.
	let marks: Mark[] = $derived(
		pointed && wrong && blank
			? [
					{
						id: "pointed",
						shape: "capsule",
						color: "var(--mark-point)",
						targets: [pointed],
						width: 1.6,
					},
					{
						id: "wrong",
						shape: "underline",
						color: "var(--mark-wrong)",
						targets: [wrong],
						width: 1.6,
					},
					{
						id: "wrong-twice",
						shape: "underline",
						color: "var(--mark-wrong)",
						targets: [wrong],
						width: 1.3,
						offset: 3,
						delay: 120,
					},
					{
						id: "blank",
						shape: "underline",
						color: "var(--mark-write)",
						targets: [blank],
						width: 1.6,
						delay: 200,
					},
				]
			: [],
	);
</script>

<!-- A board in miniature: the line in hand with a part pointed at, a rejected value, and the
     blank written again and accepted. -->
<div
	class="preview"
	data-skin={dress.skin}
	{@attach wear(dress.properties)}
	bind:this={host}
	aria-hidden="true"
>
	<div class="lines formula ink">
		<div>2 + <span class="pointed" bind:this={pointed}>3 * 2</span></div>
		<div>2 + <span class="wrong" bind:this={wrong}>5</span></div>
		<div>
			2 + <span class="write" bind:this={blank}>6</span><span class="good"
				>✓</span
			>
		</div>
	</div>
	<div class="tray"></div>
	<MarkLayer {host} {marks} />
</div>

<style>
	.preview {
		position: relative;
		display: flex;
		flex-direction: column;
		height: 86px;
		overflow: hidden;
		border-radius: 5px;
		color: var(--ink);
		background: var(--surface);
		background-size: var(--surface-size);
	}

	.lines {
		flex: 1;
		padding: 6px 10px 0;
		font-size: 14px;
		line-height: 1.42;
		white-space: nowrap;
	}

	/* Room inside the ring, which is drawn around the text's own box. */
	.pointed {
		padding: 0 0.3ch;
		color: var(--mark-point);
	}

	.wrong {
		color: var(--mark-wrong);
	}

	.write {
		padding: 0 0.2ch;
		color: var(--mark-write);
	}

	.good {
		margin-left: 0.7ch;
		color: var(--mark-good);
	}

	.tray {
		flex: none;
		height: 9px;
		background: var(--tray);
	}
</style>
