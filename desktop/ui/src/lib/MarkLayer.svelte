<script lang="ts">
	import { wornDress, type Dress } from "../skins";
	import { strokes, type Mark, type Stroke } from "./marks";

	/** How long one stroke takes to draw itself in, in milliseconds. */
	const DRAW: number = 380;

	let { host, marks }: { host: HTMLElement | undefined; marks: Mark[] } =
		$props();

	const worn: () => Dress = wornDress();

	/** When each mark first appeared. A mark draws itself in once, for as long as its own
	 * stroke takes, however often layout makes the layer redraw meanwhile. */
	let born: Map<string, number> = new Map();
	let lines: Stroke[] = $state([]);
	let tick: number = $state(0);

	$effect(() => {
		void tick;
		if (!host) return;
		const now: number = performance.now();
		const present: Set<string> = new Set(marks.map((mark) => mark.id));
		for (const id of born.keys()) if (!present.has(id)) born.delete(id);
		for (const mark of marks) if (!born.has(mark.id)) born.set(mark.id, now);
		const drawn: Set<string> = new Set(
			marks
				.filter(
					(mark) => now - born.get(mark.id)! > (mark.delay ?? 0) + DRAW + 200,
				)
				.map((mark) => mark.id),
		);
		lines = strokes(host, marks, drawn);
	});

	// Text reflows when the window resizes or a font arrives; the marks follow it.
	$effect(() => {
		if (!host) return;
		const observer: ResizeObserver = new ResizeObserver(() => tick++);
		const arrived = (): void => {
			tick++;
		};
		observer.observe(host);
		document.fonts.addEventListener("loadingdone", arrived);
		void document.fonts.ready.then(arrived);
		return () => {
			observer.disconnect();
			document.fonts.removeEventListener("loadingdone", arrived);
		};
	});

	// A new skin or face moves the text without resizing the host, and a skin sets how rough
	// a stroke is: the marks are drawn again a frame later, once the board wears it.
	$effect(() => {
		void worn();
		const frame: number = requestAnimationFrame(() => tick++);
		return () => cancelAnimationFrame(frame);
	});
</script>

<svg class="marks" aria-hidden="true">
	{#each lines as line (line.key)}
		<path
			d={line.d}
			style:stroke={line.color}
			stroke-width={line.width}
			fill="none"
			stroke-linecap="round"
			pathLength="1"
			class:fresh={line.fresh}
			style:animation-delay="{line.delay}ms"
		/>
	{/each}
</svg>

<style>
	.marks {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		overflow: visible;
		pointer-events: none;
	}

	path {
		filter: var(--stroke-filter, none);
	}

	.fresh {
		stroke-dasharray: 1;
		stroke-dashoffset: 1;
		animation: draw 380ms ease-out forwards; /* DRAW */
	}

	@keyframes draw {
		to {
			stroke-dashoffset: 0;
		}
	}
</style>
