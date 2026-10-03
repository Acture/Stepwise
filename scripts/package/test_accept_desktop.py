"""The desktop acceptance jobs must refuse the wrong bytes, and a launch must fail unless the
page finished loading in an app that stayed up without reporting a fault."""

import hashlib
import os
from pathlib import Path
import re
import sys
import tempfile
import unittest
from unittest.mock import patch

from accept_desktop import (
	SETTLE,
	gatekeeper_accepts,
	identify,
	missing_libraries,
	uncarried,
	watch,
)

LOADED: str = "Stepwise：页面 Finished tauri://localhost"


def app(*script: str) -> list[str]:
	"""A stand-in for the app: prints its lines as the shell would, UTF-8 on stderr."""
	body: str = "\n".join(("import sys, time", *script))
	return [sys.executable, "-c", body]


def say(line: str) -> str:
	return f"sys.stderr.buffer.write({line!r}.encode() + b'\\n'); sys.stderr.flush()"


class Installers(unittest.TestCase):
	def test_only_the_named_installer_with_its_recorded_hash_is_identified(
		self,
	) -> None:
		with tempfile.TemporaryDirectory() as temporary:
			root: Path = Path(temporary)
			name: str = "stepwise-desktop-0.1.0-x86_64-unknown-linux-gnu.deb"
			checksum: Path = root / (name + ".sha256")
			checksum.write_text(hashlib.sha256(b"rpm").hexdigest() + "\n")
			# A glob must never stand in for the exact file, even one whose bytes the
			# recorded hash would match.
			(root / "stepwise-desktop-0.1.0-x86_64-unknown-linux-gnu.rpm").write_bytes(
				b"rpm"
			)
			with self.assertRaises(FileNotFoundError):
				identify(root, name)
			installer: Path = root / name
			installer.write_bytes(b"changed bytes")
			checksum.write_text(hashlib.sha256(b"checked bytes").hexdigest() + "\n")
			with self.assertRaisesRegex(ValueError, "differ"):
				identify(root, name)
			checksum.write_text("not a SHA256\n")
			with self.assertRaisesRegex(ValueError, "differ"):
				identify(root, name)
			checksum.write_text(hashlib.sha256(b"changed bytes").hexdigest() + "\n")
			self.assertEqual(
				identify(root, name)["sha256"],
				hashlib.sha256(b"changed bytes").hexdigest(),
			)


class Gatekeeper(unittest.TestCase):
	def test_the_expected_verdict_follows_the_signature(self) -> None:
		self.assertFalse(
			gatekeeper_accepts("Identifier=dev.stepwise.desktop\nSignature=adhoc\n")
		)
		self.assertTrue(
			gatekeeper_accepts(
				"Authority=Developer ID Application: Someone (TEAM123456)\n"
				"Authority=Developer ID Certification Authority\n"
			)
		)
		with self.assertRaisesRegex(ValueError, "neither"):
			gatekeeper_accepts("Authority=Apple Development: Someone (TEAM123456)\n")


class Libraries(unittest.TestCase):
	def test_each_file_lacking_a_library_is_named_with_the_appimage_on_the_path(
		self,
	) -> None:
		with tempfile.TemporaryDirectory() as temporary:
			root: Path = Path(temporary)
			tree: Path = root / "squashfs-root"
			(tree / "usr/bin").mkdir(parents=True)
			(tree / "usr/lib").mkdir()
			(tree / "usr/bin/app").write_bytes(b"\x7fELF app")
			(tree / "usr/lib/libbundled.so.1").write_bytes(b"\x7fELF bundled")
			(tree / "usr/share.txt").write_text("not a program")
			# A stand-in for ldd: the app lacks one library outright, and finds the bundled
			# one only when the AppImage's own directory is searched.
			fake: Path = root / "bin"
			fake.mkdir()
			(fake / "ldd").write_text(
				"#!/bin/sh\n"
				'case "$1" in *share.txt) echo "should not be asked"; exit 1 ;; esac\n'
				'case "$1" in *app) printf "\\tlibmissing.so.1 => not found\\n" ;; esac\n'
				'case "$LD_LIBRARY_PATH" in *usr/lib*) ;; *) printf "\\tlibbundled.so.1 => not found\\n" ;; esac\n'
			)
			(fake / "ldd").chmod(0o755)
			with patch.dict(os.environ, {"PATH": f"{fake}:{os.environ['PATH']}"}):
				self.assertEqual(
					missing_libraries(tree), {"usr/bin/app": ["libmissing.so.1"]}
				)

	def test_a_library_is_carried_or_left_to_the_host_by_the_exclude_list(self) -> None:
		with tempfile.TemporaryDirectory() as temporary:
			root: Path = Path(temporary)
			tree: Path = root / "squashfs-root"
			(tree / "usr/bin").mkdir(parents=True)
			(tree / "usr/lib").mkdir()
			(tree / "usr/bin/app").write_bytes(b"\x7fELF app")
			(tree / "usr/lib/libcarried.so.1").write_bytes(b"\x7fELF carried")
			# A stand-in for objdump: the app names a carried library, two the host is
			# trusted with, and one nobody carries; the carried library names nothing.
			fake: Path = root / "bin"
			fake.mkdir()
			(fake / "objdump").write_text(
				"#!/bin/sh\n"
				'case "$2" in *app) printf "  NEEDED               %s\\n" '
				"libcarried.so.1 libc.so.6 libEGL.so.1 libglib-2.0.so.0 ;; esac\n"
			)
			(fake / "objdump").chmod(0o755)
			with patch.dict(os.environ, {"PATH": f"{fake}:{os.environ['PATH']}"}):
				self.assertEqual(
					uncarried(tree),
					(
						{"usr/bin/app": ["libglib-2.0.so.0"]},
						{"libc.so.6", "libEGL.so.1"},
					),
				)


class Launches(unittest.TestCase):
	def test_the_settle_outlasts_the_pages_watchdog(self) -> None:
		page: str = (
			Path(__file__).resolve().parents[2] / "src/desktop/ui/index.html"
		).read_text(encoding="utf-8")
		watchdog: list[str] = re.findall(r"\},\s*(\d+)\);\s*</script>", page)
		self.assertEqual(len(watchdog), 1, "index.html has one watchdog timer")
		self.assertGreater(SETTLE * 1000, int(watchdog[0]) + 1000)

	def launch(self, command: list[str], timeout: float = 20) -> list[str]:
		checked: list[str] = []
		with tempfile.TemporaryDirectory() as temporary:
			watch(
				command,
				Path(temporary) / "launch.log",
				lambda: checked.append("alive"),
				timeout,
				settle=0.5,
			)
		return checked

	def test_a_loaded_page_in_a_running_app_is_checked_while_it_runs(self) -> None:
		self.assertEqual(
			self.launch(
				app(
					say("Stepwise：页面 Started tauri://localhost"),
					say(LOADED),
					"time.sleep(60)",
				)
			),
			["alive"],
		)

	def test_an_app_that_exits_before_or_after_loading_fails(self) -> None:
		with self.assertRaisesRegex(RuntimeError, "before its page finished loading"):
			self.launch(app("sys.exit(3)"))
		with self.assertRaisesRegex(RuntimeError, "after its page loaded"):
			self.launch(app(say(LOADED)))

	def test_a_page_that_never_finishes_loading_times_out(self) -> None:
		with self.assertRaises(TimeoutError):
			self.launch(
				app(say("Stepwise：页面 Started tauri://localhost"), "time.sleep(60)"),
				timeout=1,
			)

	def test_a_fault_the_page_reports_fails_the_launch(self) -> None:
		with self.assertRaisesRegex(RuntimeError, "页面没有启动"):
			self.launch(
				app(
					say(LOADED),
					say("Stepwise 页面：页面没有启动：tauri://localhost"),
					"time.sleep(60)",
				)
			)


if __name__ == "__main__":
	unittest.main()
