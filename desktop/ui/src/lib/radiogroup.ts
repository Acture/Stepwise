// The keyboard for a group of radios, as WAI-ARIA describes it: the group is one Tab stop,
// on the checked radio or else the first, and the arrow keys move between radios. Moving
// only moves; Space or Enter on a radio picks it, so walking past three hundred fonts does
// not wear each of them on the way.

const STEPS: Record<string, number> = {
	ArrowDown: 1,
	ArrowRight: 1,
	ArrowUp: -1,
	ArrowLeft: -1,
};

export function radiogroup(group: HTMLElement): () => void {
	const radios = (): HTMLElement[] =>
		Array.from(
			group.querySelectorAll<HTMLElement>('[role="radio"]:not([disabled])'),
		);

	const stop = (at: HTMLElement | undefined): void => {
		for (const radio of radios()) radio.tabIndex = radio === at ? 0 : -1;
	};

	const settle = (): void => {
		const all: HTMLElement[] = radios();
		stop(
			all.find((radio) => radio.getAttribute("aria-checked") === "true") ??
				all[0],
		);
	};

	const key = (event: KeyboardEvent): void => {
		const all: HTMLElement[] = radios();
		const at: number = all.indexOf(document.activeElement as HTMLElement);
		if (at < 0 || all.length === 0) return;
		const next: HTMLElement | undefined =
			event.key === "Home"
				? all[0]
				: event.key === "End"
					? all.at(-1)
					: STEPS[event.key] === undefined
						? undefined
						: all[(at + STEPS[event.key]! + all.length) % all.length];
		if (!next) return;
		event.preventDefault();
		next.focus();
		stop(next);
	};

	settle();
	// A pick, a search or a new list of themes changes which radios there are and which is
	// checked; the Tab stop follows.
	const observer: MutationObserver = new MutationObserver(settle);
	observer.observe(group, {
		subtree: true,
		childList: true,
		attributes: true,
		attributeFilter: ["aria-checked", "disabled"],
	});
	group.addEventListener("keydown", key);
	return () => {
		observer.disconnect();
		group.removeEventListener("keydown", key);
	};
}
