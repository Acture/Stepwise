"""Identify one downloaded desktop installer, install it or take the app out of its dmg, and
launch an installed app until its page has loaded. Nothing here builds the app: every check
reads the shipped bytes."""

from __future__ import annotations

import argparse
from collections.abc import Callable, Sequence
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict, dataclass
import hashlib
import json
import logging
import os
from pathlib import Path
import platform
import queue
import re
import signal
import subprocess
import sys
import tempfile
import threading
import time
from typing import IO

from apple import Signature, gatekeeper, signature, stapled

LOG: logging.Logger = logging.getLogger(__name__)

# The shell's page-load line (src/desktop/src-tauri/src/main.rs, `on_page_load`), and the prefix of
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


def unpack_dmg(dmg: Path, destination: Path, target: str) -> dict[str, object]:
	"""Verifies the dmg, copies its app out and checks the copy's architecture; then that the
	dmg and the app are both signed with Developer ID, carry a stapled notarization ticket, and
	are accepted by this Mac's Gatekeeper as notarized. Returns where the copy's executable is."""
	show("hdiutil", "verify", str(dmg))
	mount: Path = Path(tempfile.mkdtemp(prefix="stepwise-dmg-"))
	show(
		"hdiutil",
		"attach",
		"-readonly",
		"-nobrowse",
		"-mountpoint",
		str(mount),
		str(dmg),
	)
	try:
		show("ditto", str(mount / "Stepwise.app"), str(destination / "Stepwise.app"))
	finally:
		show("hdiutil", "detach", str(mount))
	app: Path = destination / "Stepwise.app"
	binary: Path = app / "Contents/MacOS/stepwise-desktop"
	wanted: str = "arm64" if target.startswith("aarch64-") else "x86_64"
	architecture: str = run("lipo", "-archs", str(binary)).stdout.strip()
	if architecture != wanted:
		raise ValueError(f"{dmg.name}: expected {wanted} alone, got {architecture}")
	signed: Signature = signature(app, executable=True)
	return {
		"app": str(binary),
		"architecture": architecture,
		"authority": signed.authority,
		"team": signed.team,
		"timestamp": signed.timestamp,
		"dmgSignature": asdict(signature(dmg, executable=False)),
		"stapled": {"dmg": stapled(dmg), "app": stapled(app)},
		"gatekeeper": {"dmg": gatekeeper(dmg, "dmg"), "app": gatekeeper(app, "app")},
	}


# AppImage's exclude list, as linuxdeploy carries it: what every desktop has, so no AppImage
# carries it. From https://github.com/probonopd/AppImages/blob/master/excludelist.
HOST: frozenset[str] = frozenset(
	{
		"ld-linux.so.2",
		"ld-linux-x86-64.so.2",
		"libanl.so.1",
		"libBrokenLocale.so.1",
		"libcidn.so.1",
		"libc.so.6",
		"libdl.so.2",
		"libm.so.6",
		"libmvec.so.1",
		"libnss_compat.so.2",
		"libnss_dns.so.2",
		"libnss_files.so.2",
		"libnss_hesiod.so.2",
		"libnss_nisplus.so.2",
		"libnss_nis.so.2",
		"libpthread.so.0",
		"libresolv.so.2",
		"librt.so.1",
		"libthread_db.so.1",
		"libutil.so.1",
		"libstdc++.so.6",
		"libGL.so.1",
		"libEGL.so.1",
		"libGLdispatch.so.0",
		"libGLX.so.0",
		"libOpenGL.so.0",
		"libdrm.so.2",
		"libglapi.so.0",
		"libgbm.so.1",
		"libxcb.so.1",
		"libX11.so.6",
		"libX11-xcb.so.1",
		"libwayland-client.so.0",
		"libasound.so.2",
		"libfontconfig.so.1",
		"libfreetype.so.6",
		"libharfbuzz.so.0",
		"libcom_err.so.2",
		"libexpat.so.1",
		"libgcc_s.so.1",
		"libgpg-error.so.0",
		"libICE.so.6",
		"libSM.so.6",
		"libusb-1.0.so.0",
		"libuuid.so.1",
		"libz.so.1",
		"libjack.so.0",
		"libpipewire-0.3.so.0",
		"libxcb-dri3.so.0",
		"libxcb-dri2.so.0",
		"libfribidi.so.0",
		"libgmp.so.10",
	}
)


def elf_files(root: Path) -> list[Path]:
	elfs: list[Path] = []
	for path in sorted(root.rglob("*")):
		if path.is_file() and not path.is_symlink():
			with path.open("rb") as stream:
				if stream.read(4) == b"\x7fELF":
					elfs.append(path)
	if not elfs:
		raise ValueError(f"{root} holds no ELF file")
	return elfs


def needed(path: Path) -> list[str]:
	"""The libraries an ELF file names in its dynamic section."""
	return [
		line.split()[1]
		for line in run("objdump", "-p", str(path)).stdout.splitlines()
		if line.split()[:1] == ["NEEDED"]
	]


def uncarried(root: Path) -> tuple[dict[str, list[str]], set[str]]:
	"""Every ELF file under an extracted AppImage that names a library the AppImage neither
	carries nor may leave to the host, with those libraries; and what it does leave to the
	host. It reads each file's own dynamic section, so what this system happens to have
	cannot stand in for a library the AppImage stopped carrying."""
	carried: set[str] = {path.name for path in root.rglob("*")}
	elfs: list[Path] = elf_files(root)
	LOG.info("Reading the dynamic sections of %d ELF files", len(elfs))
	with ThreadPoolExecutor(max_workers=8) as executor:
		names: list[list[str]] = list(executor.map(needed, elfs))
	gaps: dict[str, list[str]] = {}
	host: set[str] = set()
	for path, libraries in zip(elfs, names, strict=True):
		host.update(name for name in libraries if name in HOST and name not in carried)
		if lack := [
			name for name in libraries if name not in carried and name not in HOST
		]:
			gaps[str(path.relative_to(root))] = lack
	return gaps, host


def lacking(path: Path, environment: dict[str, str]) -> list[str]:
	"""The libraries `ldd` finds neither in the AppImage nor on this system for one file. A
	file `ldd` cannot read lacks nothing here; the launch is what proves it runs."""
	listed: str = subprocess.run(
		["ldd", str(path)], capture_output=True, text=True, env=environment
	).stdout
	return [line.split()[0] for line in listed.splitlines() if "=> not found" in line]


def bundled(root: Path) -> dict[str, str]:
	"""The AppImage's own library directories first on the search path, as its AppRun puts
	them."""
	directories: list[str] = sorted(
		{str(path.parent) for path in root.rglob("*.so*") if path.is_file()}
	)
	return {**os.environ, "LD_LIBRARY_PATH": ":".join(directories)}


def missing_libraries(root: Path) -> dict[str, list[str]]:
	"""Every ELF file under an extracted AppImage that loads a library neither the AppImage
	nor this system has, with what it lacks: what this system must still provide."""
	environment: dict[str, str] = bundled(root)
	elfs: list[Path] = elf_files(root)
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


def webkit_version(root: Path) -> str:
	"""The version of the WebKitGTK the AppImage carries and renders with, from the library
	itself."""
	libraries: list[Path] = list(root.rglob("libwebkit2gtk-4.1.so.0"))
	if len(libraries) != 1:
		raise ValueError(f"expected one libwebkit2gtk-4.1.so.0, got {libraries}")
	ask: str = (
		"import ctypes, sys; library = ctypes.CDLL(sys.argv[1]); "
		"print('.'.join(str(getattr(library, f'webkit_get_{part}_version')()) "
		"for part in ('major', 'minor', 'micro')))"
	)
	return subprocess.run(
		[sys.executable, "-c", ask, str(libraries[0])],
		env=bundled(root),
		check=True,
		stdout=subprocess.PIPE,
		text=True,
	).stdout.strip()


def appimage_libraries(appimage: Path) -> dict[str, object]:
	"""Extracts the AppImage and fails, naming them all, if a library it loads is neither
	carried nor one an AppImage may leave to the host, or is nowhere on this system."""
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
	root: Path = extracted / "squashfs-root"
	gaps, host = uncarried(root)
	if gaps:
		raise ValueError(
			"libraries the AppImage neither carries nor may leave to the host:\n"
			+ "\n".join(f"{file}: {', '.join(names)}" for file, names in gaps.items())
		)
	missing: dict[str, list[str]] = missing_libraries(root)
	if missing:
		raise ValueError(
			"libraries neither the AppImage nor this system has:\n"
			+ "\n".join(
				f"{file}: {', '.join(names)}" for file, names in missing.items()
			)
		)
	return {
		"appimage": str(appimage),
		"webkitgtk": webkit_version(root),
		"leftToHost": sorted(host),
	}


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
		show("screencapture", "-x", str(path))
	else:
		show("import", "-window", "root", str(path))
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
	# A job container has no runner image variables; it names its own system instead.
	if Path("/etc/os-release").is_file():
		report["os"] = platform.freedesktop_os_release()["PRETTY_NAME"]
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
