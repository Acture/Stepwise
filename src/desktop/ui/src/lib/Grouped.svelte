<script lang="ts">
	import type { Snippet } from "svelte";
	import { group, type Piece } from "./typeset";

	let { texts, piece }: { texts: string[]; piece: Snippet<[number]> } =
		$props();

	let tree: Piece[] = $derived(group(texts));
</script>

<!-- No whitespace between the tags below: in a formula every space is the formula's own. -->
{#snippet nodes(
	list: Piece[],
)}{#each list as node, index (index)}{#if node.kind === "piece"}{@render piece(
				node.index,
			)}{:else}<span class:keep={node.keep}>{@render nodes(node.children)}</span
			>{/if}{/each}{/snippet}
{@render nodes(tree)}

<style>
	.keep {
		white-space: nowrap;
	}
</style>
