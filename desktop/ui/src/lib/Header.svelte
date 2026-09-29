<script lang="ts">
	import { ChevronLeft, ChevronRight, List } from "@lucide/svelte";
	import type { Course, Host } from "./view";

	let {
		course,
		host,
		onmenu,
	}: { course: Course; host: Host; onmenu: () => void } = $props();
</script>

<!-- The title bar is the board's top edge; the window buttons sit at its left. -->
<header data-tauri-drag-region>
	<div class="where" data-tauri-drag-region>
		<span class="language">{course.language}</span>
		<span class="title ink">{course.title}</span>
	</div>
	<nav>
		{#if course.count !== null}
			<span class="position"
				>{#if course.set}<span class="set">{course.set}</span
					>{/if}{course.position} / {course.count}</span
			>
		{/if}
		<button
			onmousedown={(event) => event.preventDefault()}
			disabled={!course.back}
			onclick={() => host.send({ kind: "previous" })}
			title="上一题"
		>
			<ChevronLeft size={20} strokeWidth={1.5} />
		</button>
		<button
			onmousedown={(event) => event.preventDefault()}
			disabled={!course.forward}
			onclick={() => host.send({ kind: "next" })}
			title="下一题"
		>
			<ChevronRight size={20} strokeWidth={1.5} />
		</button>
		<button onclick={onmenu} title="题目、题集与皮肤">
			<List size={19} strokeWidth={1.5} />
		</button>
	</nav>
</header>

<style>
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		height: 44px;
		padding: 0 14px 0 82px;
		flex: none;
	}

	.where {
		display: flex;
		align-items: baseline;
		gap: 10px;
		min-width: 0;
		white-space: nowrap;
	}

	.language {
		font-size: var(--size-small);
		color: var(--ink-faded);
	}

	.title {
		font-size: var(--size-lead);
		overflow: hidden;
		text-overflow: ellipsis;
	}

	nav {
		display: flex;
		align-items: center;
		gap: 2px;
		min-width: 0;
	}

	.position {
		display: flex;
		gap: 12px;
		margin-right: 8px;
		min-width: 0;
		font-size: var(--size-small);
		color: var(--ink-faded);
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}

	.set {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	nav button {
		display: grid;
		place-items: center;
		width: 30px;
		height: 30px;
		border-radius: 6px;
		color: var(--ink-faded);
		transition: color 120ms ease-out;
	}

	nav button:not(:disabled):hover {
		color: var(--ink);
	}
</style>
