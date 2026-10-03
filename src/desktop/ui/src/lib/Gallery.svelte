<script lang="ts">
	import { skins } from "../skins";
	import App from "./App.svelte";
	import { appearance, screens, themes } from "./fixtures";
	import type { Look } from "./protocol/Look";
	import type { Command, Host } from "./view";

	/** One way to dress every frame: a skin or theme, and for the native skin the scheme the
	 * system would be in. */
	interface Outfit {
		key: string;
		name: string;
		skin: string;
		scheme: "light" | "dark" | null;
	}

	const outfits: Outfit[] = [
		...skins.flatMap((skin): Outfit[] =>
			skin.id === "native"
				? [
						{
							key: "native-light",
							name: `${skin.name}（浅色）`,
							skin: skin.id,
							scheme: "light",
						},
						{
							key: "native-dark",
							name: `${skin.name}（深色）`,
							skin: skin.id,
							scheme: "dark",
						},
					]
				: [{ key: skin.id, name: skin.name, skin: skin.id, scheme: null }],
		),
		...themes.map((theme): Outfit => ({
			key: `theme-${theme.name.toLowerCase().replace(/[^a-z]+/g, "-")}`,
			name: theme.name,
			skin: theme.id,
			scheme: null,
		})),
	];

	const first: Outfit =
		outfits.find((outfit) => location.hash === `#${outfit.key}`) ?? outfits[0]!;

	let look: Look = $state({ ...appearance.look, skin: first.skin });
	let scheme: "light" | "dark" | null = $state(first.scheme);

	// The review page talks to no Rust side: commands are logged and the frames stay still,
	// except that a look chosen in the settings is worn by every frame.
	const host: Host = {
		send: (command: Command) => console.info("command", command),
		openSet: () => console.info("open a set"),
		setLook: (next: Look) => (look = next),
		importTheme: () => console.info("import a theme"),
	};

	function dress(outfit: Outfit): void {
		look = { ...look, skin: outfit.skin };
		scheme = outfit.scheme;
	}
</script>

<nav class="outfits">
	{#each outfits as outfit (outfit.key)}
		<a
			href="#{outfit.key}"
			class:chosen={look.skin === outfit.skin && scheme === outfit.scheme}
			onclick={() => dress(outfit)}>{outfit.name}</a
		>
	{/each}
</nav>

<div class="gallery">
	{#each screens as screen (screen.name)}
		<figure>
			<div
				class="window"
				data-scheme={scheme}
				style:width="{screen.width}px"
				style:height="{screen.height}px"
			>
				<App
					view={screen.view}
					{host}
					appearance={{
						...appearance,
						look: { ...look, ...screen.picks },
						message: screen.said ?? null,
					}}
					opened={screen.opened ?? null}
				/>
				<span class="lights" aria-hidden="true"><i></i><i></i><i></i></span>
			</div>
			<figcaption>{screen.name} · {screen.width}×{screen.height}</figcaption>
		</figure>
	{/each}
</div>

<style>
	:global(body) {
		background: #d9d6cf;
		height: auto;
		font-family: system-ui, sans-serif;
	}

	.outfits {
		display: flex;
		flex-wrap: wrap;
		gap: 8px 18px;
		padding: 24px 40px 0;
		font-size: 15px;
	}

	.outfits a {
		color: #3d3a35;
		text-decoration: none;
	}

	.outfits a.chosen {
		font-weight: 600;
		text-decoration: underline;
	}

	.gallery {
		display: flex;
		flex-wrap: wrap;
		gap: 40px;
		padding: 24px 40px 40px;
		align-items: flex-start;
	}

	figure {
		margin: 0;
	}

	.window {
		position: relative;
		border-radius: 11px;
		overflow: hidden;
		box-shadow:
			0 0 0 0.5px rgba(0, 0, 0, 0.45),
			0 22px 50px rgba(0, 0, 0, 0.35);
	}

	/* The system's appearance, for the skin that follows it: the page cannot switch the
	   system, so each frame says which one it stands in. */
	.window[data-scheme="light"] :global([data-skin="native"]) {
		color-scheme: light;
	}

	.window[data-scheme="dark"] :global([data-skin="native"]) {
		color-scheme: dark;
	}

	/* Stand-ins for the macOS window buttons the real title bar draws over the board. */
	.lights {
		position: absolute;
		top: 16px;
		left: 14px;
		display: flex;
		gap: 8px;
	}

	.lights i {
		width: 12px;
		height: 12px;
		border-radius: 50%;
		background: #ff5f57;
	}

	.lights i:nth-child(2) {
		background: #febc2e;
	}

	.lights i:nth-child(3) {
		background: #28c840;
	}

	figcaption {
		margin-top: 12px;
		font-size: 13px;
		color: #3d3a35;
	}
</style>
