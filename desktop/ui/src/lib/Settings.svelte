<script lang="ts">
	import { FileUp, X } from "@lucide/svelte";
	import { skins, themed, type Dress } from "../skins";
	import FacePicker from "./FacePicker.svelte";
	import MarkLayer from "./MarkLayer.svelte";
	import type { Mark } from "./marks";
	import type { Appearance } from "./protocol/Appearance";
	import type { FontPick } from "./protocol/FontPick";
	import type { Look } from "./protocol/Look";
	import SkinPreview from "./SkinPreview.svelte";
	import type { Host } from "./view";

	let {
		appearance,
		host,
		onclose,
	}: { appearance: Appearance; host: Host; onclose: () => void } = $props();

	/** The source the shell files an imported theme under. */
	const IMPORTED: string = "导入";

	interface Choice {
		id: string;
		name: string;
		dress: Dress;
	}

	interface Shelf {
		name: string;
		choices: Choice[];
	}

	/** The page's own skins, then each editor's themes, then the imported ones, which is
	 * where importing another belongs. */
	let shelves: Shelf[] = $derived.by(() => {
		const sources: Shelf[] = [];
		for (const theme of appearance.themes) {
			const choice: Choice = {
				id: theme.id,
				name: theme.name,
				dress: themed(theme),
			};
			const shelf: Shelf | undefined = sources.find(
				(each) => each.name === theme.source,
			);
			if (shelf) shelf.choices.push(choice);
			else sources.push({ name: theme.source, choices: [choice] });
		}
		return [
			{
				name: "内置",
				choices: skins.map((skin): Choice => ({
					id: skin.id,
					name: skin.name,
					dress: { skin: skin.id, properties: {} },
				})),
			},
			...sources.filter((shelf) => shelf.name !== IMPORTED),
			sources.find((shelf) => shelf.name === IMPORTED) ?? {
				name: IMPORTED,
				choices: [],
			},
		];
	});

	/** The skin in force: a look naming one that is gone is worn as the blackboard. */
	let chosen: string = $derived(
		shelves.some((shelf) =>
			shelf.choices.some((choice) => choice.id === appearance.look.skin),
		)
			? appearance.look.skin
			: "blackboard",
	);

	let panel: HTMLElement | undefined = $state();
	let rack: HTMLElement | undefined = $state();
	let cards: Record<string, HTMLElement | undefined> = $state({});

	// The skin in force is ringed in the pen of what is pointed at, as a chosen part is.
	let marks: Mark[] = $derived.by(() => {
		const card: HTMLElement | undefined = cards[chosen];
		return card
			? [
					{
						id: `chosen-${chosen}`,
						shape: "box",
						color: "var(--mark-point)",
						targets: [card],
						width: 2,
					},
				]
			: [];
	});

	/** Every choice is worn at once and kept by the shell; there is nothing to confirm. */
	function change(part: Partial<Look>): void {
		host.setLook({ ...appearance.look, ...part });
	}

	// The settings take the keys while they are open, so typing reaches the search and not a
	// blank left open on the board underneath.
	$effect(() => {
		panel?.focus();
	});

	/** The last thing the settings had to say, whether or not it is still said. */
	let said: string | null = null;

	// Something new to say is said under the title, which the list may have scrolled out of
	// sight: a theme file that could not be imported is picked from the foot of the skins.
	$effect(() => {
		const message: string | null = appearance.message;
		if (message !== null && message !== said)
			panel?.scrollTo({ top: 0, behavior: "smooth" });
		said = message;
	});
</script>

<div class="scrim" role="presentation" onclick={onclose}></div>
<section class="settings" aria-label="设置" tabindex="-1" bind:this={panel}>
	<header>
		<h2 class="ink">设置</h2>
		<button class="close" onclick={onclose} title="关闭"
			><X size={18} strokeWidth={1.5} /></button
		>
	</header>

	{#if appearance.message}
		<p class="message">{appearance.message}</p>
	{/if}

	<section class="part">
		<h3 class="ink">皮肤</h3>
		<div class="rack" role="radiogroup" aria-label="皮肤" bind:this={rack}>
			{#each shelves as shelf (shelf.name)}
				<div class="shelf">
					<h4>{shelf.name}</h4>
					<div class="cards">
						{#each shelf.choices as choice (choice.id)}
							<button
								class="card"
								role="radio"
								aria-checked={chosen === choice.id}
								title={choice.name}
								bind:this={cards[choice.id]}
								onclick={() => change({ skin: choice.id })}
							>
								<span class="frame"><SkinPreview dress={choice.dress} /></span>
								<span class="name">{choice.name}</span>
							</button>
						{/each}
						{#if shelf.name === IMPORTED}
							<button class="card import" onclick={() => host.importTheme()}>
								<span class="frame">
									<FileUp size={22} strokeWidth={1.4} />
									<span>VS Code 颜色主题</span>
								</span>
								<span class="name">导入主题文件</span>
							</button>
						{/if}
					</div>
				</div>
			{/each}
			<MarkLayer host={rack} {marks} />
		</div>
	</section>

	<section class="part">
		<h3 class="ink">字体</h3>
		<div class="pickers">
			<FacePicker
				role="prose"
				label="界面文字"
				pick={appearance.look.prose}
				fonts={appearance.fonts}
				editorFont={appearance.editorFont}
				onpick={(prose: FontPick) => change({ prose })}
			/>
			<FacePicker
				role="formula"
				label="公式"
				pick={appearance.look.formula}
				fonts={appearance.fonts}
				editorFont={appearance.editorFont}
				onpick={(formula: FontPick) => change({ formula })}
			/>
		</div>
	</section>
</section>

<style>
	.scrim {
		position: absolute;
		inset: 0;
		z-index: 10;
		background: var(--scrim);
	}

	/* Wider than the question list, and still short of the board's left edge, so the board
	   behind shows each choice as it is made. */
	.settings {
		position: absolute;
		z-index: 11;
		top: 0;
		right: 0;
		bottom: 0;
		width: min(620px, calc(100% - 72px));
		display: flex;
		flex-direction: column;
		gap: 26px;
		padding: 14px 22px 28px;
		background: var(--surface);
		background-size: var(--surface-size);
		box-shadow: var(--sheet-shadow);
		overflow-y: auto;
		scrollbar-width: thin;
		scrollbar-color: var(--scrollbar) transparent;
	}

	.settings:focus {
		outline: none;
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		min-height: 30px;
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

	.close:hover {
		color: var(--ink);
	}

	.message {
		margin: -10px 0 0;
		line-height: 1.6;
		color: var(--mark-wrong);
	}

	h3 {
		margin: 0 0 12px;
		font-size: var(--size-body);
		font-weight: 400;
	}

	.rack {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	h4 {
		margin: 0 0 6px;
		font-size: var(--size-small);
		font-weight: 400;
		color: var(--ink-faded);
	}

	.cards {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(124px, 1fr));
		gap: 8px 10px;
	}

	.card {
		display: flex;
		flex-direction: column;
		gap: 5px;
		min-width: 0;
		padding: 5px;
		text-align: left;
	}

	.frame {
		display: block;
		border-radius: 5px;
		box-shadow: 0 0 0 1px var(--quiet-mark);
	}

	.name {
		padding: 0 2px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: var(--size-small);
		color: var(--ink-faded);
		transition: color 120ms ease-out;
	}

	.card:hover .name {
		color: var(--ink);
	}

	.card[aria-checked="true"] .name {
		color: var(--mark-point);
	}

	.import .frame {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 6px;
		height: 86px;
		box-shadow: none;
		border: 1.5px dashed var(--quiet-mark);
		text-align: center;
		font-size: 12px;
		color: var(--ink-faded);
		transition:
			color 120ms ease-out,
			border-color 120ms ease-out;
	}

	.import:hover .frame {
		border-color: var(--ink-faded);
		color: var(--ink);
	}

	.pickers {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(230px, 1fr));
		gap: 22px 26px;
	}
</style>
