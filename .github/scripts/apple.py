"""Developer ID signing and notarization for the macOS packaging jobs, and the check that reads
a stapled ticket back. Credentials come only from the environment. The certificate, its
password, the notary key and its IDs are never printed; the signing identity is, as every
signature names it, though GitHub masks it in a job given it."""

from __future__ import annotations

import argparse
import base64
import binascii
from collections.abc import Mapping, Sequence
import hashlib
import json
import logging
import os
from pathlib import Path
import re
import secrets
import shutil
import subprocess
import time

LOG: logging.Logger = logging.getLogger(__name__)

# Every one is required: a macOS package is never signed ad hoc, and never ships unnotarized.
# APPLE_CERTIFICATE_PASSWORD is not among them: a .p12 exported without a password needs none,
# and a missing or wrong one fails the import, before the build.
CREDENTIALS: tuple[str, ...] = (
	"APPLE_CERTIFICATE",
	"APPLE_SIGNING_IDENTITY",
	"APPLE_API_ISSUER",
	"APPLE_API_KEY",
	"APPLE_API_PRIVATE_KEY",
)
NOTARY: tuple[str, ...] = ("APPLE_API_KEY", "APPLE_API_ISSUER", "APPLE_API_KEY_PATH")
# notarytool takes only these; anything else is zipped for submission.
SUBMITTABLE: frozenset[str] = frozenset({".dmg", ".pkg", ".zip"})
# Apple: most submissions finish within 5 minutes, 98 percent within 15.
NOTARY_TIMEOUT: str = "20m"
# Right after an Accepted verdict the ticket may not have reached Apple's servers yet, and
# stapling then fails with "Record not found"; Apple's advice is to try again.
STAPLE_ATTEMPTS: int = 3
STAPLE_BACKOFF: float = 30.0


def keychain_path(temporary: Path) -> Path:
	return temporary / "stepwise-signing.keychain-db"


def notary_key_path(temporary: Path) -> Path:
	return temporary / "stepwise-notary" / "AuthKey.p8"


def digest(path: Path) -> str:
	with path.open("rb") as stream:
		return hashlib.file_digest(stream, "sha256").hexdigest()


def run(*args: str, hidden: Sequence[str] = ()) -> subprocess.CompletedProcess[str]:
	"""Runs a command and returns its output. The command is logged, and a failure reported
	with what the command printed, with every hidden value (a password, a key's identity)
	masked."""
	shown: str = " ".join("***" if arg and arg in hidden else arg for arg in args)
	LOG.info("%s", shown)
	result: subprocess.CompletedProcess[str] = subprocess.run(
		args, capture_output=True, text=True, encoding="utf-8"
	)
	if result.returncode != 0:
		raise RuntimeError(
			f"{shown} exited {result.returncode}:\n{result.stdout}{result.stderr}"
		)
	return result


def required(environment: Mapping[str, str], names: Sequence[str]) -> list[str]:
	"""The named values, in order. An unset repository secret reaches a job as an empty string,
	so a blank value counts as missing; every missing name is reported at once."""
	missing: list[str] = [
		name for name in names if not environment.get(name, "").strip()
	]
	if missing:
		raise ValueError("missing Apple credentials: " + ", ".join(missing))
	return [environment[name] for name in names]


def write_private(path: Path, data: bytes) -> None:
	path.parent.mkdir(parents=True, exist_ok=True)
	descriptor: int = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
	with os.fdopen(descriptor, "wb") as stream:
		stream.write(data)


def signing_identity(listing: str, wanted: str) -> str:
	"""The one valid identity in `security find-identity -v` output whose name or SHA-1 is
	`wanted`. It must be a Developer ID Application identity: Apple notarizes nothing else."""
	identities: list[tuple[str, str]] = re.findall(
		r'^\s*\d+\)\s+([0-9A-F]{40})\s+"(.+)"$', listing, re.MULTILINE
	)
	matches: list[str] = [name for sha1, name in identities if wanted in (sha1, name)]
	if len(matches) != 1:
		raise ValueError(
			f"APPLE_SIGNING_IDENTITY names {len(matches)} of the valid signing identities, "
			f"not one: {[name for _, name in identities]}"
		)
	if not matches[0].startswith("Developer ID Application: "):
		raise ValueError(f"{matches[0]} is not a Developer ID Application identity")
	return matches[0]


def setup(environment: Mapping[str, str], temporary: Path) -> dict[str, str]:
	"""Checks that every credential is there, imports the Developer ID identity into a keychain
	of its own on the user's search list, where `codesign` and `tauri build` find it by name,
	and writes the notary key to a file only this user can read."""
	certificate, identity, issuer, key, private_key = required(environment, CREDENTIALS)
	password: str = environment.get("APPLE_CERTIFICATE_PASSWORD", "")
	# Checked as given: Tauri and notarytool receive these values unchanged.
	if not re.fullmatch(r"[A-Z0-9]{10}", key):
		raise ValueError("APPLE_API_KEY is not an App Store Connect key ID")
	if not re.fullmatch(r"[0-9a-fA-F-]{36}", issuer):
		raise ValueError("APPLE_API_ISSUER is not an App Store Connect issuer ID")
	if "-----BEGIN PRIVATE KEY-----" not in private_key:
		raise ValueError("APPLE_API_PRIVATE_KEY is not the text of a .p8 key")
	try:
		p12: bytes = base64.b64decode("".join(certificate.split()), validate=True)
	except binascii.Error as error:
		raise ValueError("APPLE_CERTIFICATE is not base64") from error
	keychain: Path = keychain_path(temporary)
	secret: str = secrets.token_urlsafe(24)
	# Masked in the job's log should a failing command ever print it.
	print(f"::add-mask::{secret}", flush=True)
	run("security", "create-keychain", "-p", secret, str(keychain), hidden=(secret,))
	# Unlocked for six hours, past any build: codesign cannot use a locked keychain.
	run("security", "set-keychain-settings", "-lut", "21600", str(keychain))
	run("security", "unlock-keychain", "-p", secret, str(keychain), hidden=(secret,))
	exported: Path = temporary / "stepwise-signing.p12"
	write_private(exported, p12)
	try:
		run(
			"security",
			"import",
			str(exported),
			"-k",
			str(keychain),
			"-f",
			"pkcs12",
			"-P",
			password,
			"-T",
			"/usr/bin/codesign",
			hidden=(password,),
		)
	finally:
		exported.unlink()
	# Lets codesign use the private key without a prompt nobody could answer.
	run(
		"security",
		"set-key-partition-list",
		"-S",
		"apple-tool:,apple:",
		"-s",
		"-k",
		secret,
		str(keychain),
		hidden=(secret,),
	)
	searched: list[str] = re.findall(
		r'"([^"]+)"', run("security", "list-keychains", "-d", "user").stdout
	)
	run("security", "list-keychains", "-d", "user", "-s", str(keychain), *searched)
	# The whole search list: the System keychain holds the Developer ID intermediate.
	name: str = signing_identity(
		run("security", "find-identity", "-v", "-p", "codesigning").stdout, identity
	)
	notary_key: Path = notary_key_path(temporary)
	write_private(notary_key, private_key.strip().encode() + b"\n")
	return {"identity": name, "keychain": str(keychain), "notaryKey": str(notary_key)}


def sign(
	binary: Path, identifier: str, environment: Mapping[str, str], temporary: Path
) -> None:
	"""Signs a bare executable as Apple requires for notarization: Developer ID, hardened
	runtime, a secure timestamp, and an identifier of its own instead of its file name."""
	(identity,) = required(environment, ("APPLE_SIGNING_IDENTITY",))
	run(
		"codesign",
		"--force",
		"--options",
		"runtime",
		"--timestamp",
		"--identifier",
		identifier,
		"--keychain",
		str(keychain_path(temporary)),
		"--sign",
		identity,
		str(binary),
	)


def staple_ticket(path: Path) -> None:
	for attempt in range(1, STAPLE_ATTEMPTS + 1):
		result: subprocess.CompletedProcess[str] = subprocess.run(
			["xcrun", "stapler", "staple", str(path)],
			capture_output=True,
			text=True,
			encoding="utf-8",
		)
		if result.returncode == 0:
			return
		LOG.warning(
			"xcrun stapler staple %s, attempt %d of %d, exited %d: %s",
			path.name,
			attempt,
			STAPLE_ATTEMPTS,
			result.returncode,
			(result.stdout + result.stderr).strip(),
		)
		if attempt < STAPLE_ATTEMPTS:
			time.sleep(STAPLE_BACKOFF * attempt)
	raise RuntimeError(f"{path.name}: stapling failed {STAPLE_ATTEMPTS} times")


def stapled(path: Path) -> str:
	"""Passes only when a notarization ticket is stapled to the item itself."""
	return run("xcrun", "stapler", "validate", str(path)).stdout.strip()


def notarize(
	path: Path, staple: bool, environment: Mapping[str, str], temporary: Path
) -> dict[str, str]:
	"""Submits `path` to Apple's notary service and fails unless its verdict is Accepted; the
	service's log is printed whatever the verdict. A bare executable cannot hold a ticket and is
	submitted in a zip of its own; a dmg can, and with `staple` gets it."""
	key, issuer, key_path = required(environment, NOTARY)
	authentication: tuple[str, ...] = (
		"--key",
		key_path,
		"--key-id",
		key,
		"--issuer",
		issuer,
	)
	hidden: tuple[str, ...] = (key, issuer)
	submission: Path = path
	if path.suffix not in SUBMITTABLE:
		submission = temporary / f"{path.name}.zip"
		submission.unlink(missing_ok=True)
		run("ditto", "-c", "-k", "--keepParent", str(path), str(submission))
	LOG.info("Submitting %s to the notary service", submission.name)
	start: float = time.monotonic()
	result: subprocess.CompletedProcess[str] = subprocess.run(
		[
			"xcrun",
			"notarytool",
			"submit",
			str(submission),
			*authentication,
			"--wait",
			"--timeout",
			NOTARY_TIMEOUT,
			"--output-format",
			"json",
		],
		capture_output=True,
		text=True,
		encoding="utf-8",
	)
	# Its exit status for a refusal is undocumented, so the verdict is read from its report.
	outcome: dict[str, str] = (
		json.loads(result.stdout) if result.stdout.lstrip().startswith("{") else {}
	)
	submitted: str = outcome.get("id", "")
	status: str = outcome.get("status", "")
	LOG.info(
		"Notary service: %s for submission %s after %.1f min",
		status or "no verdict",
		submitted or "(none)",
		(time.monotonic() - start) / 60,
	)
	# Apple: always read the log, even when notarization succeeds.
	if submitted and status in ("Accepted", "Invalid", "Rejected"):
		print(
			run(
				"xcrun", "notarytool", "log", submitted, *authentication, hidden=hidden
			).stdout,
			flush=True,
		)
	if result.returncode != 0 or status != "Accepted":
		raise RuntimeError(
			f"{path.name}: notarization ended {status or 'without a verdict'} "
			f"(exit {result.returncode}) {outcome.get('message', '')}\n{result.stderr}"
		)
	if staple:
		# Stapling writes the ticket into the dmg, so its bytes are hashed only after this.
		staple_ticket(path)
	return {
		"file": path.name,
		"sha256": digest(path),
		"submission": submitted,
		"status": status,
		"stapled": str(staple).lower(),
	}


def cleanup(temporary: Path) -> None:
	"""Deletes the keychain, which also takes it off the search list, and the notary key."""
	keychain: Path = keychain_path(temporary)
	if keychain.exists():
		run("security", "delete-keychain", str(keychain))
	shutil.rmtree(notary_key_path(temporary).parent, ignore_errors=True)


def summarize(line: str) -> None:
	if summary := os.environ.get("GITHUB_STEP_SUMMARY"):
		with Path(summary).open("a", encoding="utf-8") as stream:
			stream.write(f"- {line}\n")


def main() -> None:
	parser: argparse.ArgumentParser = argparse.ArgumentParser(description=__doc__)
	commands = parser.add_subparsers(dest="command", required=True)
	commands.add_parser("setup")
	signing: argparse.ArgumentParser = commands.add_parser("sign")
	signing.add_argument("--identifier", required=True)
	signing.add_argument("binary", type=Path)
	notarizing: argparse.ArgumentParser = commands.add_parser("notarize")
	notarizing.add_argument("--staple", action="store_true")
	notarizing.add_argument("path", type=Path)
	commands.add_parser("cleanup")
	args: argparse.Namespace = parser.parse_args()
	temporary: Path = Path(os.environ["RUNNER_TEMP"])
	if args.command == "setup":
		made: dict[str, str] = setup(os.environ, temporary)
		with Path(os.environ["GITHUB_ENV"]).open("a", encoding="utf-8") as stream:
			stream.write(f"APPLE_API_KEY_PATH={made['notaryKey']}\n")
		LOG.info("Signing as %s from %s", made["identity"], made["keychain"])
	elif args.command == "sign":
		sign(args.binary, args.identifier, os.environ, temporary)
	elif args.command == "notarize":
		report: dict[str, str] = notarize(args.path, args.staple, os.environ, temporary)
		print(json.dumps(report, indent=2), flush=True)
		summarize(
			f"`{report['file']}`: notarization {report['status']}, submission"
			f" `{report['submission']}`{', ticket stapled' if args.staple else ''},"
			f" SHA256 `{report['sha256']}`"
		)
	else:
		cleanup(temporary)


if __name__ == "__main__":
	logging.basicConfig(level=logging.INFO, format="%(levelname)s %(message)s")
	main()
