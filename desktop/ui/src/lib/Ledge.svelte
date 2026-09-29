<script lang="ts" module>
	export interface Tool {
		label: string;
		onclick: () => void;
		disabled?: boolean;
	}
</script>

<script lang="ts">
	let { tools, aside = [] }: { tools: Tool[]; aside?: Tool[] } = $props();
</script>

<!-- The tray under the board: the actions around a step, never the step itself. -->
<footer>
	{#each [tools, aside] as group, index (index)}
		<div class="group">
			{#each group as tool (tool.label)}
				<!-- A press keeps the caret where it was: the tray acts around the step being written. -->
				<button
					onmousedown={(event) => event.preventDefault()}
					onclick={tool.onclick}
					disabled={tool.disabled}>{tool.label}</button
				>
			{/each}
		</div>
	{/each}
</footer>

<style>
	footer {
		flex: none;
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 8px;
		height: 46px;
		padding: 3px 20px 0;
		background: var(--tray);
		box-shadow: var(--tray-shadow, none);
	}

	.group {
		display: flex;
		gap: 22px;
	}

	button {
		height: 32px;
		font-size: 15px;
		letter-spacing: 0.04em;
		color: var(--ink-faded);
		transition: color 120ms ease-out;
	}

	button:not(:disabled):hover {
		color: var(--ink);
	}
</style>
