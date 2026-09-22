"""Pseudo-terminal driver for the terminal test. Supplies a pty, asserts nothing.

Reads one JSON request on stdin and writes one JSON response on stdout, so the
expectations stay in tests/terminal.rs with the rest of the suite.
"""

import fcntl
import json
import os
import pty
import re
import select
import struct
import sys
import termios
import time
from typing import Callable, TypedDict


class Run(TypedDict):
	args: list[str]
	keys: list[str]


class Capture(TypedDict):
	raw: str
	visible: str


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
	"""Drop escape sequences, keeping the characters a reader would see."""
	for pattern in ESCAPES:
		text = pattern.sub("", text)
	return text


def drive(binary: str, run: Run, columns: int, rows: int) -> Capture:
	pid, fd = pty.fork()
	if pid == 0:
		os.environ["TERM"] = "xterm-256color"
		os.execv(binary, ["stepwise"] + run["args"])
	fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))
	out = bytearray()
	reported = [0]

	def pump(stop: Callable[[float], bool]) -> None:
		"""Read until `stop` says so, or to end of file.

		Every read answers the cursor-position report ratatui blocks on, so a program
		waiting for one is never left hanging.
		"""
		deadline = time.time() + STEP_TIMEOUT
		quiet_since = time.time()
		while time.time() < deadline:
			if stop(quiet_since):
				return
			ready, _, _ = select.select([fd], [], [], QUIET_SECONDS)
			if not ready:
				continue
			try:
				chunk = os.read(fd, 65536)
			except OSError:
				return
			if not chunk:
				return
			out.extend(chunk)
			quiet_since = time.time()
			for _ in range(chunk.count(b"\x1b[6n")):
				reported[0] = min(reported[0] + 1, rows)
				try:
					os.write(fd, b"\x1b[%d;1R" % reported[0])
				except OSError:
					return

	# Keys sent before raw mode is on are echoed and line-buffered instead of delivered,
	# so wait for the program to switch the terminal over. RAW_MODE is written just after
	# crossterm enables it, and a slow machine only makes this wait longer, never wrong.
	pump(lambda _: RAW_MODE in out)
	if RAW_MODE not in out:
		raise RuntimeError(f"{run['args']}: program never took over the terminal")
	pump(lambda quiet_since: time.time() - quiet_since >= QUIET_SECONDS)
	for keys in run["keys"]:
		try:
			os.write(fd, keys.encode())
		except OSError:
			break
		pump(lambda quiet_since: time.time() - quiet_since >= QUIET_SECONDS)
	# Read to end of file, so the restore sequences written while exiting are captured.
	pump(lambda _: False)
	try:
		os.close(fd)
	except OSError:
		pass
	os.waitpid(pid, 0)
	text = out.decode("utf-8", "replace")
	return {"raw": text, "visible": visible(text)}


def main() -> None:
	request = json.load(sys.stdin)
	captures = [
		drive(request["binary"], run, request["columns"], request["rows"])
		for run in request["runs"]
	]
	json.dump({"runs": captures}, sys.stdout)


if __name__ == "__main__":
	main()
