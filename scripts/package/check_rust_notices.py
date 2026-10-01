"""cargo-about can silently fall back to SPDX boilerplate even with --fail."""

import json
from pathlib import Path
import re
import sys
from typing import TypedDict, cast


class Crate(TypedDict):
	name: str
	version: str


class UsedBy(TypedDict):
	crate: Crate


class License(TypedDict):
	id: str
	source_path: str | None
	used_by: list[UsedBy]


class Report(TypedDict):
	licenses: list[License]


def check(metadata: Path, notice: Path) -> None:
	report: Report = cast(Report, json.loads(metadata.read_text(encoding="utf-8")))
	requires_holder: set[str] = {
		"MIT",
		"BSD-2-Clause",
		"BSD-3-Clause",
		"ISC",
		"Zlib",
		"0BSD",
	}
	for license in report["licenses"]:
		if license["source_path"] is None and license["id"] in requires_holder:
			raise ValueError(
				f"No licence file with copyright holder: {license['used_by']}"
			)
	text: str = notice.read_text(encoding="utf-8")
	if not text.strip() or re.search(
		r"<(year|owner|copyright holders)>", text, re.IGNORECASE
	):
		raise ValueError(f"Empty licence or placeholder copyright in {notice}")
	print(f"Verified copyright sources and rendered notice: {notice}")


if __name__ == "__main__":
	check(Path(sys.argv[1]), Path(sys.argv[2]))
