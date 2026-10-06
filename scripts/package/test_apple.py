"""A macOS package must never leave the signing job ad-hoc signed or unnotarized: missing
credentials, the wrong identity, a refused submission and a signature or verdict short of
notarized Developer ID all fail."""

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
	developer_id,
	notarize,
	notarized,
	required,
	setup,
	signing_identity,
)

TEAM: str = "A1B2C3D4E5"
IDENTITY: str = f"Developer ID Application: Example Teacher ({TEAM})"
SHA1: str = "0123456789ABCDEF0123456789ABCDEF01234567"
KEY: str = "-----BEGIN PRIVATE KEY-----\nMIGTAgEA\n-----END PRIVATE KEY-----"
# What `codesign -dvv` prints for a notarizable app, trimmed.
SIGNED: str = f"""Executable=/Volumes/Stepwise/Stepwise.app/Contents/MacOS/stepwise-desktop
Identifier=io.github.acture.stepwise
Format=app bundle with Mach-O thin (arm64)
CodeDirectory v=20500 size=1234 flags=0x10000(runtime) hashes=27+7 location=embedded
Signature size=9000
Authority={IDENTITY}
Authority=Developer ID Certification Authority
Authority=Apple Root CA
Timestamp=6 Oct 2026 at 10:00:00
Info.plist entries=20
TeamIdentifier={TEAM}
Runtime Version=26.0.0
Sealed Resources version=2 rules=13 files=10
Internal requirements count=1 size=200
"""


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
		environment = patch.dict(
			os.environ,
			{"PATH": f"{self.bin}:{os.environ['PATH']}", "CALLS": str(self.calls)},
		)
		environment.start()
		self.addCleanup(environment.stop)

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
		self.assertEqual(required(credentials(), CREDENTIALS)[2], IDENTITY)

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


class Signatures(unittest.TestCase):
	def test_a_developer_id_signature_is_read_back(self) -> None:
		signature = developer_id(SIGNED)
		self.assertEqual(signature.authority, IDENTITY)
		self.assertEqual(signature.team, TEAM)
		self.assertEqual(signature.timestamp, "6 Oct 2026 at 10:00:00")
		self.assertEqual(signature.identifier, "io.github.acture.stepwise")
		self.assertTrue(signature.runtime)
		dmg: str = SIGNED.replace("flags=0x10000(runtime)", "flags=0x0(none)")
		self.assertFalse(developer_id(dmg).runtime)

	def test_anything_short_of_a_timestamped_developer_id_signature_fails(self) -> None:
		for wrong, message in (
			(
				"Identifier=stepwise\nSignature=adhoc\nTeamIdentifier=not set\n",
				"ad-hoc",
			),
			(
				SIGNED.replace(
					"Developer ID Application: ", "Apple Development: "
				).replace("Developer ID Certification Authority", "Apple Worldwide"),
				"Developer ID Application",
			),
			(SIGNED.replace("Authority=Apple Root CA\n", ""), "Apple's root"),
			(
				SIGNED.replace(f"TeamIdentifier={TEAM}", "TeamIdentifier=ZZZZZZZZZZ"),
				"team",
			),
			(
				SIGNED.replace("Timestamp=", "Signed Time="),
				"no secure timestamp",
			),
			# What codesign prints when it cannot reach the trust daemon.
			(
				SIGNED.replace(f"Authority={IDENTITY}\n", "Authority=(unavailable)\n"),
				"Developer ID Application",
			),
		):
			with self.subTest(message=message):
				with self.assertRaisesRegex(ValueError, message):
					developer_id(wrong)


class Gatekeeper(unittest.TestCase):
	def test_only_an_acceptance_as_notarized_developer_id_passes(self) -> None:
		notarized(
			"/tmp/Stepwise.app: accepted\nsource=Notarized Developer ID\n"
			f"origin={IDENTITY}",
			0,
		)
		for verdict, code in (
			(
				f"/tmp/Stepwise.app: rejected\nsource=Unnotarized Developer ID\norigin={IDENTITY}",
				3,
			),
			(f"/tmp/Stepwise.app: accepted\nsource=Developer ID\norigin={IDENTITY}", 0),
			("/tmp/stepwise: internal error in Code Signing subsystem", 1),
			("/tmp/Stepwise.app: accepted\nsource=Notarized Developer ID", 3),
		):
			with self.subTest(verdict=verdict):
				with self.assertRaisesRegex(ValueError, "notarized Developer ID"):
					notarized(verdict, code)


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
				"xcrun stapler validate",
			],
		)
		self.assertEqual(dmg.read_bytes(), b"dmgticket")
		self.assertEqual(report["sha256"], hashlib.sha256(b"dmgticket").hexdigest())

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
