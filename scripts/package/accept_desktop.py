"""Identify one downloaded desktop installer, install it or take the app out of its dmg, and
launch an installed app until its page has loaded. Nothing here builds the app: every check
reads the shipped bytes."""

from __future__ import annotations

import argparse
from collections.abc import Callable, Sequence
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
import hashlib
import json
import logging
import os
from pathlib import Path
import queue
import re
import signal
import subprocess
import sys
import tempfile
import threading
import time
from typing import IO

LOG: logging.Logger = logging.getLogger(__name__)

# The shell's page-load line (desktop/src-tauri/src/main.rs, `on_page_load`), and the prefix of
# the line the page sends when something stopped it drawing (its `report` command).
LOADED: re.Pattern[str] = re.compile(r"^Stepwise：页面 Finished \S+")
FAULT: str = "Stepwise 页面："
# Past the page's own watchdog (index.html), which reports a module that never started after 4 s.
SETTLE: float = 6.0


def digest(path: Path) -> str:
	with path.open("rb") as stream:
		return hashlib.file_digest(stream, "sha256").hexdigest()


def identify(artifacts: Path, name: str) -> dict[str, str]:
	"""The named installer, held to the hash its packaging job recorded. No pattern: a file
	that is not there is an error, never a reason to pick another."""
	installer: Path = artifacts / name
	expected: str = (
		installer.with_name(name + ".sha256").read_text(encoding="utf-8").strip()
	)
	actual: str = digest(installer)
	if not re.fullmatch(r"[0-9a-f]{64}", expected) or actual != expected:
		raise ValueError(
			f"{name}: downloaded bytes differ from the packaging job's SHA256"
		)
	return {"installer": name, "path": str(installer.resolve()), "sha256": actual}


def gatekeeper_accepts(signature: str) -> bool:
	"""What Gatekeeper must say of an app, from `codesign -dv`'s description of its signature:
	reject an ad-hoc signature, accept a Developer ID one. Anything else is neither."""
	if re.search(r"^Signature=adhoc$", signature, re.MULTILINE):
		return False
	if re.search(r"^Authority=Developer ID Application: ", signature, re.MULTILINE):
		return True
	raise ValueError(f"neither an ad-hoc nor a Developer ID signature:\n{signature}")


def run(*args: str) -> subprocess.CompletedProcess[str]:
	LOG.info("%s", " ".join(args))
	return subprocess.run(
		args, check=True, capture_output=True, text=True, encoding="utf-8"
	)


def show(*args: str) -> None:
	"""Runs a command whose output belongs in the job's log as it happens."""
	LOG.info("%s", " ".join(args))
	subprocess.run(args, check=True)


def install(installer: Path, evidence: Path) -> dict[str, str]:
	"""Installs as a student would, without questions, and finds the program where this kind
	of installer puts it — and only there: per user for NSIS, per machine for MSI."""
	name: str = installer.name
	elsewhere: Path | None = None
	if name.endswith("-setup.exe"):
		show(str(installer), "/S")
		app: Path = Path(os.environ["LOCALAPPDATA"]) / "Stepwise/stepwise-desktop.exe"
		elsewhere = Path(os.environ["ProgramFiles"]) / "Stepwise"
	elif name.endswith(".msi"):
		show(
			"msiexec",
			"/i",
			str(installer),
			"/qn",
			"/norestart",
			"/l*v",
			str(evidence / "msi.log"),
		)
		app = Path(os.environ["ProgramFiles"]) / "Stepwise/stepwise-desktop.exe"
		elsewhere = Path(os.environ["LOCALAPPDATA"]) / "Stepwise"
	elif name.endswith(".deb"):
		# The package's own dependencies bring WebKitGTK.
		show("sudo", "apt-get", "install", "-y", str(installer))
		app = Path("/usr/bin/stepwise-desktop")
	elif name.endswith(".rpm"):
		show("dnf", "install", "-y", str(installer))
		app = Path("/usr/bin/stepwise-desktop")
	elif name.endswith(".AppImage"):
		installer.chmod(installer.stat().st_mode | 0o111)
		app = installer
	else:
		raise ValueError(f"{name}: not an installer this job knows")
	# The installer has exited; give a file it wrote last a moment to be visible.
	deadline: float = time.monotonic() + 60
	while not app.is_file():
		if time.monotonic() > deadline:
			raise FileNotFoundError(f"{name} installed no {app}")
		time.sleep(1)
	if elsewhere is not None and elsewhere.exists():
		raise ValueError(f"{name} also installed {elsewhere}")
	return {"app": str(app), "appSha256": digest(app)}


def unpack_dmg(dmg: Path, destination: Path, target: str) -> dict[str, str]:
	"""Verifies the dmg, copies its app out and checks the copy's architecture, signature and
	Gatekeeper's verdict on it; returns where the copy's executable is."""
	run("hdiutil", "verify", str(dmg))
	mount: Path = Path(tempfile.mkdtemp(prefix="stepwise-dmg-"))
	run(
		"hdiutil",
		"attach",
		"-readonly",
		"-nobrowse",
		"-mountpoint",
		str(mount),
		str(dmg),
	)
	try:
		run("ditto", str(mount / "Stepwise.app"), str(destination / "Stepwise.app"))
	finally:
		run("hdiutil", "detach", str(mount))
	app: Path = destination / "Stepwise.app"
	binary: Path = app / "Contents/MacOS/stepwise-desktop"
	wanted: str = "arm64" if target.startswith("aarch64-") else "x86_64"
	architecture: str = run("lipo", "-archs", str(binary)).stdout.strip()
	if architecture != wanted:
		raise ValueError(f"{dmg.name}: expected {wanted} alone, got {architecture}")
	run("codesign", "--verify", "--deep", "--strict", "--verbose=2", str(app))
	# codesign describes a signature on stderr.
	signature: str = run("codesign", "-dv", "--verbose=4", str(app)).stderr
	accepts: bool = gatekeeper_accepts(signature)
	# Whether assessments are enabled at all: a Mac that disabled them accepts anything.
	assessments: str = run("spctl", "--status").stdout.strip()
	assessment: subprocess.CompletedProcess[str] = subprocess.run(
		["spctl", "--assess", "--type", "execute", "-vv", str(app)],
		capture_output=True,
		text=True,
		encoding="utf-8",
	)
	verdict: str = (assessment.stdout + assessment.stderr).strip()
	LOG.info("Gatekeeper (exit %d): %s", assessment.returncode, verdict)
	# spctl exits 0 when it accepts and 3 when it rejects; anything else is no verdict at all.
	if assessment.returncode not in (0, 3) or (assessment.returncode == 0) != accepts:
		raise ValueError(
			f"{dmg.name}: Gatekeeper should {'accept' if accepts else 'reject'} this signature, "
			f"and spctl ({assessments}) exited {assessment.returncode}: {verdict}"
		)
	return {
		"app": str(binary),
		"architecture": architecture,
		"signature": "Developer ID" if accepts else "ad-hoc",
		"gatekeeper": verdict,
		"assessments": assessments,
	}


def lacking(path: Path, environment: dict[str, str]) -> list[str]:
	"""The libraries `ldd` finds neither in the AppImage nor on this system for one file. A
	file `ldd` cannot read lacks nothing here; the launch is what proves it runs."""
	listed: str = subprocess.run(
		["ldd", str(path)], capture_output=True, text=True, env=environment
	).stdout
	return [line.split()[0] for line in listed.splitlines() if "=> not found" in line]


def missing_libraries(root: Path) -> dict[str, list[str]]:
	"""Every ELF file under an extracted AppImage that loads a library neither the AppImage
	nor this system has, with what it lacks. The AppImage's own library directories are on
	the search path, as its AppRun puts them."""
	directories: list[str] = sorted(
		{str(path.parent) for path in root.rglob("*.so*") if path.is_file()}
	)
	environment: dict[str, str] = {
		**os.environ,
		"LD_LIBRARY_PATH": ":".join(directories),
	}
	elfs: list[Path] = []
	for path in sorted(root.rglob("*")):
		if path.is_file() and not path.is_symlink():
			with path.open("rb") as stream:
				if stream.read(4) == b"\x7fELF":
					elfs.append(path)
	if not elfs:
		raise ValueError(f"{root} holds no ELF file")
	LOG.info("Resolving the libraries of %d ELF files", len(elfs))
	with ThreadPoolExecutor(max_workers=8) as executor:
		found: list[list[str]] = list(
			executor.map(lambda path: lacking(path, environment), elfs)
		)
	return {
		str(path.relative_to(root)): libraries
		for path, libraries in zip(elfs, found, strict=True)
		if libraries
	}


def appimage_libraries(appimage: Path) -> dict[str, object]:
	"""Extracts the AppImage and fails, naming them all, if anything it loads is missing."""
	extracted: Path = Path(tempfile.mkdtemp(prefix="stepwise-appimage-"))
	environment: dict[str, str] = {
		key: value
		for key, value in os.environ.items()
		if key != "APPIMAGE_EXTRACT_AND_RUN"
	}
	subprocess.run(
		[str(appimage), "--appimage-extract"],
		cwd=extracted,
		env=environment,
		check=True,
		stdout=subprocess.DEVNULL,
	)
	missing: dict[str, list[str]] = missing_libraries(extracted / "squashfs-root")
	if missing:
		raise ValueError(
			"libraries neither the AppImage nor this system has:\n"
			+ "\n".join(
				f"{file}: {', '.join(names)}" for file, names in missing.items()
			)
		)
	return {"appimage": str(appimage), "missing": missing}


@dataclass(frozen=True)
class Launch:
	"""How a launch went: when the page finished loading, and everything the app printed."""

	loaded_after: float
	page: str
	lines: list[str]


def read_lines(stream: IO[bytes], log: IO[str], lines: queue.Queue[str | None]) -> None:
	for raw in stream:
		line: str = raw.decode("utf-8", "replace").rstrip("\r\n")
		log.write(line + "\n")
		log.flush()
		lines.put(line)
	lines.put(None)


def stop(process: subprocess.Popen[bytes]) -> None:
	"""Ends the app and whatever it started: an AppImage's runtime and WebKit's helpers share
	its process group."""
	if process.poll() is not None:
		return
	os.killpg(process.pid, signal.SIGTERM)
	try:
		process.wait(timeout=10)
	except subprocess.TimeoutExpired:
		os.killpg(process.pid, signal.SIGKILL)
		process.wait(timeout=10)


def watch(
	command: Sequence[str],
	log: Path,
	while_alive: Callable[[], None],
	timeout: float,
	settle: float = SETTLE,
) -> Launch:
	"""Starts `command`, waits until the shell logs that the page finished loading, lets the
	page's watchdog pass, and calls `while_alive` with the app still running. The page must not
	have reported a fault, and the app must not have exited, at any point before that."""
	start: float = time.monotonic()
	lines: queue.Queue[str | None] = queue.Queue()
	seen: list[str] = []
	with log.open("w", encoding="utf-8") as written:
		process: subprocess.Popen[bytes] = subprocess.Popen(
			command,
			stdin=subprocess.DEVNULL,
			stdout=subprocess.PIPE,
			stderr=subprocess.STDOUT,
			start_new_session=True,
		)
		assert process.stdout is not None
		reader: threading.Thread = threading.Thread(
			target=read_lines, args=(process.stdout, written, lines), daemon=True
		)
		reader.start()
		try:
			page: str | None = None
			while page is None:
				remaining: float = timeout - (time.monotonic() - start)
				if remaining <= 0:
					raise TimeoutError(
						f"the page did not finish loading in {timeout:.0f}s"
					)
				try:
					line: str | None = lines.get(timeout=remaining)
				except queue.Empty:
					continue
				if line is None:
					raise RuntimeError(
						f"the app exited with {process.wait()} before its page finished loading"
					)
				seen.append(line)
				if LOADED.match(line):
					page = line
			loaded: float = time.monotonic() - start
			LOG.info("Page loaded after %.1fs: %s", loaded, page)
			time.sleep(settle)
			while not lines.empty():
				line = lines.get_nowait()
				if line is not None:
					seen.append(line)
			if (code := process.poll()) is not None:
				raise RuntimeError(f"the app exited with {code} after its page loaded")
			if faults := [line for line in seen if line.startswith(FAULT)]:
				raise RuntimeError("the page reported a fault: " + " | ".join(faults))
			while_alive()
			if (code := process.poll()) is not None:
				raise RuntimeError(
					f"the app exited with {code} while it was being checked"
				)
			return Launch(loaded_after=loaded, page=page, lines=seen)
		finally:
			stop(process)
			reader.join(timeout=10)
			process.stdout.close()


def screenshot(path: Path) -> None:
	"""The whole screen, from outside the app: the macOS session the runner logs in, or the
	X display the job started."""
	if sys.platform == "darwin":
		run("screencapture", "-x", str(path))
	else:
		run("import", "-window", "root", str(path))
	with path.open("rb") as image:
		if image.read(8) != b"\x89PNG\r\n\x1a\n":
			raise ValueError(f"{path} is not a PNG")


def launch(app: Path, evidence: Path, timeout: float) -> dict[str, object]:
	"""Launches the installed app on a progress file of its own, which an app that only opens
	must leave unwritten."""
	progress: Path = Path(tempfile.mkdtemp(prefix="stepwise 进度 ")) / "progress.json"
	image: Path = evidence / "launch.png"
	result: Launch = watch(
		[str(app), "--progress-file", str(progress)],
		evidence / "launch.log",
		lambda: screenshot(image),
		timeout,
	)
	if progress.exists():
		raise ValueError(f"opening the window wrote {progress}")
	return {
		"app": str(app),
		"appSha256": digest(app),
		"page": result.page,
		"loadedAfterSeconds": round(result.loaded_after, 1),
		"aliveAfterSeconds": SETTLE,
		"screenshot": image.name,
	}


def main() -> None:
	parser: argparse.ArgumentParser = argparse.ArgumentParser(description=__doc__)
	parser.add_argument("--evidence", type=Path, required=True)
	commands = parser.add_subparsers(dest="command", required=True)
	identifying: argparse.ArgumentParser = commands.add_parser("identify")
	identifying.add_argument("--artifacts", type=Path, required=True)
	identifying.add_argument("--name", required=True)
	installing: argparse.ArgumentParser = commands.add_parser("install")
	installing.add_argument("--installer", type=Path, required=True)
	unpacking: argparse.ArgumentParser = commands.add_parser("dmg")
	unpacking.add_argument("--dmg", type=Path, required=True)
	unpacking.add_argument("--target", required=True)
	unpacking.add_argument("--destination", type=Path, required=True)
	resolving: argparse.ArgumentParser = commands.add_parser("libraries")
	resolving.add_argument("--appimage", type=Path, required=True)
	launching: argparse.ArgumentParser = commands.add_parser("launch")
	launching.add_argument("--app", type=Path, required=True)
	launching.add_argument("--timeout", type=float, default=120)
	args: argparse.Namespace = parser.parse_args()
	args.evidence.mkdir(parents=True, exist_ok=True)
	report: dict[str, object]
	if args.command == "identify":
		report = dict(identify(args.artifacts, args.name))
	elif args.command == "install":
		report = dict(install(args.installer.resolve(), args.evidence))
	elif args.command == "dmg":
		args.destination.mkdir(parents=True, exist_ok=True)
		report = dict(unpack_dmg(args.dmg, args.destination, args.target))
	elif args.command == "libraries":
		report = appimage_libraries(args.appimage.resolve())
	else:
		report = launch(args.app, args.evidence, args.timeout)
	report.update(
		{
			key: os.environ.get(key, "")
			for key in ("GITHUB_SHA", "GITHUB_RUN_ID", "ImageOS", "ImageVersion")
		}
	)
	text: str = json.dumps(report, indent=2, ensure_ascii=False)
	(args.evidence / f"{args.command}.json").write_text(text + "\n", encoding="utf-8")
	print(text)
	if output := os.environ.get("GITHUB_OUTPUT"):
		with Path(output).open("a", encoding="utf-8") as stream:
			for key in ("path", "sha256", "app"):
				if key in report:
					stream.write(f"{key}={report[key]}\n")
	if summary := os.environ.get("GITHUB_STEP_SUMMARY"):
		with Path(summary).open("a", encoding="utf-8") as stream:
			stream.write(f"```json\n{text}\n```\n")


if __name__ == "__main__":
	logging.basicConfig(level=logging.INFO, format="%(levelname)s %(message)s")
	main()
