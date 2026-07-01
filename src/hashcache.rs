//! hashcache — content-hash gating to skip expensive checks on unchanged files.
//!
//! Uses FNV-1a (64-bit) for file content hashing (fast, no external deps).
//! Cache is stored in `$XDG_CACHE_HOME/claudey/<subcommand>/` or
//! `~/.cache/claudey/<subcommand>/`.

use std::path::Path;

/// FNV-1a 64-bit offset basis.
const FNV1A_OFFSET_BASIS: u64 = 14695981039346656037;
/// FNV-1a 64-bit prime.
const FNV1A_PRIME: u64 = 1099511628211;

/// Compute FNV-1a 64-bit hash of a byte slice.
fn fnv1a(data: &[u8]) -> u64 {
    let mut hash = FNV1A_OFFSET_BASIS;
    for &b in data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV1A_PRIME);
    }
    hash
}

/// Hex-encode the FNV-1a hash of a canonical path for use as a cache filename.
fn path_key(path: &Path) -> Option<String> {
    let canonical = std::fs::canonicalize(path).ok()?;
    let bytes = canonical.to_string_lossy().as_bytes().to_vec();
    Some(format!("{:016x}", fnv1a(&bytes)))
}

/// Get the cache directory for a given subcommand.
fn cache_dir(subcommand: &str) -> Option<std::path::PathBuf> {
    let base = if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        if !xdg.is_empty() {
            std::path::PathBuf::from(xdg)
        } else {
            home_cache()?
        }
    } else {
        home_cache()?
    };
    Some(base.join("claudey").join(subcommand))
}

fn home_cache() -> Option<std::path::PathBuf> {
    std::env::var("HOME")
        .ok()
        .filter(|h| !h.is_empty())
        .map(|h| std::path::PathBuf::from(h).join(".cache"))
}

/// Returns `true` if `path` is unchanged since the last call for this subcommand.
/// Stores the hash in `~/.cache/claudey/<subcommand>/<path_hash>.hash`.
pub fn is_unchanged(subcommand: &str, path: &Path) -> bool {
    let dir = match cache_dir(subcommand) {
        Some(d) => d,
        None => return false,
    };
    let key = match path_key(path) {
        Some(k) => k,
        None => return false,
    };

    let cache_file = dir.join(format!("{key}.hash"));

    let stored = match std::fs::read_to_string(&cache_file) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let content = match std::fs::read(path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let current_hash = format!("{:016x}", fnv1a(&content));
    stored.trim() == current_hash
}

/// Update the stored hash for `path` under `subcommand`.
pub fn mark_checked(subcommand: &str, path: &Path) {
    let dir = match cache_dir(subcommand) {
        Some(d) => d,
        None => return,
    };
    let key = match path_key(path) {
        Some(k) => k,
        None => return,
    };

    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }

    let content = match std::fs::read(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let hash = format!("{:016x}", fnv1a(&content));
    let cache_file = dir.join(format!("{key}.hash"));
    let _ = std::fs::write(cache_file, hash);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_file_returns_true_on_second_call() {
        let tmp = crate::testutil::TempDir::new();
        let file = tmp.path().join("test.rs");
        std::fs::write(&file, "fn main() {}").unwrap();

        // Override cache location via env
        let cache_dir = tmp.path().join("cache");
        std::env::set_var("XDG_CACHE_HOME", &cache_dir);

        assert!(!is_unchanged("test-cmd", &file));
        mark_checked("test-cmd", &file);
        assert!(is_unchanged("test-cmd", &file));

        std::env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn modified_file_returns_false() {
        let tmp = crate::testutil::TempDir::new();
        let file = tmp.path().join("test.rs");
        std::fs::write(&file, "fn main() {}").unwrap();

        let cache_dir = tmp.path().join("cache");
        std::env::set_var("XDG_CACHE_HOME", &cache_dir);

        mark_checked("test-cmd2", &file);
        assert!(is_unchanged("test-cmd2", &file));

        // Modify the file
        std::fs::write(&file, "fn main() { println!(\"hi\"); }").unwrap();
        assert!(!is_unchanged("test-cmd2", &file));

        std::env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn missing_cache_entry_returns_false() {
        let tmp = crate::testutil::TempDir::new();
        let file = tmp.path().join("test.rs");
        std::fs::write(&file, "content").unwrap();

        let cache_dir = tmp.path().join("cache-missing");
        std::env::set_var("XDG_CACHE_HOME", &cache_dir);

        assert!(!is_unchanged("no-entry", &file));

        std::env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn fnv1a_basic() {
        // Sanity check: different inputs produce different hashes
        let h1 = fnv1a(b"hello");
        let h2 = fnv1a(b"world");
        assert_ne!(h1, h2);
        // Same input produces same hash
        assert_eq!(fnv1a(b"hello"), h1);
    }
}
