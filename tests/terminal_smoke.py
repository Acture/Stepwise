"""Drive the actual binary through a Unix pty; Rust asserts the captured observations."""

from __future__ import annotations

import errno
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time
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
# Bracketed paste on; crossterm enables raw mode just before writing it.
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


def controlling_terminal() -> None:
	fcntl.ioctl(0, termios.TIOCSCTTY, 0)


def settings(slave: int) -> str:
	return subprocess.run(
		["stty", "-g"], stdin=slave, capture_output=True, check=True, text=True
	).stdout.strip()


def supervise() -> None:
	# Keep the session leader alive until after stty: macOS hangs up a pty when its leader
	# exits, even if the parent holds a slave fd. This process never changes terminal modes.
	before: str = settings(0)
	code: int = subprocess.call(sys.argv[3:])
	after: str = settings(0)
	Path(sys.argv[2]).write_text(
		json.dumps({"before": before, "after": after}), encoding="utf-8"
	)
	sys.exit(code)


def drive(binary: str, run: Run, columns: int, rows: int) -> Capture:
	master: int
	slave: int
	master, slave = pty.openpty()
	fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))
	directory: tempfile.TemporaryDirectory[str] = tempfile.TemporaryDirectory()
	report: Path = Path(directory.name) / "modes.json"
	child: subprocess.Popen[bytes] = subprocess.Popen(
		[sys.executable, __file__, "--child", str(report), binary] + run["args"],
		stdin=slave,
		stdout=slave,
		stderr=slave,
		start_new_session=True,
		preexec_fn=controlling_terminal,
		env={**os.environ, "TERM": "xterm-256color"},
	)
	out: bytearray = bytearray()
	reported: int = 0

	def pump(stop: Callable[[float], bool]) -> None:
		nonlocal reported
		deadline: float = time.monotonic() + STEP_TIMEOUT
		quiet_since: float = time.monotonic()
		while time.monotonic() < deadline:
			if stop(quiet_since):
				return
			ready: list[int] = select.select([master], [], [], QUIET_SECONDS)[0]
			if not ready:
				continue
			try:
				chunk: bytes = os.read(master, 65536)
			except OSError as error:
				if error.errno == errno.EIO:
					return  # A closed Unix pty reports EIO rather than EOF.
				raise
			if not chunk:
				return
			previous: int = out.count(b"\x1b[6n")
			out.extend(chunk)
			quiet_since = time.monotonic()
			# A cursor query can straddle two reads.
			for _ in range(out.count(b"\x1b[6n") - previous):
				reported = min(reported + 1, rows)
				os.write(master, b"\x1b[%d;1R" % reported)

	try:
		pump(lambda _: RAW_MODE in out or child.poll() is not None)
		if RAW_MODE not in out:
			raise RuntimeError(
				f"{run['args']}: program never took over the terminal: {out.decode('utf-8', 'replace')}"
			)
		pump(lambda quiet: time.monotonic() - quiet >= QUIET_SECONDS)
		for keys in run["keys"]:
			os.write(master, keys.encode())
			pump(lambda quiet: time.monotonic() - quiet >= QUIET_SECONDS)
		# The parent's slave stays open for stty, so poll instead of waiting for EOF.
		pump(
			lambda quiet: child.poll() is not None
			and time.monotonic() - quiet >= QUIET_SECONDS
		)
		exit_code: int = child.wait(timeout=STEP_TIMEOUT)
		modes: dict[str, str] = cast(
			"dict[str, str]", json.loads(report.read_text(encoding="utf-8"))
		)
	finally:
		if child.poll() is None:
			child.kill()
			child.wait()
		os.close(master)
		os.close(slave)
		directory.cleanup()
	text: str = out.decode("utf-8", "replace")
	return {
		"raw": text,
		"visible": visible(text),
		"exit_code": exit_code,
		"terminal_before": modes["before"],
		"terminal_after": modes["after"],
	}


def main() -> None:
	# Rust sends UTF-8 bytes; read them as such whatever the locale says.
	request: Request = cast(Request, json.load(sys.stdin.buffer))
	captures: list[Capture] = [
		drive(request["binary"], run, request["columns"], request["rows"])
		for run in request["runs"]
	]
	json.dump({"runs": captures}, sys.stdout)


if __name__ == "__main__":
	if len(sys.argv) > 1 and sys.argv[1] == "--child":
		supervise()
	else:
		main()
