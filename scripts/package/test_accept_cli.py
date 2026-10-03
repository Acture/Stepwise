"""The acceptance job must refuse missing, altered and ambiguous archive bytes."""

import hashlib
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile

from accept_cli import extract, identify


class Archives(unittest.TestCase):
	def test_only_the_exact_file_can_be_identified(self) -> None:
		with tempfile.TemporaryDirectory() as temporary:
			root: Path = Path(temporary)
			# A glob must never substitute a different target/version for the requested archive.
			(root / "stepwise-other.zip").write_bytes(b"different target")
			with self.assertRaises(FileNotFoundError):
				identify(root, "x86_64-pc-windows-msvc", "0.1.0", root, root / "out")
			archive: Path = root / "stepwise-0.1.0-x86_64-pc-windows-msvc.zip"
			archive.write_bytes(b"changed bytes")
			checksum: Path = archive.with_name(archive.name + ".sha256")
			checksum.write_text(hashlib.sha256(b"checked bytes").hexdigest())
			with self.assertRaisesRegex(ValueError, "downloaded bytes differ"):
				identify(root, "x86_64-pc-windows-msvc", "0.1.0", root, root / "out")
			checksum.write_text("not a SHA256")
			with self.assertRaisesRegex(ValueError, "downloaded bytes differ"):
				identify(root, "x86_64-pc-windows-msvc", "0.1.0", root, root / "out")

	def test_archive_members_must_be_complete_and_unambiguous(self) -> None:
		with tempfile.TemporaryDirectory() as temporary:
			root: Path = Path(temporary)
			archive: Path = root / "cli.zip"
			files: list[str] = [
				"stepwise.exe",
				"LICENSE",
				"README.md",
				"THIRD-PARTY-NOTICES.txt",
				"RUST-STD-COPYRIGHT.html",
			]
			for members in (files[:-1], files + ["../outside"], files):
				with zipfile.ZipFile(archive, "w") as bundle:
					for file in members:
						bundle.writestr("cli/" + file, b"payload")
				if members == files:
					self.assertEqual(
						extract(
							archive, "cli", "stepwise.exe", root / "out"
						).read_bytes(),
						b"payload",
					)
				else:
					with self.assertRaises(ValueError):
						extract(archive, "cli", "stepwise.exe", root / "out")

	def test_tar_links_cannot_replace_the_program(self) -> None:
		with tempfile.TemporaryDirectory() as temporary:
			root: Path = Path(temporary)
			archive: Path = root / "cli.tar.gz"
			with tarfile.open(archive, "w:gz") as bundle:
				entry: tarfile.TarInfo = tarfile.TarInfo("cli/stepwise")
				entry.type = tarfile.SYMTYPE
				entry.linkname = "../../other-program"
				bundle.addfile(entry, io.BytesIO())
			with self.assertRaisesRegex(ValueError, "Unexpected"):
				extract(archive, "cli", "stepwise", root / "out")

	def test_a_tar_binary_must_ship_with_execute_permission(self) -> None:
		with tempfile.TemporaryDirectory() as temporary:
			root: Path = Path(temporary)
			archive: Path = root / "cli.tar.gz"
			with tarfile.open(archive, "w:gz") as bundle:
				for file in (
					"stepwise",
					"LICENSE",
					"README.md",
					"THIRD-PARTY-NOTICES.txt",
					"RUST-STD-COPYRIGHT.html",
				):
					entry: tarfile.TarInfo = tarfile.TarInfo(f"cli/{file}")
					entry.size = 7
					entry.mode = 0o644
					bundle.addfile(entry, io.BytesIO(b"payload"))
			with self.assertRaisesRegex(ValueError, "not executable"):
				extract(archive, "cli", "stepwise", root / "out")


if __name__ == "__main__":
	unittest.main()
