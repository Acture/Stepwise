"""The capture protocol shared by the Unix pty and Windows ConPTY drivers."""

from __future__ import annotations

import json
import re
import sys
from typing import Callable, TypedDict, cast


class Run(TypedDict):
	args: list[str]
	keys: list[str]


class Capture(TypedDict):
	raw: str
	visible: str
	exit_code: int
	terminal_before: str
	terminal_after: str


class Request(TypedDict):
	binary: str
	columns: int
	rows: int
	runs: list[Run]


QUIET_SECONDS = 0.15
RAW_MODE = b"\x1b[?2004h"
STEP_TIMEOUT = 5.0
ESCAPES = (
	re.compile(r"\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)"),
	re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]"),
	re.compile(r"\x1b[()][B0]"),
)


def visible(text: str) -> str:
	for pattern in ESCAPES:
		text = pattern.sub("", text)
	return text


def main(drive: Callable[[str, Run, int, int], Capture]) -> None:
	# Rust sends UTF-8 bytes. Windows pipes otherwise use the locale's ANSI code page.
	request: Request = cast(Request, json.load(sys.stdin.buffer))
	captures: list[Capture] = [
		drive(request["binary"], run, request["columns"], request["rows"])
		for run in request["runs"]
	]
	json.dump({"runs": captures}, sys.stdout)
