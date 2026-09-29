import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig, type UserConfig } from "vite";
import { viteSingleFile } from "vite-plugin-singlefile";

// `gallery` builds the static design review: every screen from fixtures, in one HTML file
// that opens from disk. The default build is the page the desktop shell loads.
export default defineConfig(({ mode }): UserConfig =>
	mode === "gallery"
		? {
				plugins: [svelte(), viteSingleFile()],
				build: { outDir: "dist-gallery", rollupOptions: { input: "gallery.html" } },
			}
		: { plugins: [svelte()], build: { outDir: "dist" } },
);
