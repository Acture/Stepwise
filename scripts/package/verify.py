"""Inspect the installers that ship, then hash those same bytes. Never launch the app."""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import logging
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time
import tomllib

ROOT: Path = Path(__file__).resolve().parents[2]
LOG: logging.Logger = logging.getLogger(__name__)


def run(*args: str, cwd: Path | None = None) -> str:
	LOG.info("%s", " ".join(args))
	return subprocess.run(
		args, cwd=cwd, check=True, stdout=subprocess.PIPE, text=True, encoding="utf-8"
	).stdout


def digest(path: Path) -> str:
	with path.open("rb") as stream:
		return hashlib.file_digest(stream, "sha256").hexdigest()


def verify_resources(directory: Path) -> None:
	"""Compare every installed licence to its source, not just the filename or length."""
	expected: dict[str, Path] = {
		"LICENSE": ROOT / "LICENSE",
		"COPYRIGHT": ROOT / "COPYRIGHT",
		"LXGW-WenKai-OFL.txt": ROOT / "desktop/src-tauri/licenses/LXGW-WenKai-OFL.txt",
		"THIRD-PARTY-NOTICES.txt": ROOT
		/ "target/desktop-notices/THIRD-PARTY-NOTICES.txt",
		"RUST-STD-COPYRIGHT.html": ROOT
		/ "target/desktop-notices/RUST-STD-COPYRIGHT.html",
		"JS-THIRD-PARTY-NOTICES.json": ROOT
		/ "desktop/ui/dist/JS-THIRD-PARTY-NOTICES.json",
	}
	licenses: list[Path] = list(directory.rglob("licenses/LICENSE"))
	if len(licenses) != 1:
		raise ValueError(
			f"{directory}: expected one installed licenses/LICENSE, got {licenses}"
		)
	for name, source in expected.items():
		installed: Path = licenses[0].parent / name
		if source.stat().st_size == 0 or digest(installed) != digest(source):
			raise ValueError(f"{installed}: differs from {source}")
	LOG.info("Verified all licence bytes in %s", licenses[0].parent)


def glibc_versions(path: Path) -> tuple[Path, tuple[int, ...]]:
	result: subprocess.CompletedProcess[str] = subprocess.run(
		["objdump", "-T", str(path)], capture_output=True, text=True, encoding="utf-8"
	)
	# Static ELF files have no dynamic symbols. Every other failure remains a failure.
	if result.returncode != 0 and "not a dynamic object" not in result.stderr:
		result.check_returncode()
	versions: list[tuple[int, ...]] = [
		tuple(map(int, version.split(".")))
		for line in result.stdout.splitlines()
		if "*UND*" in line
		for version in re.findall(r"\bGLIBC_(\d+(?:\.\d+)+)\b", line)
	]
	return path, max(versions, default=())


def glibc_report(tree: Path, binary: Path, image: Path, output: Path) -> None:
	elfs: list[Path] = []
	for path in sorted(tree.rglob("*")):
		if path.is_file() and not path.is_symlink():
			with path.open("rb") as stream:
				if stream.read(4) == b"\x7fELF":
					elfs.append(path)
	if not elfs:
		raise ValueError("AppImage contains no ELF files")
	# The AppImage runtime itself must also be loadable before its payload can run.
	paths: list[Path] = [binary, image, *elfs]
	records: dict[str, str | None] = {}
	maximum: tuple[int, ...] = ()
	with ThreadPoolExecutor(max_workers=8) as executor:
		for index, (path, version) in enumerate(executor.map(glibc_versions, paths), 1):
			name: str = (
				str(path.relative_to(tree)) if path.is_relative_to(tree) else path.name
			)
			records[name] = ".".join(map(str, version)) if version else None
			maximum = max(maximum, version)
			if index % 25 == 0 or index == len(paths):
				LOG.info("GLIBC symbol inspection: %d/%d ELF files", index, len(paths))
	binary_key: str = str(binary.relative_to(tree))
	if not maximum or not records[binary_key]:
		raise ValueError("No GLIBC requirement found for the desktop binary")
	floor: str = ".".join(map(str, maximum))
	output.write_text(
		json.dumps(
			{
				"glibcMinimum": floor,
				"binaryGlibcMinimum": records[binary_key],
				"files": records,
			},
			indent=2,
		)
		+ "\n",
		encoding="utf-8",
	)
	LOG.info(
		"Highest required GLIBC symbol version: %s (not a runtime support claim)", floor
	)


def verify_macos(installer: Path, extracted: Path, target: str) -> None:
	run(
		"hdiutil",
		"attach",
		"-readonly",
		"-nobrowse",
		"-mountpoint",
		str(extracted),
		str(installer),
	)
	try:
		app: Path = extracted / "Stepwise.app"
		run("codesign", "--verify", "--deep", "--strict", str(app))
		binary: Path = app / "Contents/MacOS/stepwise-desktop"
		architecture: str = run("lipo", "-archs", str(binary)).strip()
		wanted: str = "arm64" if target.startswith("aarch64-") else "x86_64"
		if architecture != wanted:
			raise ValueError(f"{installer}: expected {wanted}, got {architecture}")
		minimum: str = run(
			"/usr/libexec/PlistBuddy",
			"-c",
			"Print :LSMinimumSystemVersion",
			str(app / "Contents/Info.plist"),
		).strip()
		if minimum != "11.0":
			raise ValueError(f"{installer}: minimumSystemVersion is {minimum}")
		verify_resources(app)
	finally:
		run("hdiutil", "detach", str(extracted))


def verify_windows(installer: Path, extracted: Path) -> None:
	if installer.suffix == ".msi":
		# An administrative extraction; it does not install or launch the application.
		result: subprocess.CompletedProcess[str] = subprocess.run(
			[
				"msiexec",
				"/a",
				str(installer),
				"/qn",
				f"TARGETDIR={extracted}",
				"/l*v",
				str(extracted / "msi.log"),
			],
			text=True,
		)
		if result.returncode != 0:
			LOG.error("%s", (extracted / "msi.log").read_text(encoding="utf-16"))
			result.check_returncode()
	else:
		run("7z", "x", "-y", f"-o{extracted}", str(installer))
	if len(list(extracted.rglob("stepwise-desktop.exe"))) != 1:
		raise ValueError(f"{installer}: expected one desktop executable")
	verify_resources(extracted)


def verify_linux(installer: Path, extracted: Path, evidence: Path) -> None:
	if installer.suffix == ".deb":
		run("dpkg-deb", "--extract", str(installer), str(extracted))
	elif installer.suffix == ".rpm":
		run("bsdtar", "-xf", str(installer), "-C", str(extracted))
	else:
		installer.chmod(installer.stat().st_mode | 0o111)
		run(str(installer), "--appimage-extract", cwd=extracted)
		extracted = extracted / "squashfs-root"
	verify_resources(extracted)
	binaries: list[Path] = [
		p for p in extracted.rglob("stepwise-desktop") if p.is_file()
	]
	if len(binaries) != 1:
		raise ValueError(
			f"{installer}: expected one desktop executable, got {binaries}"
		)
	if installer.suffix == ".AppImage":
		glibc_report(extracted, binaries[0], installer, evidence / "GLIBC.json")


def main() -> None:
	parser: argparse.ArgumentParser = argparse.ArgumentParser(description=__doc__)
	parser.add_argument("--target", required=True)
	args: argparse.Namespace = parser.parse_args()
	target: str = args.target
	version: str = tomllib.loads((ROOT / "desktop/src-tauri/Cargo.toml").read_text())[
		"package"
	]["version"]
	bundle: Path = ROOT / "target" / target / "release/bundle"
	dist: Path = ROOT / "dist"
	evidence: Path = ROOT / "target/package-evidence"
	dist.mkdir(exist_ok=True)
	evidence.mkdir(parents=True, exist_ok=True)
	formats: dict[str, str]
	if target.endswith("apple-darwin"):
		formats = {"dmg/*.dmg": ".dmg"}
	elif target.endswith("windows-msvc"):
		formats = {"nsis/*-setup.exe": "-setup.exe", "msi/*.msi": ".msi"}
	else:
		formats = {
			"deb/*.deb": ".deb",
			"rpm/*.rpm": ".rpm",
			"appimage/*.AppImage": ".AppImage",
		}
	for pattern, suffix in formats.items():
		start: float = time.monotonic()
		matches: list[Path] = list(bundle.glob(pattern))
		if len(matches) != 1:
			raise ValueError(f"Expected one {pattern} in {bundle}, got {matches}")
		installer: Path = dist / f"stepwise-desktop-{version}-{target}{suffix}"
		shutil.copyfile(matches[0], installer)
		LOG.info("Inspecting %s", installer.name)
		with tempfile.TemporaryDirectory(prefix="stepwise-package-") as temporary:
			extracted: Path = Path(temporary)
			if suffix == ".dmg":
				verify_macos(installer, extracted, target)
			elif suffix in (".msi", "-setup.exe"):
				verify_windows(installer, extracted)
			else:
				verify_linux(installer, extracted, evidence)
		installer.with_name(installer.name + ".sha256").write_text(
			digest(installer) + "\n", encoding="ascii", newline="\n"
		)
		LOG.info(
			"Verified and hashed %s in %.1fs", installer.name, time.monotonic() - start
		)
		if summary := os.environ.get("GITHUB_STEP_SUMMARY"):
			with Path(summary).open("a", encoding="utf-8") as stream:
				stream.write(
					f"- `{installer.name}`: extracted licence bytes verified\n"
				)


if __name__ == "__main__":
	logging.basicConfig(level=logging.INFO, format="%(levelname)s %(message)s")
	main()
