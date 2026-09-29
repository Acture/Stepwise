<script lang="ts">
	import MarkLayer from "./MarkLayer.svelte";
	import type { Mark } from "./marks";
	import Grouped from "./Grouped.svelte";
	import type { Run } from "./view";

	let {
		runs,
		extents,
		selected,
		drafted,
		interactive,
		onpick,
	}: {
		runs: Run[];
		extents: Record<number, [number, number]>;
		/** Pointed at with the keyboard; ringed. */
		selected: number | null;
		/** Being answered on the line below, every occurrence one answer fills; each is
		 * underlined on its own so every blank is traceable to its place. */
		drafted: number[];
		/** False once the question has ended: nothing is left to pick. */
		interactive: boolean;
		onpick: (node: number) => void;
	} = $props();

	let host: HTMLElement | undefined = $state();
	let spans: HTMLElement[] = $state([]);
	let hovered: number | null = $state(null);

	function within(node: number | null, index: number): boolean {
		const range: [number, number] | undefined =
			node === null ? undefined : extents[node];
		return range !== undefined && index >= range[0] && index <= range[1];
	}

	function covering(node: number | null): HTMLElement[] {
		const range: [number, number] | undefined =
			node === null ? undefined : extents[node];
		return range ? spans.slice(range[0], range[1] + 1).filter(Boolean) : [];
	}

	let marks: Mark[] = $derived([
		...(selected === null
			? []
			: [
					{
						id: `selected-${selected}`,
						shape: "capsule" as const,
						color: "var(--mark-point)",
						targets: covering(selected),
					},
				]),
		...drafted.map((node: number): Mark => ({
			id: `drafted-${node}`,
			shape: "underline",
			color: "var(--mark-point)",
			targets: covering(node),
			width: 2,
		})),
	]);
</script>

<div
	class="line formula ink"
	class:still={!interactive}
	bind:this={host}
	role="group"
	onmouseleave={() => (hovered = null)}
>
	<Grouped texts={runs.map((run) => run.text)}>
		<!-- Every glyph of an operation picks that operation. The keyboard reaches the same
		     nodes with ↑ ↓, and hovering never filters by what is allowed. -->
		{#snippet piece(index: number)}{@const run =
				runs[index]!}{#if run.node === null || !interactive}<span
					bind:this={spans[index]}>{run.text}</span
				>{:else}<span
					class="node"
					class:lit={within(hovered, index)}
					class:pointed={within(selected, index) ||
						drafted.some((node: number) => within(node, index))}
					bind:this={spans[index]}
					onmouseenter={() => (hovered = run.node)}
					onclick={() => onpick(run.node!)}
					role="presentation">{run.text}</span
				>{/if}{/snippet}
	</Grouped>
	<MarkLayer {host} {marks} />
</div>

<style>
	/* A long formula wraps only between tokens, and a continuation hangs under the first line. */
	.line {
		position: relative;
		font-size: clamp(22px, 3.3cqi, var(--size-formula));
		line-height: 1.55;
		padding-left: 1.5em;
		text-indent: -1.5em;
	}

	.still {
		font-size: var(--size-history);
		color: var(--ink-faded);
	}

	.node {
		cursor: pointer;
		transition:
			color 120ms ease-out,
			text-shadow 120ms ease-out;
	}

	.lit {
		color: var(--ink-bright);
		text-shadow: var(--ink-glow, none);
	}

	.pointed {
		color: var(--mark-point);
	}
</style>
