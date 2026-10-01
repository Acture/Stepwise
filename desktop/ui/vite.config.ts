import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig, type UserConfig } from "vite";
import { viteSingleFile } from "vite-plugin-singlefile";
import { pageNotices } from "./licenses";

// `gallery` builds the static design review: every screen from fixtures, in one HTML file
// that opens from disk. The default build is the page the desktop shell loads.
export default defineConfig(({ mode }): UserConfig =>
	mode === "gallery"
		? {
				plugins: [svelte(), viteSingleFile()],
				build: {
					outDir: "dist-gallery",
					rollupOptions: { input: "gallery.html" },
				},
			}
		: {
				plugins: [svelte(), pageNotices()],
				build: {
					outDir: "dist",
					// Vite reads the licence texts of the packages present in the built chunks,
					// including Svelte's runtime even though Svelte is a devDependency.
					license: { fileName: "JS-THIRD-PARTY-NOTICES.json" },
				},
			},
);
