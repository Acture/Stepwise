<script lang="ts">
	import { X } from "@lucide/svelte";
	import { skins, type Skin } from "../skins";
	import type { Catalog, Host } from "./view";

	let {
		catalog,
		host,
		skin,
		onskin,
		onclose,
	}: {
		catalog: Catalog;
		host: Host;
		skin: Skin;
		onskin: (skin: Skin) => void;
		onclose: () => void;
	} = $props();

	/** Every choice here changes the board, so the sheet gets out of the way after it. */
	function then(action: () => void): () => void {
		return () => {
			action();
			onclose();
		};
	}
</script>

<!-- A sheet over the board, not a second pane: it holds what changes the question in hand. -->
<div class="scrim" role="presentation" onclick={onclose}></div>
<section class="sheet" aria-label="题目与题集">
	<header>
		<h2 class="ink">{catalog.title}</h2>
		<button class="close" onclick={onclose} title="关闭"
			><X size={18} strokeWidth={1.5} /></button
		>
	</header>

	<ol class="questions">
		{#each catalog.questions as question, index (question.name)}
			<li>
				<button
					class:current={catalog.current === index}
					onclick={then(() => host.send({ kind: "pick", index }))}
				>
					<span class="number">{index + 1}</span>
					<span class="title">{question.title}</span>
					{#if question.proof}<span class="kind">证明</span>{/if}
				</button>
			</li>
		{/each}
	</ol>

	<div class="actions">
		<button onclick={then(() => host.send({ kind: "random" }))}>随机出题</button
		>
		<button onclick={then(() => host.openSet())}>打开题集文件</button>
		<button onclick={then(() => host.send({ kind: "leave" }))}
			>换一种语言</button
		>
	</div>

	<div class="skins" role="radiogroup" aria-label="皮肤">
		<span class="label">皮肤</span>
		{#each skins as choice (choice.id)}
			<button
				role="radio"
				aria-checked={skin === choice.id}
				class:chosen={skin === choice.id}
				onclick={() => onskin(choice.id)}>{choice.name}</button
			>
		{/each}
	</div>
</section>

<style>
	.scrim {
		position: absolute;
		inset: 0;
		z-index: 10;
		background: rgba(0, 0, 0, 0.35);
	}

	.sheet {
		position: absolute;
		z-index: 11;
		top: 0;
		right: 0;
		bottom: 0;
		width: min(360px, 88%);
		display: flex;
		flex-direction: column;
		gap: 14px;
		padding: 14px 18px 18px;
		background: var(--surface);
		background-size: var(--surface-size, auto);
		box-shadow: -18px 0 36px rgba(0, 0, 0, 0.45);
		overflow-y: auto;
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
	}

	h2 {
		margin: 0;
		font-size: var(--size-lead);
		font-weight: 400;
	}

	.close {
		display: grid;
		place-items: center;
		width: 30px;
		height: 30px;
		color: var(--ink-faded);
	}

	.questions {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 2px;
	}

	.questions button {
		display: grid;
		grid-template-columns: 2em 1fr auto;
		align-items: baseline;
		gap: 8px;
		width: 100%;
		padding: 7px 8px;
		border-radius: 6px;
		text-align: left;
		color: var(--ink-faded);
	}

	.questions button:hover {
		color: var(--ink);
	}

	.questions button.current {
		color: var(--mark-point);
	}

	.number {
		font-family: var(--formula);
		font-size: var(--size-small);
		text-align: right;
	}

	.kind {
		font-size: var(--size-small);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 6px 20px;
		padding-top: 12px;
		border-top: 1px solid var(--quiet-mark);
	}

	.actions button,
	.skins button {
		color: var(--ink-faded);
	}

	.actions button:hover,
	.skins button:hover {
		color: var(--ink);
	}

	.skins {
		display: flex;
		align-items: baseline;
		gap: 16px;
	}

	.label {
		font-size: var(--size-small);
		color: var(--ink-faded);
	}

	.skins button.chosen {
		color: var(--mark-point);
	}
</style>
