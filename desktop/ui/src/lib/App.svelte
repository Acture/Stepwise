<script lang="ts" module>
	/** What may lie over the board: the question list, or the settings. */
	export type Panel = "questions" | "settings";
</script>

<script lang="ts">
	import { untrack } from "svelte";
	import { dress, wear, wearDress, type Dress } from "../skins";
	import EntryBoard from "./EntryBoard.svelte";
	import EvaluationBoard from "./EvaluationBoard.svelte";
	import ProofBoard from "./ProofBoard.svelte";
	import type { Appearance } from "./protocol/Appearance";
	import Settings from "./Settings.svelte";
	import Sheet from "./Sheet.svelte";
	import type { Command, Host, View } from "./view";

	let {
		view,
		host,
		appearance,
		opened = null,
	}: {
		view: View;
		host: Host;
		appearance: Appearance;
		/** A panel open from the start; only the design review asks for one. */
		opened?: Panel | null;
	} = $props();

	// Only where the board starts: a panel is the student's to open and close after that.
	let panel: Panel | null = $state(untrack(() => opened));
	let worn: Dress = $derived(dress(appearance));
	wearDress(() => worn);

	/** The keys an expression question binds when no field has the caret. The same keys as
	 * the terminal, so a student moving between the two does not relearn them. */
	const keys: Record<string, Command> = {
		ArrowDown: { kind: "step", forward: true },
		j: { kind: "step", forward: true },
		ArrowUp: { kind: "step", forward: false },
		k: { kind: "step", forward: false },
		Enter: { kind: "submit" },
		Escape: { kind: "cancel" },
		u: { kind: "undo" },
		r: { kind: "reset" },
		n: { kind: "next" },
		p: { kind: "previous" },
		h: { kind: "hint" },
		"?": { kind: "help" },
	};

	function key(event: KeyboardEvent): void {
		if (event.isComposing || event.keyCode === 229) return;
		if (panel) {
			if (event.key === "Escape") panel = null;
			return;
		}
		// In a proof every letter belongs to the line being written; changing question takes
		// Ctrl, as in the terminal.
		if (event.ctrlKey && (event.key === "n" || event.key === "p")) {
			event.preventDefault();
			host.send({ kind: event.key === "n" ? "next" : "previous" });
			return;
		}
		// A field takes its own keys, and a focused button or link its own Tab, Space and Enter.
		const owned: boolean =
			event.target instanceof HTMLInputElement ||
			event.target instanceof HTMLButtonElement ||
			event.target instanceof HTMLAnchorElement;
		if (owned || event.metaKey || event.ctrlKey || event.altKey) return;
		// While a blank or a proof line is open every character belongs to it, as in the
		// terminal: Enter and Escape still check and cancel, and a typed character puts the
		// caret back in the field instead of acting as a command on the work in hand.
		if (view.kind === "proof" || (view.kind === "evaluation" && view.draft)) {
			if (
				view.kind === "evaluation" &&
				(event.key === "Enter" || event.key === "Escape")
			) {
				event.preventDefault();
				host.send({ kind: event.key === "Enter" ? "submit" : "cancel" });
			} else if (event.key.length === 1) {
				document
					.querySelector<HTMLInputElement>(".blank input, .slot input")
					?.focus();
			}
			return;
		}
		if (view.kind !== "evaluation") return;
		const command: Command | undefined = keys[event.key];
		if (command) {
			event.preventDefault();
			host.send(command);
		}
	}
</script>

<svelte:window onkeydown={key} />

<!-- Grain a skin may lay over hand-drawn strokes (the blackboard does: chalk breaks up at
     its edges). Streaked along the stroke rather than speckled across it. -->
<svg class="defs" aria-hidden="true">
	<filter
		id="chalk-grain"
		filterUnits="userSpaceOnUse"
		x="-10%"
		y="-10%"
		width="120%"
		height="120%"
	>
		<feTurbulence
			type="fractalNoise"
			baseFrequency="0.12 0.9"
			numOctaves="2"
			seed="7"
			result="grain"
		/>
		<feColorMatrix
			in="grain"
			type="matrix"
			values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 -1.1 1.4"
			result="mask"
		/>
		<feComposite in="SourceGraphic" in2="mask" operator="in" />
	</filter>
</svg>

<div class="board" data-skin={worn.skin} {@attach wear(worn.properties)}>
	{#if view.kind === "entry"}
		<EntryBoard {view} {host} onsettings={() => (panel = "settings")} />
	{:else if view.kind === "evaluation"}
		<EvaluationBoard
			{view}
			{host}
			onmenu={() => (panel = "questions")}
			onsettings={() => (panel = "settings")}
		/>
	{:else}
		<ProofBoard
			{view}
			{host}
			onmenu={() => (panel = "questions")}
			onsettings={() => (panel = "settings")}
		/>
	{/if}
	{#if panel === "questions" && view.kind !== "entry"}
		<Sheet catalog={view.catalog} {host} onclose={() => (panel = null)} />
	{:else if panel === "settings"}
		<Settings {appearance} {host} onclose={() => (panel = null)} />
	{/if}
</div>

<style>
	.defs {
		position: absolute;
		width: 0;
		height: 0;
	}
</style>
