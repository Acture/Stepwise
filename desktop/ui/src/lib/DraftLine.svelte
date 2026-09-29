<script lang="ts">
	import MarkLayer from "./MarkLayer.svelte";
	import type { Mark } from "./marks";
	import Grouped from "./Grouped.svelte";
	import type { Run } from "./view";

	let {
		runs,
		input,
		rejected,
		ondraft,
		onsubmit,
		oncancel,
	}: {
		runs: Run[];
		/** The draft as the Rust side holds it; the field is only written when they differ. */
		input: string;
		/** The last answer the rules turned down, while that verdict stands. */
		rejected: boolean;
		ondraft: (text: string) => void;
		onsubmit: () => void;
		oncancel: () => void;
	} = $props();

	let host: HTMLElement | undefined = $state();
	let field: HTMLInputElement | undefined = $state();
	let blanks: HTMLElement[] = $state([]);
	let typed: string = $state("");
	/** What was in the blank when it was last submitted: a mistake stays pink only until edited. */
	let submitted: string | null = $state(null);
	let composing: boolean = false;

	let wrong: boolean = $derived(rejected && typed === (submitted ?? input));

	// Rust owns the draft. Its value reaches the field only when it differs from what the
	// student typed — a limit or a dropped control character — and never mid-composition.
	$effect(() => {
		if (field && !composing && field.value !== input) {
			field.value = input;
			typed = input;
		}
	});

	$effect(() => {
		field?.focus();
	});

	function commit(): void {
		if (!field) return;
		typed = field.value;
		ondraft(field.value);
	}

	function key(event: KeyboardEvent): void {
		// WebKit delivers the Enter that confirms a candidate after compositionend with
		// isComposing false, so keyCode 229 is the guard that keeps it from submitting.
		if (event.isComposing || event.keyCode === 229) return;
		if (event.key === "Enter") {
			event.preventDefault();
			submitted = typed;
			onsubmit();
		} else if (event.key === "Escape") {
			event.preventDefault();
			oncancel();
		}
	}

	// A mistake is underlined twice, so it does not rest on hue alone.
	let marks: Mark[] = $derived([
		{
			id: wrong ? "blank-wrong" : "blank",
			shape: "underline",
			color: wrong ? "var(--mark-wrong)" : "var(--mark-write)",
			targets: blanks.filter(Boolean),
		},
		...(wrong
			? [
					{
						id: "blank-wrong-twice",
						shape: "underline" as const,
						color: "var(--mark-wrong)",
						targets: blanks.filter(Boolean),
						offset: 6,
						width: 2.2,
						delay: 160,
					},
				]
			: []),
	]);
</script>

<div class="line formula ink" bind:this={host}>
	<Grouped texts={runs.map((run) => run.text)}>
		{#snippet piece(index: number)}{@const run =
				runs[index]!}{#if run.blank === "input"}<span
					class="blank"
					class:wrong
					data-value={typed || " "}
					bind:this={blanks[index]}
					><input
						bind:this={field}
						class="formula"
						spellcheck="false"
						autocomplete="off"
						aria-label="这一步的值"
						oninput={(event) => {
							if (!(event instanceof InputEvent && event.isComposing)) commit();
						}}
						oncompositionstart={() => (composing = true)}
						oncompositionend={() => {
							composing = false;
							commit();
						}}
						onkeydown={key}
					/></span
				>{:else if run.blank === "mirror"}<span
					class="blank mirror"
					bind:this={blanks[index]}>{typed || " "}</span
				>{:else}<span>{run.text}</span>{/if}{/snippet}
	</Grouped>
	<MarkLayer {host} {marks} />
</div>

<style>
	.line {
		position: relative;
		font-size: clamp(22px, 3.3cqi, var(--size-formula));
		line-height: 1.55;
		padding-left: 1.5em;
		text-indent: -1.5em;
	}

	/* The blank hugs what was typed: an invisible copy of the value sizes it, and the field
	   lies over that copy, so the underline is as long as the answer and no longer. */
	.blank {
		position: relative;
		display: inline-block;
		min-width: 2.4ch;
		line-height: 1.15;
		text-indent: 0;
		color: var(--mark-write);
	}

	.blank::after {
		content: attr(data-value);
		visibility: hidden;
		white-space: pre;
		padding: 0 0.15ch;
	}

	.blank input {
		position: absolute;
		inset: 0;
		width: 100%;
		padding: 0 0.15ch;
		border: 0;
		outline: none;
		background: transparent;
		font: inherit;
		color: inherit;
		caret-color: currentColor;
		text-shadow: inherit;
	}

	.blank.wrong {
		color: var(--mark-wrong);
	}

	.mirror::after {
		content: none;
	}

	.mirror {
		text-align: center;
		white-space: pre;
	}
</style>
