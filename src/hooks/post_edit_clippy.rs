//! post-edit-clippy hook — run clippy after editing .rs files.

use crate::{hashcache, hookio};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

const MAX_WALK_DEPTH: usize = 20;
const MAX_RELEVANT_LINES: usize = 10;

pub fn post_edit_clippy(input: Value, raw: Vec<u8>) {
    let file_path = hookio::get_tool_input_string(&input, "file_path");

    if file_path.is_empty() || !is_rust_file(file_path) {
        hookio::passthrough(&raw);
        return;
    }

    let path = Path::new(file_path);
    if hashcache::is_unchanged("post-edit-clippy", path) {
        hookio::log("[Hook] clippy: skipped (unchanged)");
        hookio::passthrough(&raw);
        return;
    }

    let resolved = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => {
            hookio::passthrough(&raw);
            return;
        }
    };

    let cargo_dir = match find_cargo_dir(&resolved) {
        Some(d) => d,
        None => {
            hookio::passthrough(&raw);
            return;
        }
    };

    let output = Command::new("cargo")
        .args(["clippy", "--message-format=short"])
        .current_dir(&cargo_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output();

    if let Ok(out) = output {
        if !out.status.success() {
            let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
            combined.push_str(&String::from_utf8_lossy(&out.stderr));

            let rel = resolved
                .strip_prefix(&cargo_dir)
                .ok()
                .map(|p| p.to_string_lossy().into_owned());
            let candidates: Vec<String> = [
                Some(file_path.to_string()),
                Some(resolved.to_string_lossy().into_owned()),
                rel,
            ]
            .into_iter()
            .flatten()
            .filter(|s| !s.is_empty())
            .collect();

            let relevant: Vec<&str> = combined
                .lines()
                .filter(|line| candidates.iter().any(|c| line.contains(c)))
                .take(MAX_RELEVANT_LINES)
                .collect();

            if !relevant.is_empty() {
                let base = Path::new(file_path)
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| file_path.to_string());
                hookio::log(&format!("[Hook] clippy warnings in {base}:"));
                for line in relevant {
                    hookio::log(line);
                }
            }
        } else {
            hashcache::mark_checked("post-edit-clippy", path);
        }
    }

    hookio::passthrough(&raw);
}

pub fn is_rust_file(path: &str) -> bool {
    path.ends_with(".rs")
}

pub fn find_cargo_dir(start: &Path) -> Option<PathBuf> {
    let mut dir = start.parent()?.to_path_buf();
    for _ in 0..MAX_WALK_DEPTH {
        if dir.join("Cargo.toml").is_file() {
            return Some(dir);
        }
        match dir.parent() {
            Some(p) if p != dir => dir = p.to_path_buf(),
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_rust_file_matches_rs() {
        assert!(is_rust_file("foo.rs"));
        assert!(is_rust_file("/abs/path/main.rs"));
        assert!(is_rust_file("src/lib.rs"));
    }

    #[test]
    fn is_rust_file_rejects_non_rs() {
        assert!(!is_rust_file("foo.ts"));
        assert!(!is_rust_file("foo.py"));
        assert!(!is_rust_file("foo.rsx"));
        assert!(!is_rust_file("foo"));
        assert!(!is_rust_file(""));
    }

    #[test]
    fn find_cargo_dir_walks_up() {
        let base = crate::testutil::TempDir::new();
        let nested = base.path().join("a").join("b").join("c");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(base.path().join("Cargo.toml"), "[package]").unwrap();

        let file = nested.join("file.rs");
        std::fs::write(&file, "").unwrap();

        let found = find_cargo_dir(&file).expect("Cargo.toml should be found");
        assert_eq!(
            std::fs::canonicalize(found).unwrap(),
            std::fs::canonicalize(base.path()).unwrap()
        );
    }

    #[test]
    fn find_cargo_dir_returns_none_when_absent() {
        let base = crate::testutil::TempDir::new();
        let file = base.path().join("file.rs");
        std::fs::write(&file, "").unwrap();
        assert!(find_cargo_dir(&file).is_none());
    }
}
