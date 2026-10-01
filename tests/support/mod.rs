//! Explicit acceptance tests may run a shipped binary instead of Cargo's debug build.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

fn resolve(binary: Option<&Path>, expected: Option<&str>, ci: bool) -> Result<PathBuf, String> {
	match (binary, expected) {
		(Some(path), Some(expected)) => {
			if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
				return Err("STEPWISE_BINARY_SHA256 must be a SHA256 digest".into());
			}
			let bytes: Vec<u8> = std::fs::read(path).map_err(|error| error.to_string())?;
			let actual: String = format!("{:x}", Sha256::digest(&bytes));
			if !actual.eq_ignore_ascii_case(expected) {
				return Err(format!("{}: SHA256 {actual} differs from {expected}", path.display()));
			}
			eprintln!("Acceptance binary: {} SHA256 {actual}", path.display());
			path.canonicalize().map_err(|error| error.to_string())
		}
		(None, None) if !ci => Ok(PathBuf::from(env!("CARGO_BIN_EXE_stepwise"))),
		_ => Err("set both STEPWISE_BINARY and STEPWISE_BINARY_SHA256; CI never falls back to Cargo's binary".into()),
	}
}

pub fn binary() -> PathBuf {
	let path: Option<std::ffi::OsString> = std::env::var_os("STEPWISE_BINARY");
	let expected: Option<String> = std::env::var("STEPWISE_BINARY_SHA256").ok();
	resolve(
		path.as_deref().map(Path::new),
		expected.as_deref(),
		std::env::var_os("CI").is_some(),
	)
	.expect("the acceptance binary must be identified and verified")
}

#[test]
fn an_unidentified_or_changed_binary_cannot_be_accepted() {
	let directory: tempfile::TempDir = tempfile::tempdir().unwrap();
	let path: PathBuf = directory.path().join("binary");
	std::fs::write(&path, b"checked bytes").unwrap();
	let digest: String = format!("{:x}", Sha256::digest(b"checked bytes"));
	assert_eq!(
		resolve(Some(&path), Some(&digest), true).unwrap(),
		path.canonicalize().unwrap()
	);
	std::fs::write(&path, b"changed bytes").unwrap();
	assert!(resolve(Some(&path), Some(&digest), true).is_err());
	assert!(resolve(None, None, true).is_err());
	assert!(resolve(Some(&path), None, false).is_err());
	assert!(resolve(None, Some(&digest), false).is_err());
	assert!(resolve(Some(&path), Some("bad hash"), true).is_err());
}
