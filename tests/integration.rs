use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Run the binary with the given subcommand, optionally piping JSON to stdin.
/// Returns `(success, stdout, stderr, exit_code)`. An empty `HOME` override
/// can be supplied to sandbox hooks that touch `~/.claude`.
fn run_with_home(
    subcmd: &str,
    stdin_json: Option<&str>,
    home: Option<&Path>,
) -> (bool, String, String, Option<i32>) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudey"));
    if !subcmd.is_empty() {
        cmd.arg(subcmd);
    }
    if let Some(h) = home {
        cmd.env("HOME", h);
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn binary");
    if let Some(json) = stdin_json {
        let mut sin = child.stdin.take().expect("stdin");
        sin.write_all(json.as_bytes()).expect("write stdin");
    }
    let out = child.wait_with_output().expect("wait");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.code(),
    )
}

fn run(subcmd: &str, stdin_json: Option<&str>) -> (bool, String, String, Option<i32>) {
    run_with_home(subcmd, stdin_json, None)
}

/// Assert a subcommand routes cleanly: exit success AND stderr does not
/// carry the "Unknown subcommand" dispatch failure.
fn assert_routes(subcmd: &str, stdin_json: Option<&str>) {
    // Give each hook a fresh sandbox HOME so file I/O stays isolated.
    let tmp = std::env::temp_dir().join(format!(
        "claudey-it-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&tmp).unwrap();
    let (ok, _, stderr, _) = run_with_home(subcmd, stdin_json, Some(&tmp));
    let _ = std::fs::remove_dir_all(&tmp);

    assert!(
        !stderr.contains("Unknown subcommand"),
        "subcommand {subcmd:?} failed to route: {stderr}"
    );
    assert!(ok, "subcommand {subcmd:?} exited non-zero: {stderr}");
}

#[test]
fn prints_usage_when_no_subcommand() {
    let (ok, _, stderr, _) = run("", None);
    assert!(!ok);
    assert!(stderr.contains("Usage: claudey <subcommand>"));
}

#[test]
fn unknown_subcommand_fails_with_message() {
    let (ok, _, stderr, _) = run("definitely-not-a-hook", None);
    assert!(!ok);
    assert!(stderr.contains("Unknown subcommand"));
}

// ── routing tests: each in-scope subcommand routes cleanly ────────────────

#[test]
fn routes_session_start() {
    assert_routes("session-start", None);
}

#[test]
fn routes_session_end() {
    assert_routes("session-end", Some("{}"));
}

#[test]
fn routes_pre_compact() {
    assert_routes("pre-compact", None);
}

#[test]
fn routes_suggest_compact() {
    assert_routes("suggest-compact", None);
}

#[test]
fn routes_post_edit_clippy() {
    assert_routes(
        "post-edit-clippy",
        Some(r#"{"tool_input":{"file_path":"a.rs"}}"#),
    );
}

#[test]
fn routes_post_edit_rustfmt() {
    assert_routes(
        "post-edit-rustfmt",
        Some(r#"{"tool_input":{"file_path":"a.rs"}}"#),
    );
}

#[test]
fn routes_evaluate_session() {
    assert_routes("evaluate-session", Some("{}"));
}

#[test]
fn routes_git_push_reminder() {
    assert_routes(
        "git-push-reminder",
        Some(r#"{"tool_input":{"command":"git push"}}"#),
    );
}

#[test]
fn routes_block_random_docs() {
    let tmp = std::env::temp_dir().join(format!("claudey-it-brd-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let (_, _, stderr, code) = run_with_home(
        "block-random-docs",
        Some(r#"{"tool_input":{"file_path":"README.md"}}"#),
        Some(&tmp),
    );
    let _ = std::fs::remove_dir_all(&tmp);
    // README.md is in the allow-list, so the hook exits 0.
    assert!(!stderr.contains("Unknown subcommand"));
    assert_eq!(code, Some(0));
}

#[test]
fn block_random_docs_blocks_disallowed_md() {
    let tmp = std::env::temp_dir().join(format!("claudey-it-brd2-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let (_, _, stderr, code) = run_with_home(
        "block-random-docs",
        Some(r#"{"tool_input":{"file_path":"random-notes.md"}}"#),
        Some(&tmp),
    );
    let _ = std::fs::remove_dir_all(&tmp);
    assert_eq!(code, Some(2), "expected block exit-code 2, got {code:?}");
    assert!(
        stderr.contains("BLOCKED"),
        "expected block message on stderr: {stderr}"
    );
}

#[test]
fn routes_pr_created_log() {
    assert_routes(
        "pr-created-log",
        Some(r#"{"tool_input":{"command":"gh pr create"}}"#),
    );
}

// ── session-start behavioral check ────────────────────────────────────────

#[test]
fn pre_compact_writes_compaction_log() {
    let tmp = std::env::temp_dir().join(format!(
        "claudey-it-pc-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&tmp).unwrap();
    let (ok, _, _, _) = run_with_home("pre-compact", None, Some(&tmp));
    assert!(ok);
    let log = tmp.join(".claude/sessions/compaction-log.txt");
    let content = std::fs::read_to_string(&log).expect("compaction-log.txt should exist");
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(
        content.contains("Context compaction triggered"),
        "log missing expected text: {content}"
    );
}

#[test]
fn session_start_surfaces_previous_session_summary() {
    let tmp = std::env::temp_dir().join(format!(
        "claudey-it-ss-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let sessions = tmp.join(".claude").join("sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    let f = sessions.join("2026-04-18-abc12345-session.tmp");
    std::fs::write(&f, "# Session 2026-04-18\n\nSome real summary content.\n").unwrap();

    let (ok, stdout, stderr, _) = run_with_home("session-start", None, Some(&tmp));
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(ok);
    assert!(
        stderr.contains("[SessionStart] Found 1 recent session(s)"),
        "expected discovery log in stderr: {stderr}"
    );
    assert!(
        stdout.contains("Previous session summary"),
        "expected summary on stdout: {stdout}"
    );
}
