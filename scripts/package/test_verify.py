"""Regression checks for the package gates; the native jobs inspect the real installers."""

import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from check_rust_notices import check
from verify import glibc_report, glibc_versions, verify_resources


class Gates(unittest.TestCase):
	def test_glibc_report_keeps_binary_requirement_separate_from_bundled_libraries(
		self,
	) -> None:
		with tempfile.TemporaryDirectory() as temporary:
			root: Path = Path(temporary)
			tree: Path = root / "squashfs-root"
			binary: Path = tree / "usr/bin/stepwise-desktop"
			library: Path = tree / "usr/lib/libexample.so"
			for path in (binary, library):
				path.parent.mkdir(parents=True, exist_ok=True)
				path.write_bytes(b"\x7fELF")

			def required(path: Path) -> tuple[Path, tuple[int, ...]]:
				return path, (2, 38) if path == library else (2, 34)

			output: Path = root / "GLIBC.json"
			with patch("verify.glibc_versions", side_effect=required):
				glibc_report(tree, binary, root / "program.AppImage", output)
			report: dict[str, object] = json.loads(output.read_text(encoding="utf-8"))
			self.assertEqual(report["glibcMinimum"], "2.38")
			self.assertEqual(report["binaryGlibcMinimum"], "2.34")

	def test_glibc_requirement_ignores_exports_and_orders_versions_numerically(
		self,
	) -> None:
		# An exported 2.99 is not a requirement; 2.10 must sort after 2.9.
		output: str = """0000 DF *UND* 0 (GLIBC_2.9) old
0000 DF *UND* 0 (GLIBC_2.10) new
0000 g DF .text 0 GLIBC_2.99 provided
0000 DF *UND* 0 (GLIBCXX_3.4.30) cxx
"""
		with patch(
			"verify.subprocess.run",
			return_value=subprocess.CompletedProcess([], 0, output, ""),
		):
			self.assertEqual(glibc_versions(Path("program"))[1], (2, 10))

	def test_failed_objdump_is_not_reported_as_no_requirement(self) -> None:
		with patch(
			"verify.subprocess.run",
			return_value=subprocess.CompletedProcess([], 1, "", "corrupt ELF"),
		):
			with self.assertRaises(subprocess.CalledProcessError):
				glibc_versions(Path("program"))

	def test_cargo_about_spdx_fallback_is_rejected(self) -> None:
		with tempfile.TemporaryDirectory() as temporary:
			root: Path = Path(temporary)
			report: Path = root / "report.json"
			notice: Path = root / "notice.txt"
			notice.write_text("MIT License", encoding="utf-8")
			report.write_text(
				json.dumps(
					{"licenses": [{"id": "MIT", "source_path": None, "used_by": []}]}
				),
				encoding="utf-8",
			)
			with self.assertRaisesRegex(ValueError, "No licence file"):
				check(report, notice)
			report.write_text(
				json.dumps(
					{
						"licenses": [
							{"id": "MIT", "source_path": "LICENSE", "used_by": []}
						]
					}
				),
				encoding="utf-8",
			)
			notice.write_text(
				"Copyright (c) <year> <copyright holders>", encoding="utf-8"
			)
			with self.assertRaisesRegex(ValueError, "placeholder copyright"):
				check(report, notice)

	def test_installed_licences_must_match_every_source_byte(self) -> None:
		with tempfile.TemporaryDirectory() as temporary:
			root: Path = Path(temporary) / "repo"
			installed: Path = Path(temporary) / "installed/licenses"
			installed.mkdir(parents=True)
			paths: list[str] = [
				"LICENSE",
				"COPYRIGHT",
				"desktop/src-tauri/licenses/LXGW-WenKai-OFL.txt",
				"target/desktop-notices/THIRD-PARTY-NOTICES.txt",
				"target/desktop-notices/RUST-STD-COPYRIGHT.html",
				"desktop/ui/dist/JS-THIRD-PARTY-NOTICES.json",
			]
			for name in paths:
				source: Path = root / name
				source.parent.mkdir(parents=True, exist_ok=True)
				source.write_bytes(b"licence and copyright\n")
				(installed / source.name).write_bytes(source.read_bytes())
			with patch("verify.ROOT", root):
				verify_resources(installed.parent)
				(installed / "COPYRIGHT").write_bytes(b"licence without copyright\n")
				with self.assertRaisesRegex(ValueError, "differs from"):
					verify_resources(installed.parent)
				(installed / "COPYRIGHT").unlink()
				with self.assertRaises(FileNotFoundError):
					verify_resources(installed.parent)


if __name__ == "__main__":
	unittest.main()
