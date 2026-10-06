"""Developer ID signing and notarization for the macOS packaging jobs, and the checks that read a
signature, a stapled ticket and Gatekeeper's verdict back. Credentials come only from the
environment, and nothing here prints them."""

from __future__ import annotations

import argparse
import base64
import binascii
from collections.abc import Mapping, Sequence
from dataclasses import asdict, dataclass
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
CREDENTIALS: tuple[str, ...] = (
	"APPLE_CERTIFICATE",
	"APPLE_CERTIFICATE_PASSWORD",
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
# How Gatekeeper is asked about each kind of item. `--type execute` judges only apps, so a bare
# executable is assessed as something to install, and a dmg by its own signature.
ASSESSMENTS: dict[str, tuple[str, ...]] = {
	"app": ("--type", "execute"),
	"dmg": ("--type", "open", "--context", "context:primary-signature"),
	"binary": ("--type", "install"),
}


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
	shown: str = " ".join("***" if arg in hidden else arg for arg in args)
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
	certificate, password, identity, issuer, key, private_key = required(
		environment, CREDENTIALS
	)
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
		hidden=(identity,),
	)


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
		run("xcrun", "stapler", "staple", str(path))
		stapled(path)
	return {
		"file": path.name,
		"sha256": digest(path),
		"submission": submitted,
		"status": status,
		"stapled": str(staple).lower(),
	}


@dataclass(frozen=True)
class Signature:
	"""A Developer ID signature with a secure timestamp, as `codesign -dvv` describes it."""

	identifier: str
	authority: str
	team: str
	timestamp: str
	runtime: bool


def developer_id(description: str) -> Signature:
	"""Reads `codesign -dvv`'s description of a signature. It must be a Developer ID Application
	certificate's under Apple's root, from the team the signature names, with a secure
	timestamp: Apple notarizes nothing signed ad hoc, by another kind of certificate, or with a
	signing time from the local clock."""
	fields: dict[str, list[str]] = {}
	for line in description.splitlines():
		name, separator, value = line.partition("=")
		if separator:
			fields.setdefault(name, []).append(value)
	if fields.get("Signature") == ["adhoc"]:
		raise ValueError("an ad-hoc signature, not Developer ID")
	authorities: list[str] = fields.get("Authority", [])
	leaf: re.Match[str] | None = (
		re.fullmatch(r"Developer ID Application: .+ \(([A-Z0-9]{10})\)", authorities[0])
		if authorities
		else None
	)
	if leaf is None or authorities[1:] != [
		"Developer ID Certification Authority",
		"Apple Root CA",
	]:
		raise ValueError(
			f"not signed by a Developer ID Application certificate under Apple's root: {authorities}"
		)
	teams: list[str] = fields.get("TeamIdentifier", [])
	if teams != [leaf.group(1)]:
		raise ValueError(
			f"TeamIdentifier {teams} is not the certificate's team {leaf.group(1)}"
		)
	timestamps: list[str] = fields.get("Timestamp", [])
	if len(timestamps) != 1:
		raise ValueError(
			f"no secure timestamp (Signed Time {fields.get('Signed Time', [])})"
		)
	flags: re.Match[str] | None = re.search(
		r"\bflags=0x[0-9a-f]+\(([^)]*)\)", description
	)
	return Signature(
		identifier=fields.get("Identifier", [""])[0],
		authority=authorities[0],
		team=teams[0],
		timestamp=timestamps[0],
		runtime=flags is not None and "runtime" in flags.group(1).split(","),
	)


def signature(path: Path, executable: bool) -> Signature:
	"""Verifies a signature as strictly as notarization does and reads it back; an executable,
	or an app, must also run with the hardened runtime."""
	run("codesign", "--verify", "--deep", "--strict", "--verbose=2", str(path))
	# codesign describes a signature on stderr.
	description: str = run("codesign", "-dvv", str(path)).stderr
	print(description, flush=True)
	found: Signature = developer_id(description)
	if executable and not found.runtime:
		raise ValueError(f"{path}: signed without the hardened runtime")
	return found


def notarized(verdict: str, code: int) -> None:
	"""spctl exits 0 when it accepts, 3 when it refuses and 1 when it could not assess at all;
	only an acceptance as notarized Developer ID code passes."""
	if (
		code != 0
		or not re.search(r": accepted$", verdict, re.MULTILINE)
		or not re.search(r"^source=Notarized Developer ID$", verdict, re.MULTILINE)
	):
		raise ValueError(
			f"Gatekeeper did not accept it as notarized Developer ID code (spctl exited {code}):\n{verdict}"
		)


def gatekeeper(path: Path, kind: str) -> str:
	"""Gatekeeper's verdict on an item, which must be accepted as notarized. An item with no
	stapled ticket, such as a bare executable, is looked up online."""
	# A Mac that assesses nothing accepts anything, so its verdict would say nothing.
	assessments: str = run("spctl", "--status").stdout.strip()
	if assessments != "assessments enabled":
		raise ValueError(f"Gatekeeper is not assessing on this Mac: {assessments}")
	result: subprocess.CompletedProcess[str] = subprocess.run(
		["spctl", "--assess", "-vv", *ASSESSMENTS[kind], str(path)],
		capture_output=True,
		text=True,
		encoding="utf-8",
	)
	# spctl writes its verdict on stderr.
	verdict: str = (result.stdout + result.stderr).strip()
	LOG.info("Gatekeeper (exit %d): %s", result.returncode, verdict)
	notarized(verdict, result.returncode)
	return verdict


def check(binary: Path, online: bool) -> dict[str, object]:
	"""The signature of a CLI binary as it ships, and with `online`, Gatekeeper's verdict on it,
	which for a bare executable depends on Apple's servers knowing its notarized code."""
	report: dict[str, object] = {
		"binary": str(binary),
		"sha256": digest(binary),
		**asdict(signature(binary, executable=True)),
	}
	if online:
		report["gatekeeper"] = gatekeeper(binary, "binary")
	return report


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
	checking: argparse.ArgumentParser = commands.add_parser("check")
	checking.add_argument("--gatekeeper", action="store_true")
	checking.add_argument("--evidence", type=Path)
	checking.add_argument("binary", type=Path)
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
	elif args.command == "check":
		checked: dict[str, object] = check(args.binary, args.gatekeeper)
		text: str = json.dumps(checked, indent=2)
		print(text, flush=True)
		if args.evidence is not None:
			args.evidence.mkdir(parents=True, exist_ok=True)
			(args.evidence / "signature.json").write_text(text + "\n", encoding="utf-8")
		summarize(
			f"`{args.binary.name}` SHA256 `{checked['sha256']}`: {checked['authority']},"
			f" timestamp {checked['timestamp']}"
			+ (", Gatekeeper: notarized Developer ID" if args.gatekeeper else "")
		)
	else:
		cleanup(temporary)


if __name__ == "__main__":
	logging.basicConfig(level=logging.INFO, format="%(levelname)s %(message)s")
	main()
