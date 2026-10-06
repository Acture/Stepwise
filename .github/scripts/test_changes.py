"""A pull request runs the checks for what it touches, and every check when there is nothing to
go by or when it changes the checks themselves."""

import unittest

from changes import CHECKS, needed


def run(*paths: str, family: str = "") -> set[str]:
	return {check for check, wanted in needed(paths, family).items() if wanted}


class Changes(unittest.TestCase):
	def test_each_part_runs_only_its_own_checks(self) -> None:
		self.assertEqual(run("README.md", "docs/usage.md", "notes"), set())
		self.assertEqual(run("src/tui/render.rs"), {"cli", "rustfmt"})
		self.assertEqual(run("src/main.rs"), {"cli", "rustfmt"})
		self.assertEqual(run("src/core/session.rs"), {"cli", "desktop", "rustfmt"})
		self.assertEqual(run("src/desktop/desk.rs"), {"cli", "desktop", "rustfmt"})
		self.assertEqual(
			run("src/desktop/src-tauri/src/main.rs"), {"desktop", "rustfmt"}
		)
		self.assertEqual(
			run("src/desktop/src-tauri/tauri.macos.conf.json"), {"desktop"}
		)
		self.assertEqual(run("src/desktop/ui/src/App.svelte"), {"desktop", "page"})
		self.assertEqual(run("questions/example.toml"), {"cli"})
		self.assertEqual(run("questions/builtin.toml"), {"cli", "desktop"})
		self.assertEqual(run("docs/reference.md"), {"cli"})
		self.assertEqual(run("tests/terminal_smoke.py"), {"cli"})
		self.assertEqual(run("Cargo.lock"), {"cli", "desktop"})
		self.assertEqual(run("rustfmt.toml"), {"rustfmt"})
		self.assertEqual(run(".github/scripts/verify.py"), {"scripts"})
		self.assertEqual(run(".github/workflows/package-cli.yml"), set())

	def test_changing_the_checks_or_having_no_pull_request_runs_everything(
		self,
	) -> None:
		for paths in ((".github/workflows/test.yml",), (".github/scripts/changes.py",)):
			with self.subTest(paths=paths):
				self.assertEqual(run(*paths), set(CHECKS))
		self.assertEqual(
			{check for check, wanted in needed(None).items() if wanted}, set(CHECKS)
		)

	def test_a_packaging_run_tests_its_own_family_only(self) -> None:
		self.assertEqual(
			{check for check, wanted in needed(None, "cli").items() if wanted},
			{"cli", "rustfmt", "scripts"},
		)
		self.assertEqual(
			{check for check, wanted in needed(None, "desktop").items() if wanted},
			{"desktop", "page", "rustfmt", "scripts"},
		)
		self.assertEqual(run("src/tui/render.rs", family="desktop"), {"rustfmt"})


if __name__ == "__main__":
	unittest.main()
