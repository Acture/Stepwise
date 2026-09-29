<script lang="ts">
	import { strokes, type Mark, type Stroke } from "./marks";

	let { host, marks }: { host: HTMLElement | undefined; marks: Mark[] } =
		$props();

	let drawn: Set<string> = new Set();
	let lines: Stroke[] = $state([]);
	let tick: number = $state(0);

	$effect(() => {
		void tick;
		if (!host) return;
		lines = strokes(host, marks, drawn);
		drawn = new Set(marks.map((mark) => mark.id));
	});

	// Text reflows when the window resizes or a font arrives; the marks follow it.
	$effect(() => {
		if (!host) return;
		const observer: ResizeObserver = new ResizeObserver(() => tick++);
		observer.observe(host);
		void document.fonts.ready.then(() => tick++);
		return () => observer.disconnect();
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
		animation: draw 380ms ease-out forwards;
	}

	@keyframes draw {
		to {
			stroke-dashoffset: 0;
		}
	}
</style>
