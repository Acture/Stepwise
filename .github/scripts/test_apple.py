"""A macOS package must never leave the signing job ad-hoc signed or unnotarized: missing
credentials, the wrong identity and a refused submission all fail, and an accepted dmg is
stapled before it is hashed."""

import base64
import hashlib
import json
import os
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch

from apple import (
	CREDENTIALS,
	notarize,
	required,
	setup,
	signing_identity,
	staple_ticket,
)

TEAM: str = "A1B2C3D4E5"
IDENTITY: str = f"Developer ID Application: Example Teacher ({TEAM})"
SHA1: str = "0123456789ABCDEF0123456789ABCDEF01234567"
KEY: str = "-----BEGIN PRIVATE KEY-----\nMIGTAgEA\n-----END PRIVATE KEY-----"


def credentials(**changes: str) -> dict[str, str]:
	values: dict[str, str] = {
		"APPLE_CERTIFICATE": base64.b64encode(b"p12 bytes").decode(),
		"APPLE_CERTIFICATE_PASSWORD": "export password",
		"APPLE_SIGNING_IDENTITY": IDENTITY,
		"APPLE_API_ISSUER": "c055ca8c-e5a8-4836-b61d-aa5794eeb3f4",
		"APPLE_API_KEY": "2X9R4HXF34",
		"APPLE_API_PRIVATE_KEY": KEY,
	}
	values.update(changes)
	return values


def fake(directory: Path, name: str, script: str) -> None:
	"""A stand-in for a macOS tool: records its arguments, one call per line, and runs `script`."""
	tool: Path = directory / name
	tool.write_text(
		"#!/bin/sh\n" f'printf "%s\\n" "{name} $*" >> "$CALLS"\n' + script,
		encoding="utf-8",
	)
	tool.chmod(tool.stat().st_mode | stat.S_IXUSR)


class Tools(unittest.TestCase):
	def setUp(self) -> None:
		temporary: tempfile.TemporaryDirectory[str] = tempfile.TemporaryDirectory()
		self.addCleanup(temporary.cleanup)
		self.root: Path = Path(temporary.name)
		self.bin: Path = self.root / "bin"
		self.bin.mkdir()
		self.calls: Path = self.root / "calls"
		self.calls.touch()
		self.enterContext(
			patch.dict(
				os.environ,
				{"PATH": f"{self.bin}:{os.environ['PATH']}", "CALLS": str(self.calls)},
			)
		)

	def recorded(self) -> list[str]:
		return self.calls.read_text(encoding="utf-8").splitlines()


class Credentials(Tools):
	def test_every_missing_or_blank_credential_is_named_at_once(self) -> None:
		environment: dict[str, str] = credentials(
			APPLE_SIGNING_IDENTITY=" ", APPLE_API_KEY=""
		)
		del environment["APPLE_CERTIFICATE"]
		with self.assertRaisesRegex(
			ValueError, "APPLE_CERTIFICATE, APPLE_SIGNING_IDENTITY, APPLE_API_KEY$"
		):
			required(environment, CREDENTIALS)
		self.assertEqual(required(credentials(), CREDENTIALS)[1], IDENTITY)

	def test_malformed_credentials_stop_before_any_keychain_exists(self) -> None:
		fake(self.bin, "security", "")
		for change, message in (
			({"APPLE_API_KEY": "2X9R4HXF34\n"}, "key ID"),
			({"APPLE_API_ISSUER": "issuer"}, "issuer ID"),
			({"APPLE_API_PRIVATE_KEY": "2X9R4HXF34"}, ".p8"),
			({"APPLE_CERTIFICATE": "not base64!"}, "base64"),
		):
			with self.subTest(change=change):
				with self.assertRaisesRegex(ValueError, message):
					setup(credentials(**change), self.root)
		self.assertEqual(self.recorded(), [])

	def test_the_identity_is_imported_searched_and_checked_and_the_key_kept_private(
		self,
	) -> None:
		fake(
			self.bin,
			"security",
			'case "$1" in\n'
			'  list-keychains) [ "$4" = -s ] || printf \'    "/Users/runner/Library/Keychains/login.keychain-db"\\n\' ;;\n'
			f"  find-identity) printf '  1) {SHA1} \"{IDENTITY}\"\\n     1 valid identities found\\n' ;;\n"
			# The certificate must still be on disk while it is imported.
			'  import) test -s "$2" ;;\n'
			"esac\n",
		)
		made: dict[str, str] = setup(credentials(), self.root)
		self.assertEqual(made["identity"], IDENTITY)
		calls: list[str] = self.recorded()
		self.assertEqual(
			[call.split()[1] for call in calls],
			[
				"create-keychain",
				"set-keychain-settings",
				"unlock-keychain",
				"import",
				"set-key-partition-list",
				"list-keychains",
				"list-keychains",
				"find-identity",
			],
		)
		keychain: str = made["keychain"]
		self.assertIn(
			f"security list-keychains -d user -s {keychain} /Users/runner/Library/Keychains/login.keychain-db",
			calls,
		)
		self.assertIn("-T /usr/bin/codesign", calls[3])
		self.assertNotIn("-A", calls[3].split())
		self.assertFalse((self.root / "stepwise-signing.p12").exists())
		# A .p12 exported without a password is imported with an empty one.
		self.calls.write_text("")
		environment: dict[str, str] = credentials()
		del environment["APPLE_CERTIFICATE_PASSWORD"]
		setup(environment, self.root)
		self.assertIn(" -P  -T /usr/bin/codesign", self.recorded()[3])
		key: Path = Path(made["notaryKey"])
		self.assertEqual(key.read_text(encoding="utf-8"), KEY + "\n")
		self.assertEqual(stat.S_IMODE(key.stat().st_mode), 0o600)

	def test_only_one_developer_id_application_identity_may_match(self) -> None:
		listing: str = (
			f'  1) {SHA1} "{IDENTITY}"\n'
			f'  2) {"F" * 40} "Apple Development: Example Teacher (ZZZZZZZZZZ)"\n'
			"     2 valid identities found\n"
		)
		self.assertEqual(signing_identity(listing, IDENTITY), IDENTITY)
		self.assertEqual(signing_identity(listing, SHA1), IDENTITY)
		with self.assertRaisesRegex(ValueError, "names 0"):
			signing_identity(listing, "Developer ID Application")
		with self.assertRaisesRegex(ValueError, "names 0"):
			signing_identity(listing, IDENTITY + "\n")
		with self.assertRaisesRegex(ValueError, "not a Developer ID Application"):
			signing_identity(listing, "F" * 40)
		with self.assertRaisesRegex(ValueError, "names 2"):
			signing_identity(listing + f'  3) {"E" * 40} "{IDENTITY}"\n', IDENTITY)


class Notarization(Tools):
	ENVIRONMENT: dict[str, str] = {
		"APPLE_API_KEY": "2X9R4HXF34",
		"APPLE_API_ISSUER": "c055ca8c-e5a8-4836-b61d-aa5794eeb3f4",
		"APPLE_API_KEY_PATH": "/runner/temp/stepwise-notary/AuthKey.p8",
	}

	def notary(self, status: str, code: int = 0) -> None:
		report: str = json.dumps(
			{
				"id": "6f1f4b4e-0000-4000-8000-000000000000",
				"status": status,
				"message": "Processing complete",
			}
		)
		fake(
			self.bin,
			"xcrun",
			'case "$1 $2" in\n'
			f"  'notarytool submit') printf '%s\\n' '{report}'; exit {code} ;;\n"
			"  'notarytool log') printf '{\"issues\": null}\\n' ;;\n"
			"  'stapler staple') printf 'ticket' >> \"$3\" ;;\n"
			"esac\n",
		)
		fake(self.bin, "ditto", 'printf "zip" > "$5"\n')

	def test_a_bare_executable_is_zipped_and_must_be_accepted(self) -> None:
		self.notary("Accepted")
		binary: Path = self.root / "stepwise"
		binary.write_bytes(b"signed binary")
		report: dict[str, str] = notarize(binary, False, self.ENVIRONMENT, self.root)
		self.assertEqual(report["status"], "Accepted")
		self.assertEqual(report["submission"], "6f1f4b4e-0000-4000-8000-000000000000")
		calls: list[str] = self.recorded()
		self.assertEqual(
			calls[0], f"ditto -c -k --keepParent {binary} {self.root / 'stepwise.zip'}"
		)
		self.assertTrue(
			calls[1].startswith(
				f"xcrun notarytool submit {self.root / 'stepwise.zip'} "
			)
		)
		self.assertIn("--wait", calls[1].split())
		self.assertTrue(calls[2].startswith("xcrun notarytool log 6f1f4b4e-"))
		self.assertEqual(len(calls), 3)
		self.assertEqual(binary.read_bytes(), b"signed binary")

	def test_a_refusal_fails_after_its_log_is_read(self) -> None:
		for code in (0, 1):
			with self.subTest(exit=code):
				self.notary("Invalid", code)
				dmg: Path = self.root / "Stepwise.dmg"
				dmg.write_bytes(b"dmg")
				with self.assertRaisesRegex(RuntimeError, "notarization ended Invalid"):
					notarize(dmg, True, self.ENVIRONMENT, self.root)
				self.assertTrue(self.recorded()[-1].startswith("xcrun notarytool log "))
				self.assertEqual(dmg.read_bytes(), b"dmg")

	def test_no_verdict_fails_without_a_log(self) -> None:
		fake(self.bin, "xcrun", "echo 'Error: HTTP status code: 401' >&2; exit 1\n")
		dmg: Path = self.root / "Stepwise.dmg"
		dmg.write_bytes(b"dmg")
		with self.assertRaisesRegex(RuntimeError, "without a verdict(?s:.*)401"):
			notarize(dmg, True, self.ENVIRONMENT, self.root)
		self.assertEqual(len(self.recorded()), 1)

	def test_an_accepted_dmg_is_stapled_before_it_is_hashed(self) -> None:
		self.notary("Accepted")
		dmg: Path = self.root / "Stepwise.dmg"
		dmg.write_bytes(b"dmg")
		report: dict[str, str] = notarize(dmg, True, self.ENVIRONMENT, self.root)
		self.assertEqual(
			[" ".join(call.split()[:3]) for call in self.recorded()],
			[
				"xcrun notarytool submit",
				"xcrun notarytool log",
				"xcrun stapler staple",
			],
		)
		self.assertEqual(dmg.read_bytes(), b"dmgticket")
		self.assertEqual(report["sha256"], hashlib.sha256(b"dmgticket").hexdigest())

	def test_stapling_is_tried_again_before_it_fails(self) -> None:
		# Fails while the ticket is still on its way to Apple's servers, here for two attempts.
		fake(
			self.bin,
			"xcrun",
			'case "$1 $2" in\n'
			"  'stapler staple') printf x >> \"$CALLS.staple\";"
			' [ "$(wc -c < "$CALLS.staple")" -gt 2 ] || { echo "Record not found"; exit 65; } ;;\n'
			"esac\n",
		)
		dmg: Path = self.root / "Stepwise.dmg"
		dmg.write_bytes(b"dmg")
		with patch("apple.STAPLE_BACKOFF", 0.0):
			staple_ticket(dmg)
			self.assertEqual(len(self.recorded()), 3)
			with self.assertRaisesRegex(RuntimeError, "stapling failed 3 times"):
				fake(self.bin, "xcrun", "echo 'Record not found'; exit 65\n")
				staple_ticket(dmg)
		self.assertEqual(len(self.recorded()), 6)

	def test_missing_notary_credentials_fail_before_submitting(self) -> None:
		self.notary("Accepted")
		dmg: Path = self.root / "Stepwise.dmg"
		dmg.write_bytes(b"dmg")
		with self.assertRaisesRegex(ValueError, "APPLE_API_KEY_PATH"):
			notarize(
				dmg, True, {**self.ENVIRONMENT, "APPLE_API_KEY_PATH": ""}, self.root
			)
		self.assertEqual(self.recorded(), [])


if __name__ == "__main__":
	unittest.main()
