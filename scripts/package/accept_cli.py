"""Identify and extract exactly one downloaded CLI archive before running acceptance tests."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tarfile
import zipfile


def digest(path: Path) -> str:
	return hashlib.sha256(path.read_bytes()).hexdigest()


def extract(archive: Path, name: str, executable: str, destination: Path) -> Path:
	wanted: set[str] = {
		f"{name}/{file}"
		for file in (
			executable,
			"LICENSE",
			"README.md",
			"THIRD-PARTY-NOTICES.txt",
			"RUST-STD-COPYRIGHT.html",
		)
	}
	files: dict[str, bytes] = {}
	binary_mode: int = 0
	if archive.suffix == ".zip":
		with zipfile.ZipFile(archive) as bundle:
			for entry in bundle.infolist():
				if entry.is_dir() and entry.filename.rstrip("/") == name:
					continue
				if entry.filename not in wanted or entry.filename in files:
					raise ValueError(
						f"Unexpected or repeated archive entry: {entry.filename}"
					)
				files[entry.filename] = bundle.read(entry)
	else:
		with tarfile.open(archive, "r:gz") as bundle:
			for entry in bundle.getmembers():
				if entry.isdir() and entry.name.rstrip("/") == name:
					continue
				if (
					not entry.isfile()
					or entry.name not in wanted
					or entry.name in files
				):
					raise ValueError(
						f"Unexpected or repeated archive entry: {entry.name}"
					)
				stream = bundle.extractfile(entry)
				if stream is None:
					raise ValueError(f"Unreadable archive entry: {entry.name}")
				files[entry.name] = stream.read()
				if entry.name == f"{name}/{executable}":
					binary_mode = entry.mode
	if files.keys() != wanted or any(not contents for contents in files.values()):
		raise ValueError(f"Missing or empty archive entries: {wanted - files.keys()}")
	if executable == "stepwise" and not binary_mode & 0o111:
		raise ValueError("The archived Unix binary is not executable")
	# Write only the five known paths, never archive-supplied links or traversal paths.
	for relative, contents in files.items():
		path: Path = destination / relative
		path.parent.mkdir(parents=True, exist_ok=True)
		path.write_bytes(contents)
	binary: Path = destination / name / executable
	if executable == "stepwise":
		binary.chmod(binary_mode & 0o777)
	return binary.resolve()


def verify_windows_imports(binary: Path) -> None:
	vswhere: Path = (
		Path(os.environ["ProgramFiles(x86)"])
		/ "Microsoft Visual Studio/Installer/vswhere.exe"
	)
	paths: str = subprocess.check_output(
		[
			str(vswhere),
			"-latest",
			"-products",
			"*",
			"-requires",
			"Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
			"-find",
			r"VC\Tools\MSVC\**\bin\Hostx64\x64\dumpbin.exe",
		],
		text=True,
	)
	if not paths.strip():
		raise ValueError("vswhere found no dumpbin")
	imports: str = subprocess.check_output(
		[paths.strip().splitlines()[-1], "/nologo", "/dependents", str(binary)],
		text=True,
	)
	print(imports)
	if (
		"Image has the following dependencies" not in imports
		or "kernel32.dll" not in imports.lower()
	):
		raise ValueError("dumpbin listed no imports")
	if re.search(r"vcruntime|msvcp|ucrtbase|api-ms-win-crt-", imports, re.IGNORECASE):
		raise ValueError("The downloaded Windows CLI imports a C runtime")


def identify(
	artifacts: Path, target: str, version: str, notices: Path, destination: Path
) -> dict[str, str]:
	windows: bool = target == "x86_64-pc-windows-msvc"
	name: str = f"stepwise-{version}-{target}"
	archive: Path = artifacts / (name + (".zip" if windows else ".tar.gz"))
	expected: str = (
		archive.with_name(archive.name + ".sha256").read_text(encoding="utf-8").strip()
	)
	actual: str = digest(archive)
	if not re.fullmatch(r"[a-fA-F0-9]{64}", expected) or actual != expected.lower():
		raise ValueError(
			f"{archive.name}: downloaded bytes differ from the packaging job's SHA256"
		)
	binary: Path = extract(
		archive, name, "stepwise.exe" if windows else "stepwise", destination
	)
	root: Path = Path(__file__).resolve().parents[2]
	for file, source in {
		"LICENSE": root / "LICENSE",
		"README.md": root / "README.md",
		"THIRD-PARTY-NOTICES.txt": notices / target / "THIRD-PARTY-NOTICES.txt",
		"RUST-STD-COPYRIGHT.html": notices / "RUST-STD-COPYRIGHT.html",
	}.items():
		if (binary.parent / file).read_bytes() != source.read_bytes():
			raise ValueError(f"{archive.name}: {file} differs from its source")
	return {
		"target": target,
		"archive": archive.name,
		"archiveSha256": actual,
		"binary": str(binary),
		"binarySha256": digest(binary),
	}


def main() -> None:
	parser: argparse.ArgumentParser = argparse.ArgumentParser(description=__doc__)
	parser.add_argument("--target", required=True)
	parser.add_argument("--version", required=True)
	parser.add_argument("--artifacts", type=Path, default=Path("downloaded"))
	parser.add_argument("--notices", type=Path, default=Path("notices"))
	parser.add_argument("--destination", type=Path, default=Path("accepted"))
	args: argparse.Namespace = parser.parse_args()
	report: dict[str, str] = identify(
		args.artifacts, args.target, args.version, args.notices, args.destination
	)
	if args.target == "x86_64-pc-windows-msvc":
		verify_windows_imports(Path(report["binary"]))
	report.update(
		{
			key: os.environ.get(key, "")
			for key in ("GITHUB_SHA", "GITHUB_RUN_ID", "ImageOS", "ImageVersion")
		}
	)
	evidence: Path = Path("target/cli-evidence")
	evidence.mkdir(parents=True, exist_ok=True)
	text: str = json.dumps(report, indent=2)
	(evidence / "identity.json").write_text(text + "\n", encoding="utf-8")
	print(text)
	with Path(os.environ["GITHUB_ENV"]).open("a", encoding="utf-8") as environment:
		environment.write(
			f"STEPWISE_BINARY={report['binary']}\nSTEPWISE_BINARY_SHA256={report['binarySha256']}\n"
		)
	with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a", encoding="utf-8") as summary:
		summary.write(
			f"Downloaded `{report['archive']}`\n\nArchive SHA256: `{report['archiveSha256']}`\n\nBinary SHA256: `{report['binarySha256']}`\n"
		)


if __name__ == "__main__":
	main()
