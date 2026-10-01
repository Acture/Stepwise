import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import type { Plugin, ResolvedConfig } from "vite";

interface Package {
	name: string;
	version: string;
	license: string;
	module?: string;
	dependencies?: Record<string, string>;
}

interface Notice {
	name: string;
	version: string;
	identifier: string;
	text: string;
}

const accepted: ReadonlySet<string> = new Set([
	"MIT",
	"ISC",
	"Apache-2.0",
	"Apache-2.0 OR MIT",
	"MIT OR Apache-2.0",
]);

// Vite sees the modules it bundled, including Svelte's runtime. Published bundles such as
// Rough.js already contain their dependencies, and CSS/fonts need notices too, so include
// the runtime dependency closure as well. All text comes from the installed, locked packages.
export function pageNotices(): Plugin {
	let root: string;
	let output: string;
	return {
		name: "stepwise-page-notices",
		apply: "build",
		configResolved(config: ResolvedConfig): void {
			root = config.root;
			output = join(root, config.build.outDir, "JS-THIRD-PARTY-NOTICES.json");
		},
		closeBundle(): void {
			const notices: Notice[] = JSON.parse(readFileSync(output, "utf8"));
			const packages: Map<string, Notice> = new Map();
			const visited: Set<string> = new Set();
			function include(
				manifest: string,
				dependencies: boolean = false,
				version?: string,
			): void {
				const pkg: Package = JSON.parse(readFileSync(manifest, "utf8"));
				if (version !== undefined && pkg.version !== version) {
					throw new Error(
						`Bundled ${pkg.name}@${version} resolves to ${pkg.version}`,
					);
				}
				if (visited.has(manifest)) return;
				visited.add(manifest);
				const directory: string = dirname(manifest);
				if (!accepted.has(pkg.license)) {
					throw new Error(
						`Unreviewed page licence: ${pkg.name}: ${pkg.license}`,
					);
				}
				const text: string = readdirSync(directory)
					.filter((file: string): boolean =>
						/^(licen[cs]e|copying|notice)(\.|$)/i.test(file),
					)
					.sort()
					.map((file: string): string =>
						readFileSync(join(directory, file), "utf8").trim(),
					)
					.join("\n\n");
				if (!text || /<(year|owner|copyright holders)>/i.test(text)) {
					throw new Error(
						`Missing licence text or copyright holder: ${pkg.name}`,
					);
				}
				packages.set(`${pkg.name}@${pkg.version}`, {
					name: pkg.name,
					version: pkg.version,
					identifier: pkg.license,
					text,
				});
				if (dependencies || pkg.module?.startsWith("bundled/")) {
					for (const name of Object.keys(pkg.dependencies ?? {})) {
						include(resolvePackage(name, manifest), true);
					}
				}
			}
			const manifest: string = join(root, "package.json");
			const page: Package = JSON.parse(readFileSync(manifest, "utf8"));
			for (const notice of notices) {
				include(resolvePackage(notice.name, manifest), false, notice.version);
			}
			for (const name of Object.keys(page.dependencies ?? {})) {
				include(resolvePackage(name, manifest));
			}
			// The module-preload helper is a virtual Vite module, absent from Vite's own report.
			include(resolvePackage("vite", manifest), false);
			const result: Notice[] = [...packages.values()].sort(
				(a: Notice, b: Notice): number =>
					`${a.name}@${a.version}`.localeCompare(
						`${b.name}@${b.version}`,
						"en",
					),
			);
			writeFileSync(output, `${JSON.stringify(result, null, "\t")}\n`);
			console.info(`Page licences: ${result.length} packages -> ${output}`);
		},
	};
}

function resolvePackage(name: string, from: string): string {
	// Package exports may hide package.json. Search Node's resolution paths directly.
	const paths: string[] = createRequire(from).resolve.paths(name) ?? [];
	for (const directory of paths) {
		const candidate: string = join(directory, name, "package.json");
		if (existsSync(candidate)) return candidate;
	}
	throw new Error(`Cannot resolve ${name} from ${from}`);
}
