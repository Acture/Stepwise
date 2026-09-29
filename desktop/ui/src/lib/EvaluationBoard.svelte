<script lang="ts">
	import DraftLine from "./DraftLine.svelte";
	import Expression from "./Expression.svelte";
	import Header from "./Header.svelte";
	import Ledge from "./Ledge.svelte";
	import MarkLayer from "./MarkLayer.svelte";
	import type { Mark } from "./marks";
	import Grouped from "./Grouped.svelte";
	import { pieces } from "./typeset";
	import type { EvaluationView, Host } from "./view";

	let {
		view,
		host,
		onmenu,
	}: { view: EvaluationView; host: Host; onmenu: () => void } = $props();

	let work: HTMLElement | undefined = $state();
	let next: HTMLElement | undefined = $state();
	let nextRow: HTMLElement | undefined = $state();

	/** The nodes the open blanks stand for — the one clicked and every other occurrence of
	 * its name — each underlined in the line above. */
	let drafted: number[] = $derived(
		(view.draft?.runs ?? [])
			.filter((run) => run.blank !== null && run.node !== null)
			.map((run) => run.node!),
	);

	// What to do next is pointed at, in the chalk of what is pointed at.
	let nextMarks: Mark[] = $derived(
		next
			? [
					{
						id: "next",
						shape: "capsule",
						color: "var(--mark-point)",
						targets: [next],
					},
				]
			: [],
	);

	// New lines are written at the bottom of the board; keep them in view.
	$effect(() => {
		void view;
		work?.scrollTo({ top: work.scrollHeight });
	});
</script>

<Header course={view.course} {host} {onmenu} />

<main bind:this={work}>
	{#if view.message}
		<p class="message">{view.message}</p>
	{/if}
	{#if view.bindings.length > 0}
		<p class="bindings">
			<span class="label">设</span>
			{#each view.bindings as binding (binding)}<span class="formula"
					>{binding}</span
				>{/each}
		</p>
	{/if}

	{#if view.history.length > 0}
		<ol class="history formula">
			{#each view.history as line, index (index)}
				{#if line.kind === "expression"}
					<li>
						<Grouped texts={pieces(line.text)}>
							{#snippet piece(index: number)}{pieces(line.text)[
									index
								]}{/snippet}
						</Grouped>
					</li>
				{:else}
					<!-- The record keeps what was taken back and says so. -->
					<li class="taken-back">{line.text}</li>
				{/if}
			{/each}
		</ol>
	{/if}

	<Expression
		runs={view.current}
		extents={view.extents}
		selected={view.draft ? null : view.selected}
		{drafted}
		interactive={view.ending === null}
		onpick={(node) => host.send({ kind: "select", node })}
	/>

	{#if view.draft}
		{#key view.edition}
			<DraftLine
				runs={view.draft.runs}
				input={view.draft.input}
				rejected={view.feedback.tone === "bad"}
				ondraft={(text) =>
					host.send({ kind: "draft", text, edition: view.edition })}
				onsubmit={() => host.send({ kind: "submit" })}
				oncancel={() => host.send({ kind: "cancel" })}
			/>
		{/key}
	{/if}

	{#if view.ending}
		<p class="ending">
			<span class="done">完成</span>
			<span class="formula ink">{view.ending}</span>
		</p>
	{/if}

	<p class="feedback {view.feedback.tone}">{view.feedback.text}</p>

	{#if view.ending}
		<div class="next-row" bind:this={nextRow}>
			<button
				class="next ink"
				bind:this={next}
				onclick={() => host.send({ kind: "next" })}>下一题</button
			>
			<MarkLayer host={nextRow} marks={nextMarks} />
		</div>
	{/if}
</main>

<Ledge
	tools={[
		{ label: "撤销", onclick: () => host.send({ kind: "undo" }) },
		{ label: "重来", onclick: () => host.send({ kind: "reset" }) },
		{
			label: "提示",
			onclick: () => host.send({ kind: "hint" }),
			disabled: view.ending !== null,
		},
	]}
	aside={[{ label: "帮助", onclick: () => host.send({ kind: "help" }) }]}
/>

<style>
	main {
		flex: 1;
		overflow-y: auto;
		padding: 14px 40px 24px;
		display: flex;
		flex-direction: column;
		scrollbar-width: thin;
		scrollbar-color: var(--scrollbar) transparent;
	}

	.bindings {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 2px 18px;
		margin: 0 0 18px;
		font-size: var(--size-body);
		color: var(--ink-faded);
	}

	.bindings span {
		white-space: nowrap;
	}

	.label,
	.done {
		font-size: var(--size-small);
		color: var(--ink-faded);
	}

	/* Every earlier step on its own line; a wrapped step hangs under its first line. */
	.history {
		list-style: none;
		margin: 0 0 6px;
		padding: 0;
		font-size: var(--size-lead);
		line-height: 1.4;
		color: var(--ink-faded);
	}

	.history li {
		padding-left: 1.5em;
		text-indent: -1.5em;
	}

	.history li + li {
		margin-top: 0.4em;
	}

	.taken-back {
		font-family: var(--prose);
		font-size: var(--size-small);
	}

	.message {
		margin: 0 0 14px;
		color: var(--mark-wrong);
		line-height: 1.6;
	}

	.ending {
		display: flex;
		align-items: baseline;
		flex-wrap: wrap;
		gap: 4px 14px;
		margin: 10px 0 0;
		font-size: clamp(22px, 3.3cqi, var(--size-formula));
	}

	.feedback {
		max-width: 34em;
		margin: 14px 0 0;
		font-size: var(--size-body);
		line-height: 1.7;
		text-wrap: pretty;
		color: var(--ink-faded);
	}

	.feedback.bad {
		color: var(--mark-wrong);
	}

	.feedback.good {
		color: var(--mark-good, var(--mark-point));
	}

	.next-row {
		position: relative;
		align-self: flex-start;
		margin-top: 16px;
	}

	.next {
		padding: 4px 12px;
		font-size: var(--size-lead);
	}
</style>
