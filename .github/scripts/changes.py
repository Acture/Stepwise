"""Which checks a pull request needs, from the files it touches: lint.yml and test.yml run a check
only when its part of the repository changed. Without a pull request (a packaging run calling the
tests) every check of the requested family runs."""

from __future__ import annotations

import argparse
from collections.abc import Iterable
import os
from pathlib import Path
import re
import subprocess

# Each check and the paths that can change its outcome, matched from the start of the path.
CHECKS: dict[str, tuple[str, ...]] = {
	# The stepwise crate and its tests, which also read the example set and two documents.
	"cli": (
		r"Cargo\.(toml|lock)$",
		r"src/(?!desktop/(src-tauri|ui)/)",
		r"tests/",
		r"questions/",
		r"docs/(reference|question-sets)\.md$",
	),
	# The shell, the page it embeds, and the library it links: all but the terminal front end.
	"desktop": (
		r"Cargo\.(toml|lock)$",
		r"src/(?!tui/|main\.rs$)",
		r"questions/builtin\.toml$",
	),
	"page": (r"src/desktop/ui/",),
	"rustfmt": (r".*\.rs$", r"rustfmt\.toml$"),
	"scripts": (r"\.github/scripts/",),
}
# A change to the checks themselves runs all of them.
EVERYTHING: tuple[str, ...] = (
	r"\.github/workflows/(lint|test)\.yml$",
	r"\.github/scripts/changes\.py$",
)
FAMILIES: dict[str, frozenset[str]] = {
	"": frozenset(CHECKS),
	"cli": frozenset({"cli", "rustfmt", "scripts"}),
	"desktop": frozenset({"desktop", "page", "rustfmt", "scripts"}),
}


def needed(paths: Iterable[str] | None, family: str = "") -> dict[str, bool]:
	"""The checks to run; every check of the family when there are no paths to go by."""
	allowed: frozenset[str] = FAMILIES[family]
	touched: list[str] | None = None if paths is None else list(paths)
	if touched is None or any(
		re.match(pattern, path) for path in touched for pattern in EVERYTHING
	):
		return {check: check in allowed for check in CHECKS}
	return {
		check: check in allowed
		and any(re.match(pattern, path) for path in touched for pattern in patterns)
		for check, patterns in CHECKS.items()
	}


def changed(base: str, head: str) -> list[str]:
	"""Every path the pull request adds, changes or removes; a moved file counts at both ends."""
	return subprocess.run(
		["git", "diff", "--name-only", "--no-renames", f"{base}...{head}"],
		check=True,
		capture_output=True,
		text=True,
		encoding="utf-8",
	).stdout.splitlines()


def main() -> None:
	parser: argparse.ArgumentParser = argparse.ArgumentParser(description=__doc__)
	parser.add_argument("--base", default="")
	parser.add_argument("--head", default="")
	parser.add_argument("--family", default="", choices=sorted(FAMILIES))
	args: argparse.Namespace = parser.parse_args()
	paths: list[str] | None = (
		changed(args.base, args.head) if args.base and args.head else None
	)
	if paths is not None:
		print("\n".join(paths))
	checks: dict[str, bool] = needed(paths, args.family)
	lines: str = "".join(
		f"{check}={str(run).lower()}\n" for check, run in checks.items()
	)
	print(lines, end="")
	with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
		output.write(lines)


if __name__ == "__main__":
	main()
