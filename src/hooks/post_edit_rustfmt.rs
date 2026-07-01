//! post-edit-rustfmt hook — format .rs files with rustfmt after edits.

use crate::{hashcache, hookio, sysutil};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

pub fn post_edit_rustfmt(input: Value, raw: Vec<u8>) {
    let file_path = hookio::get_tool_input_string(&input, "file_path");

    if file_path.is_empty() || !file_path.ends_with(".rs") {
        hookio::passthrough(&raw);
        return;
    }

    let path = Path::new(file_path);
    if hashcache::is_unchanged("post-edit-rustfmt", path) {
        hookio::log("[Hook] rustfmt: skipped (unchanged)");
        hookio::passthrough(&raw);
        return;
    }

    if !sysutil::command_exists("rustfmt") {
        hookio::log("[Hook] rustfmt not found, skipping");
        hookio::passthrough(&raw);
        return;
    }

    let output = Command::new("rustfmt")
        .args(["--edition", "2021", file_path])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            hashcache::mark_checked("post-edit-rustfmt", path);
            let base = Path::new(file_path)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| file_path.to_string());
            hookio::log(&format!("[Hook] rustfmt: formatted {base}"));
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if !stderr.is_empty() {
                hookio::log(&format!("[Hook] rustfmt error: {}", stderr.trim()));
            }
        }
        Err(e) => {
            hookio::log(&format!("[Hook] rustfmt failed to run: {e}"));
        }
    }

    hookio::passthrough(&raw);
}

#[cfg(test)]
mod tests {
    #[test]
    fn rustfmt_skips_non_rs_files() {
        // Verify the extension check logic
        assert!(!"foo.ts".ends_with(".rs"));
        assert!(!"foo.py".ends_with(".rs"));
        assert!("foo.rs".ends_with(".rs"));
        assert!("/path/to/main.rs".ends_with(".rs"));
    }
}
