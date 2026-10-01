"""Experimental headless ConPTY driver using pywinpty 3.0.2 (never WinPTY).

API: https://github.com/andfoy/pywinpty/tree/v3.0.2/winpty
"""

from __future__ import annotations

from importlib import import_module
import os
import select
import socket
import time
from typing import Callable, Protocol, cast

from terminal_protocol import QUIET_SECONDS, STEP_TIMEOUT, Capture, Run, main, visible


class Process(Protocol):
	fileobj: socket.socket

	@property
	def exitstatus(self) -> int | None: ...
	def read(self, size: int) -> str: ...
	def write(self, text: str) -> int: ...
	def isalive(self) -> bool: ...
	def close(self, force: bool = False) -> None: ...


class ProcessFactory(Protocol):
	def spawn(
		self, argv: list[str], *, dimensions: tuple[int, int], env: dict[str, str]
	) -> Process: ...


class Winpty(Protocol):
	PtyProcess: ProcessFactory


def drive(binary: str, run: Run, columns: int, rows: int) -> Capture:
	winpty: Winpty = cast(Winpty, import_module("winpty"))
	child: Process = winpty.PtyProcess.spawn(
		[binary] + run["args"],
		dimensions=(rows, columns),
		env={
			**os.environ,
			"PYWINPTY_BACKEND": "0",
		},  # Backend.ConPTY in the pinned version.
	)
	out: str = ""
	eof: bool = False

	def pump(stop: Callable[[float], bool]) -> None:
		nonlocal out, eof
		deadline: float = time.monotonic() + STEP_TIMEOUT
		quiet_since: float = time.monotonic()
		while not eof and time.monotonic() < deadline:
			if stop(quiet_since):
				return
			if not select.select([child.fileobj], [], [], QUIET_SECONDS)[0]:
				continue
			try:
				chunk: str = child.read(65536)
			except EOFError:
				eof = True
				return
			previous: int = out.count("\x1b[6n")
			out += chunk
			quiet_since = time.monotonic()
			for _ in range(out.count("\x1b[6n") - previous):
				child.write("\x1b[1;1R")

	try:
		# ConPTY translates console writes. A rendered input hint shows the event loop is ready.
		pump(lambda _: "Enter" in out or eof)
		if "Enter" not in out:
			raise RuntimeError(f"ConPTY did not render the input hint: {out}")
		pump(lambda quiet: time.monotonic() - quiet >= QUIET_SECONDS)
		for keys in run["keys"]:
			child.write(keys)
			pump(lambda quiet: time.monotonic() - quiet >= QUIET_SECONDS)
		pump(
			lambda quiet: not child.isalive()
			and time.monotonic() - quiet >= QUIET_SECONDS
		)
		if child.isalive():
			raise TimeoutError(f"ConPTY child did not exit: {out}")
		exit_code: int | None = child.exitstatus
		if exit_code is None:
			raise RuntimeError("ConPTY supplied no exit status")
	finally:
		child.close(force=True)
	return {
		"raw": out,
		"visible": visible(out),
		"exit_code": exit_code,
		"terminal_before": None,
		"terminal_after": None,
	}


if __name__ == "__main__":
	main(drive)
