<script lang="ts">
	import { ChevronRight } from "@lucide/svelte";
	import Ledge from "./Ledge.svelte";
	import MarkLayer from "./MarkLayer.svelte";
	import type { Mark } from "./marks";
	import { skins, type Skin } from "../skins";
	import type { EntryView, Host } from "./view";

	let {
		view,
		host,
		skin,
		onskin,
	}: { view: EntryView; host: Host; skin: Skin; onskin: (skin: Skin) => void } =
		$props();

	/** The skin after this one, for the ledge's single switch. */
	let following: (typeof skins)[number] = $derived(
		skins[
			(skins.findIndex((choice) => choice.id === skin) + 1) % skins.length
		]!,
	);

	let demo: HTMLElement | undefined = $state();
	let picked: HTMLElement | undefined = $state();
	let blank: HTMLElement | undefined = $state();
	let choices: HTMLElement | undefined = $state();
	let boxes: HTMLElement[] = $state([]);
	let hovered: number | null = $state(null);

	// The whole gesture in the real colours: point at a part, then write its value below.
	let demoMarks: Mark[] = $derived(
		picked && blank
			? [
					{
						id: "demo-picked",
						shape: "capsule",
						color: "var(--mark-point)",
						targets: [picked],
					},
					{
						id: "demo-blank",
						shape: "underline",
						color: "var(--mark-write)",
						targets: [blank],
						delay: 450,
					},
				]
			: [],
	);

	let choiceMarks: Mark[] = $derived(
		boxes.filter(Boolean).map((box, index) => ({
			id: `choice-${index}-${hovered === index ? "on" : "off"}`,
			shape: "box",
			color: hovered === index ? "var(--ink)" : "var(--quiet-mark)",
			targets: [box],
			width: 1.8,
		})),
	);

	const languages: {
		language: "python" | "logic";
		name: string;
		sample: string;
		note: string;
	}[] = [
		{
			language: "python",
			name: "Python 表达式",
			sample: "7 % -3 + 2 ** 3",
			note: "运算、比较、and / or 短路",
		},
		{
			language: "logic",
			name: "命题逻辑",
			sample: "¬P ∨ (Q → R)",
			note: "真值求值，以及自然演绎证明",
		},
	];
</script>

<header data-tauri-drag-region><span class="wordmark">Stepwise</span></header>

<main>
	<section class="idea">
		<div class="demo ink" bind:this={demo} aria-hidden="true">
			<div class="formula">5 + <span bind:this={picked}>2 * 3</span></div>
			<div class="formula">
				5 + <span class="demo-blank" bind:this={blank}>6</span>
			</div>
			<MarkLayer host={demo} marks={demoMarks} />
		</div>
		<h1 class="ink">点一处子式，在下一行写出这一步的值。</h1>
		{#if view.set}
			<p class="opened">
				题集「{view.set}」已打开：选一种语言，按题集顺序练习。
			</p>
		{/if}
	</section>

	<section class="choices" bind:this={choices}>
		{#each languages as choice, index (choice.language)}
			<button
				class="choice"
				bind:this={boxes[index]}
				onclick={() => host.send({ kind: "choose", language: choice.language })}
				onmouseenter={() => (hovered = index)}
				onmouseleave={() => (hovered = null)}
				onfocus={() => (hovered = index)}
				onblur={() => (hovered = null)}
			>
				<span class="text">
					<span class="name ink">{choice.name}</span>
					<span class="sample formula">{choice.sample}</span>
					<span class="note">{choice.note}</span>
				</span>
				<ChevronRight size={20} strokeWidth={1.5} />
			</button>
		{/each}
		<MarkLayer host={choices} marks={choiceMarks} />
	</section>

	{#if view.message}
		<p class="message">{view.message}</p>
	{/if}
</main>

<Ledge
	tools={[{ label: "打开题集文件", onclick: () => host.openSet() }]}
	aside={[
		{ label: `换成${following.name}皮肤`, onclick: () => onskin(following.id) },
	]}
/>

<style>
	header {
		display: flex;
		align-items: center;
		height: 44px;
		padding-left: 82px;
		flex: none;
	}

	.wordmark {
		font-size: var(--size-body);
		color: var(--ink-faded);
		letter-spacing: 0.02em;
	}

	/* Wide board: the idea on the left, where to start on the right. Narrow: one column. */
	main {
		flex: 1;
		overflow-y: auto;
		padding: 8px 44px 24px;
		display: grid;
		grid-template-columns: 1fr;
		align-content: start;
		gap: 12px 64px;
	}

	@container board (min-width: 720px) {
		main {
			grid-template-columns: 1.1fr 1fr;
			align-content: center;
			align-items: center;
		}
	}

	.demo {
		position: relative;
		font-size: 30px;
		line-height: 1.6;
		margin-bottom: 18px;
	}

	.demo > div:first-child {
		color: var(--ink-faded);
	}

	.demo-blank {
		display: inline-block;
		min-width: 2.4ch;
		padding: 0 0.3ch;
		color: var(--mark-write);
	}

	h1 {
		margin: 0;
		font-size: 22px;
		font-weight: 400;
		line-height: 1.5;
		text-wrap: pretty;
	}

	.choices {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.choice {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		width: 100%;
		padding: 14px 18px;
		text-align: left;
		color: var(--ink-faded);
	}

	.choice:focus-visible {
		outline: none;
	}

	.text {
		display: grid;
		justify-items: start;
		row-gap: 4px;
	}

	.name {
		font-size: var(--size-lead);
		color: var(--ink);
	}

	.sample {
		font-size: var(--size-body);
		color: var(--ink-faded);
	}

	.note {
		font-size: var(--size-small);
		color: var(--ink-faded);
	}

	.opened {
		margin: 14px 0 0;
		font-size: var(--size-small);
		color: var(--ink-faded);
		line-height: 1.6;
	}

	.message {
		grid-column: 1 / -1;
		margin: 8px 0 0;
		color: var(--mark-wrong);
		line-height: 1.7;
	}
</style>
