<script lang="ts">
	import { radiogroup } from "./radiogroup";
	import { Check, Search } from "@lucide/svelte";
	import { face, stack, type Role } from "../skins";
	import type { Font } from "./protocol/Font";
	import type { FontPick } from "./protocol/FontPick";

	let {
		role,
		label,
		pick,
		fonts,
		editorFont,
		onpick,
	}: {
		role: Role;
		label: string;
		pick: FontPick;
		fonts: Font[];
		editorFont: string | null;
		onpick: (pick: FontPick) => void;
	} = $props();

	/** Faces that ship with the window, so they are there on every machine; each is noted by
	 * its Chinese name. Both draw every logic symbol a formula may hold. */
	const BUNDLED: { family: string; name: string }[] = [
		{ family: "LXGW WenKai", name: "霞鹜文楷" },
		{ family: "LXGW WenKai Mono", name: "霞鹜文楷等宽" },
	];

	/** A formula face is judged by the symbols it has to draw. */
	const SAMPLE: string = "(P ∧ ¬Q) → ⊥   2 ** -1";

	interface Option {
		key: string;
		pick: FontPick;
		name: string;
		/** The stack the option is shown in: its own face in front of the skin's. */
		shown: string;
		note: string | null;
		/** The face lacks some of ∧ ∨ → ↔ ¬ ⊥. */
		lacks: boolean;
		disabled: boolean;
	}

	interface Group {
		name: string;
		options: Option[];
	}

	let query: string = $state("");
	let list: HTMLElement | undefined = $state();

	function same(one: FontPick, other: FontPick): boolean {
		return one.kind === "family" && other.kind === "family"
			? one.family === other.family
			: one.kind === other.kind;
	}

	function lacks(family: string | null): boolean {
		return (
			role === "formula" &&
			fonts.some((font) => font.family === family && font.logic === false)
		);
	}

	function installed(font: Font): Option {
		return {
			key: `family:${font.family}`,
			pick: { kind: "family", family: font.family },
			name: font.family,
			shown: stack(font.family, role),
			note: null,
			lacks: lacks(font.family),
			disabled: false,
		};
	}

	let fixed: Option[] = $derived([
		{
			key: "skin",
			pick: { kind: "skin" },
			name: "跟随皮肤",
			shown: `var(--skin-${role})`,
			note: null,
			lacks: false,
			disabled: false,
		},
		{
			key: "editor",
			pick: { kind: "editor" },
			name: editorFont ?? "跟随编辑器",
			shown:
				editorFont === null ? `var(--skin-${role})` : stack(editorFont, role),
			note: editorFont === null ? "没有读到编辑器的字体" : "跟随编辑器",
			lacks: lacks(editorFont),
			disabled: editorFont === null,
		},
		...BUNDLED.map((bundled): Option => ({
			key: `family:${bundled.family}`,
			pick: { kind: "family", family: bundled.family },
			name: bundled.family,
			shown: stack(bundled.family, role),
			note: bundled.name,
			lacks: false,
			disabled: false,
		})),
	]);

	let groups: Group[] = $derived.by(() => {
		const needle: string = query.trim().toLowerCase();
		const found: Font[] = fonts.filter(
			(font) =>
				!BUNDLED.some((bundled) => bundled.family === font.family) &&
				font.family.toLowerCase().includes(needle),
		);
		// A formula lines up best in a monospaced face, so those come first there.
		return role === "formula"
			? [
					{
						name: "等宽字体",
						options: found.filter((font) => font.monospace).map(installed),
					},
					{
						name: "其他字体",
						options: found.filter((font) => !font.monospace).map(installed),
					},
				].filter((group) => group.options.length > 0)
			: [{ name: "已安装的字体", options: found.map(installed) }].filter(
					(group) => group.options.length > 0,
				);
	});

	/** What the heading names as in force; a family no longer installed is still named. */
	let chosen: { name: string; shown: string } = $derived(
		[...fixed, ...fonts.map(installed)].find((option) =>
			same(option.pick, pick),
		) ??
			(pick.kind === "family"
				? { name: pick.family, shown: stack(pick.family, role) }
				: fixed[0]!),
	);

	/** Said under the list when the face in force cannot draw every formula. */
	let short: string | null = $derived.by(() => {
		const family: string | null = face(pick, editorFont);
		return family !== null && lacks(family)
			? `${family} 缺少 ∧ ∨ → ↔ ¬ ⊥ 中的一些符号，缺的由皮肤的字体补上。`
			: null;
	});

	// The list opens scrolled to what is chosen, without moving the settings around it.
	$effect(() => {
		if (!list) return;
		const option: HTMLElement | null = list.querySelector(
			'[aria-checked="true"]',
		);
		if (option) list.scrollTop = option.offsetTop - list.clientHeight / 3;
	});
</script>

<div class="picker">
	<div class="heading">
		<h4>{label}</h4>
		<span class="current" style:font-family={chosen.shown}>{chosen.name}</span>
	</div>

	<label class="search">
		<Search size={15} strokeWidth={1.5} />
		<input
			type="search"
			bind:value={query}
			placeholder="搜索已安装的字体"
			spellcheck="false"
			autocomplete="off"
		/>
	</label>

	<div
		class="list"
		role="radiogroup"
		aria-label={label}
		bind:this={list}
		{@attach radiogroup}
	>
		{#snippet choice(option: Option)}
			<button
				role="radio"
				aria-checked={same(option.pick, pick)}
				disabled={option.disabled}
				title={option.lacks
					? "缺少 ∧ ∨ → ↔ ¬ ⊥ 中的一些符号，缺的由皮肤的字体补上"
					: option.name}
				onclick={() => onpick(option.pick)}
			>
				<span class="tick"
					>{#if same(option.pick, pick)}<Check
							size={15}
							strokeWidth={2}
						/>{/if}</span
				>
				<span class="name" style:font-family={option.shown}>{option.name}</span>
				{#if option.lacks}<span class="lacks">缺逻辑符号</span
					>{:else if option.note}<span class="note">{option.note}</span>{/if}
				{#if role === "formula"}
					<span class="sample" style:font-family={option.shown}>{SAMPLE}</span>
				{/if}
			</button>
		{/snippet}

		{#each fixed as option (option.key)}{@render choice(option)}{/each}
		{#each groups as group (group.name)}
			<p class="group">{group.name}</p>
			{#each group.options as option (option.key)}{@render choice(
					option,
				)}{/each}
		{:else}
			<p class="group">
				{fonts.length === 0
					? "没有读到已安装的字体。"
					: `没有名字里含「${query.trim()}」的字体。`}
			</p>
		{/each}
	</div>

	{#if short}<p class="short">{short}</p>{/if}
</div>

<style>
	.picker {
		display: flex;
		flex-direction: column;
		gap: 8px;
		min-width: 0;
	}

	.heading {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: 12px;
		min-width: 0;
	}

	h4 {
		flex: none;
		margin: 0;
		font-size: var(--size-body);
		font-weight: 400;
	}

	.current {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: var(--size-small);
		color: var(--mark-point);
	}

	.search {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 0 2px 5px;
		border-bottom: 1px solid var(--quiet-mark);
		color: var(--ink-faded);
	}

	.search:focus-within {
		border-bottom-color: var(--mark-point);
	}

	.search input {
		flex: 1;
		min-width: 0;
		border: 0;
		outline: none;
		background: transparent;
		font: inherit;
		font-size: var(--size-small);
		color: var(--ink);
		-webkit-user-select: text;
		user-select: text;
	}

	.search input::placeholder {
		color: var(--ink-faded);
	}

	.search input::-webkit-search-cancel-button {
		display: none;
	}

	.list {
		position: relative;
		height: 232px;
		overflow-y: auto;
		margin: 0 -6px;
		scrollbar-width: thin;
		scrollbar-color: var(--scrollbar) transparent;
	}

	button {
		display: grid;
		grid-template-columns: 18px minmax(0, 1fr) auto;
		align-items: baseline;
		column-gap: 6px;
		width: 100%;
		padding: 5px 6px;
		border-radius: 6px;
		text-align: left;
		color: var(--ink-faded);
	}

	button:not(:disabled):hover {
		color: var(--ink);
	}

	button[aria-checked="true"] {
		color: var(--mark-point);
	}

	.tick {
		align-self: center;
		display: grid;
		place-items: center;
	}

	.name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: var(--size-body);
	}

	.note,
	.lacks {
		font-size: 12px;
		white-space: nowrap;
		color: var(--ink-faded);
	}

	.sample {
		grid-column: 2 / -1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: pre;
		font-size: 15px;
		color: var(--ink-faded);
	}

	button[aria-checked="true"] .sample {
		color: var(--ink);
	}

	.group {
		margin: 10px 6px 4px;
		font-size: 12px;
		color: var(--ink-faded);
	}

	.short {
		margin: 0;
		font-size: var(--size-small);
		line-height: 1.6;
		color: var(--ink-faded);
	}
</style>
