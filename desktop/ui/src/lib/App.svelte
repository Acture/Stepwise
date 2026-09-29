<script lang="ts">
	import type { Skin } from "../skins";
	import EntryBoard from "./EntryBoard.svelte";
	import EvaluationBoard from "./EvaluationBoard.svelte";
	import ProofBoard from "./ProofBoard.svelte";
	import type { Command, View } from "./view";

	let {
		view,
		send,
		skin,
	}: { view: View; send: (command: Command) => void; skin: Skin } = $props();
</script>

<!-- Grain a skin may lay over hand-drawn strokes (blackboard does: chalk breaks up at its
     edges). Streaked along the stroke rather than speckled across it. -->
<svg class="defs" aria-hidden="true">
	<filter
		id="chalk-grain"
		filterUnits="userSpaceOnUse"
		x="-10%"
		y="-10%"
		width="120%"
		height="120%"
	>
		<feTurbulence
			type="fractalNoise"
			baseFrequency="0.12 0.9"
			numOctaves="2"
			seed="7"
			result="grain"
		/>
		<feColorMatrix
			in="grain"
			type="matrix"
			values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 -1.1 1.4"
			result="mask"
		/>
		<feComposite in="SourceGraphic" in2="mask" operator="in" />
	</filter>
</svg>

<div class="board" data-skin={skin}>
	{#if view.kind === "entry"}
		<EntryBoard {view} {send} />
	{:else if view.kind === "evaluation"}
		<EvaluationBoard {view} {send} />
	{:else}
		<ProofBoard {view} {send} />
	{/if}
</div>

<style>
	.defs {
		position: absolute;
		width: 0;
		height: 0;
	}
</style>
