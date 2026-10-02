"""Experimental headless ConPTY driver using pywinpty 3.0.2 (never WinPTY).

API: https://github.com/andfoy/pywinpty/tree/v3.0.2/winpty
"""

from __future__ import annotations

import ctypes
from ctypes import wintypes
from importlib import import_module
import json
import os
from pathlib import Path
import select
import socket
import subprocess
import sys
import tempfile
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


def settings() -> str:
	kernel32: ctypes.CDLL = ctypes.WinDLL("kernel32", use_last_error=True)
	kernel32.GetStdHandle.argtypes = [wintypes.DWORD]
	kernel32.GetStdHandle.restype = wintypes.HANDLE
	kernel32.GetConsoleMode.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]
	kernel32.GetConsoleMode.restype = wintypes.BOOL
	handle: int | None = kernel32.GetStdHandle(wintypes.DWORD(-10))
	mode: wintypes.DWORD = wintypes.DWORD()
	if not kernel32.GetConsoleMode(handle, ctypes.byref(mode)):
		raise ctypes.WinError(ctypes.get_last_error())
	return str(mode.value)


def supervise() -> None:
	# Keep the same console alive to inspect its input mode after Stepwise exits.
	before: str = settings()
	code: int = subprocess.call(sys.argv[3:])
	after: str = settings()
	Path(sys.argv[2]).write_text(
		json.dumps({"before": before, "after": after}), encoding="utf-8"
	)
	sys.exit(code)


def drive(binary: str, run: Run, columns: int, rows: int) -> Capture:
	winpty: Winpty = cast(Winpty, import_module("winpty"))
	# spawn reads this from the driver, not the environment passed to its child. Its
	# `backend or ...` discards integer 0 too, so use the documented environment override.
	os.environ["PYWINPTY_BACKEND"] = "0"
	directory: tempfile.TemporaryDirectory[str] = tempfile.TemporaryDirectory()
	report: Path = Path(directory.name) / "modes.json"
	child: Process = winpty.PtyProcess.spawn(
		[sys.executable, __file__, "--child", str(report), binary] + run["args"],
		dimensions=(rows, columns),
		env=dict(os.environ),  # Backend.ConPTY in the pinned version.
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
		modes: dict[str, str] = cast(
			"dict[str, str]", json.loads(report.read_text(encoding="utf-8"))
		)
		print(
			f"ConPTY console input mode: {modes['before']} -> {modes['after']}",
			file=sys.stderr,
		)
	finally:
		child.close(force=True)
		directory.cleanup()
	return {
		"raw": out,
		"visible": visible(out),
		"exit_code": exit_code,
		"terminal_before": modes["before"],
		"terminal_after": modes["after"],
	}


if __name__ == "__main__":
	if len(sys.argv) > 1 and sys.argv[1] == "--child":
		supervise()
	else:
		print(
			f"ConPTY request uses UTF-8 bytes; stdin text encoding is {sys.stdin.encoding}",
			file=sys.stderr,
		)
		main(drive)
