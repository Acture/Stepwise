<script lang="ts">
	import { untrack } from "svelte";
	import Header from "./Header.svelte";
	import Ledge from "./Ledge.svelte";
	import MarkLayer from "./MarkLayer.svelte";
	import type { Mark } from "./marks";
	import type { Host, ProofView } from "./view";

	let {
		view,
		host,
		onmenu,
	}: { view: ProofView; host: Host; onmenu: () => void } = $props();

	let sheet: HTMLElement | undefined = $state();
	let slot: HTMLElement | undefined = $state();
	let field: HTMLInputElement | undefined = $state();
	/** bars[row][level]: the spacer each scope bar is drawn along; the last row is the draft. */
	let bars: HTMLElement[][] = $state([]);
	let formulas: HTMLElement[] = $state([]);

	// The line being written holds the draft and Rust mirrors it, as in an expression's
	// blank. The field takes Rust's value once per line: when an accepted or undone line
	// changes the count, the next row is a new row.
	$effect(() => {
		if (!field) return;
		field.value = untrack(() => view.input);
		field.focus();
	});

	function commit(): void {
		if (field) host.send({ kind: "draft", text: field.value });
	}

	function key(event: KeyboardEvent): void {
		if (event.isComposing || event.keyCode === 229) return;
		if (event.key === "Enter") {
			event.preventDefault();
			host.send({ kind: "submit" });
		}
	}

	/** Every row's depth, the line being written included. */
	let depths: number[] = $derived([
		...view.lines.map((line) => line.depth),
		...(view.finished ? [] : [view.open]),
	]);

	let fitch: Mark[] = $derived.by(() => {
		const marks: Mark[] = [];
		const deepest: number = Math.max(0, ...depths);
		// One bar per scope: level 0 is the whole proof, each deeper level a run of rows
		// inside one assumption.
		for (let level: number = 0; level <= deepest; level++) {
			let start: number | null = null;
			depths.forEach((depth, row) => {
				const inside: boolean = depth >= level;
				if (inside && start === null) start = row;
				const ends: boolean =
					inside && (row === depths.length - 1 || depths[row + 1]! < level);
				if (ends && start !== null) {
					const targets: HTMLElement[] = [];
					for (let each: number = start; each <= row; each++) {
						const bar: HTMLElement | undefined = bars[each]?.[level];
						if (bar) targets.push(bar);
					}
					marks.push({
						id: `bar-${level}-${start}`,
						shape: "bar",
						color: "var(--ink-faded)",
						targets,
						width: 2.2,
					});
					start = null;
				}
			});
		}
		// Fitch's short rules close the premises and each assumption, joined to their bar.
		view.lines.forEach((line, row) => {
			const closesPremises: boolean =
				line.premise && !view.lines[row + 1]?.premise;
			if (!closesPremises && !line.assumption) return;
			const bar: HTMLElement | undefined = bars[row]?.[line.depth];
			const formula: HTMLElement | undefined = formulas[row];
			if (!bar || !formula) return;
			marks.push({
				id: `rule-${row}`,
				shape: "rule",
				color: "var(--ink-faded)",
				targets: [bar, formula],
				width: 2.2,
			});
		});
		if (slot && !view.finished) {
			marks.push({
				id: "next-line",
				shape: "underline",
				color: "var(--mark-write)",
				targets: [slot],
			});
		}
		return marks;
	});

	/** Premises are marked as such; every other rule is shown as the student wrote it. */
	function given(name: string): boolean {
		return name === "premise";
	}

	let judged: number | null = $derived(view.judged);
</script>

<Header course={view.course} {host} {onmenu} />

<main>
	{#if view.message}
		<p class="message">{view.message}</p>
	{/if}
	<p class="goal">
		<span class="label">目标</span>
		<span class="formula ink">{view.goal}</span>
		{#if view.finished}<span class="done">证明完成</span>{/if}
	</p>

	<div class="sheet" bind:this={sheet}>
		<ol class="lines">
			{#each view.lines as line, row (line.number)}
				<li class:judged={judged === line.number}>
					<span class="number">{line.number}</span>
					<span class="bars"
						>{#each { length: line.depth + 1 } as _, level (level)}<i
								bind:this={
									() => bars[row]?.[level],
									(bar) => ((bars[row] ??= [])[level] = bar)
								}
							></i>{/each}</span
					>
					<span class="formula-cell"
						><span class="formula ink" bind:this={formulas[row]}
							>{line.formula}</span
						></span
					>
					<span class="why">
						{#if given(line.rule)}<span class="given">前提</span>{:else}<span
								class="rule formula">{line.rule}</span
							>{#if line.references.length > 0}<span class="refs formula"
									>{line.references.join(",")}</span
								>{/if}{/if}
					</span>
				</li>
			{/each}
			{#if !view.finished}
				{#key view.lines.length}
					<li class="next">
						<span class="number">{view.lines.length + 1}</span>
						<span class="bars"
							>{#each { length: view.open + 1 } as _, level (level)}<i
									bind:this={
										() => bars[view.lines.length]?.[level],
										(bar) => ((bars[view.lines.length] ??= [])[level] = bar)
									}
								></i>{/each}</span
						>
						<span class="slot" bind:this={slot}>
							<input
								bind:this={field}
								class="formula"
								spellcheck="false"
								autocomplete="off"
								placeholder="公式 ; 规则 ; 引用行"
								aria-label="下一行"
								oninput={(event) => {
									if (!(event instanceof InputEvent && event.isComposing))
										commit();
								}}
								oncompositionend={commit}
								onkeydown={key}
							/>
						</span>
					</li>
				{/key}
			{/if}
		</ol>
		<MarkLayer host={sheet} marks={fitch} />
	</div>

	<!-- A verdict on an accepted line is about that line, not the one being written: it names
	     its line until the student starts the next, when the Rust side stops calling it one. -->
	<p class="feedback {view.feedback.tone}">
		{#if judged !== null}<span class="which">第 {judged} 行</span>{/if}{view
			.feedback.text}
	</p>

	{#if view.rules}
		<pre class="rules">{view.rules}</pre>
	{/if}
</main>

<Ledge
	tools={[
		{ label: "撤销一行", onclick: () => host.send({ kind: "undo" }) },
		{ label: "规则", onclick: () => host.send({ kind: "rules" }) },
	]}
	aside={[{ label: "帮助", onclick: () => host.send({ kind: "help" }) }]}
/>

<style>
	main {
		flex: 1;
		overflow-y: auto;
		padding: 12px 40px 24px;
		scrollbar-width: thin;
		scrollbar-color: var(--scrollbar) transparent;
	}

	.goal {
		display: flex;
		align-items: baseline;
		gap: 14px;
		margin: 0 0 14px;
	}

	.label,
	.done {
		font-size: var(--size-small);
		color: var(--ink-faded);
	}

	.done {
		color: var(--mark-good, var(--mark-point));
	}

	.goal .formula {
		font-size: 26px;
	}

	/* Justifications stay within reach of their formulas on a wide board. */
	.sheet {
		position: relative;
		max-width: 640px;
	}

	.lines {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	/* Fitch layout: line numbers, one bar per open scope, the formula, then why it holds. */
	li {
		display: grid;
		grid-template-columns: 2em auto 1fr auto;
		align-items: baseline;
		column-gap: 10px;
		min-height: 40px;
	}

	.number {
		text-align: right;
		font-family: var(--formula);
		font-size: var(--size-small);
		color: var(--ink-faded);
	}

	.bars {
		display: flex;
		align-self: stretch;
	}

	.bars i {
		width: 16px;
	}

	.formula-cell {
		padding: 6px 0;
	}

	/* Wide enough that a Fitch rule under a one-letter formula still reads as a rule. */
	.formula-cell .formula {
		display: inline-block;
		min-width: 3.2em;
		padding-right: 0.8em;
		font-size: 22px;
	}

	.why {
		display: flex;
		gap: 8px;
		align-items: baseline;
		font-size: var(--size-small);
		color: var(--ink-faded);
		white-space: nowrap;
	}

	.refs {
		font-size: 15px;
		color: var(--ink);
	}

	.judged .why,
	.judged .refs {
		color: var(--mark-good, var(--mark-point));
	}

	.message {
		margin: 0 0 14px;
		color: var(--mark-wrong);
		line-height: 1.6;
	}

	.slot {
		grid-column: 3 / 5;
		padding: 6px 0;
	}

	.slot input {
		width: 100%;
		border: 0;
		outline: none;
		background: transparent;
		font-size: 22px;
		color: var(--mark-write);
		caret-color: var(--mark-write);
		text-shadow: var(--ink-shadow, none);
	}

	.slot input::placeholder {
		color: color-mix(in srgb, var(--mark-write) 65%, transparent);
	}

	.feedback {
		max-width: 34em;
		margin: 16px 0 0;
		line-height: 1.7;
		text-wrap: pretty;
		color: var(--ink-faded);
		transition:
			color 160ms ease-out,
			opacity 160ms ease-out;
	}

	.feedback.bad {
		color: var(--mark-wrong);
	}

	.feedback.good {
		color: var(--mark-good, var(--mark-point));
	}

	.which {
		margin-right: 0.6em;
		font-size: var(--size-small);
	}

	.rules {
		margin: 16px 0 0;
		font-family: var(--prose);
		font-size: var(--size-small);
		line-height: 1.8;
		white-space: pre-wrap;
		color: var(--ink-faded);
	}
</style>
