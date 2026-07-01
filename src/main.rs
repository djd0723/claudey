mod aliases;
mod datetime;
mod fileutil;
mod gitutil;
mod hashcache;
mod hookio;
mod hooks;
mod platform;
mod sysutil;
#[cfg(test)]
mod testutil;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

const STDIN_TIMEOUT: Duration = Duration::from_secs(5);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        print_usage();
        return ExitCode::from(1);
    }

    let subcmd = args[1].as_str();
    match subcmd {
        "session-start" => hooks::session_start(),

        "session-end" => {
            let (input, _) = hookio::read_stdin_json(STDIN_TIMEOUT, hookio::DEFAULT_MAX_SIZE);
            hooks::session_end(input);
        }

        "pre-compact" => hooks::pre_compact(),

        "suggest-compact" => hooks::suggest_compact(),

        "post-edit-clippy" => {
            let (input, raw) = hookio::read_stdin_json(STDIN_TIMEOUT, hookio::DEFAULT_MAX_SIZE);
            hooks::post_edit_clippy(input, raw);
        }

        "post-edit-rustfmt" => {
            let (input, raw) = hookio::read_stdin_json(STDIN_TIMEOUT, hookio::DEFAULT_MAX_SIZE);
            hooks::post_edit_rustfmt(input, raw);
        }

        "evaluate-session" => {
            let (input, _) = hookio::read_stdin_json(STDIN_TIMEOUT, hookio::DEFAULT_MAX_SIZE);
            let root = find_plugin_root();
            hooks::evaluate_session(input, root);
        }

        "git-push-reminder" => {
            let (input, raw) = hookio::read_stdin_json(STDIN_TIMEOUT, hookio::DEFAULT_MAX_SIZE);
            hooks::git_push_reminder(input, raw);
        }

        "block-random-docs" => {
            let (input, raw) = hookio::read_stdin_json(STDIN_TIMEOUT, hookio::DEFAULT_MAX_SIZE);
            let code = hooks::block_random_docs(input, raw);
            return ExitCode::from(code.clamp(0, 255) as u8);
        }

        "pr-created-log" => {
            let (input, raw) = hookio::read_stdin_json(STDIN_TIMEOUT, hookio::DEFAULT_MAX_SIZE);
            hooks::pr_created_log(input, raw);
        }

        other => {
            eprintln!("Unknown subcommand: {other}");
            return ExitCode::from(1);
        }
    }

    ExitCode::SUCCESS
}

/// Discover the plugin root directory. Honours `CLAUDE_PLUGIN_ROOT` first,
/// then walks up from the binary location, then from cwd (bounded at 10 levels).
fn find_plugin_root() -> String {
    if let Ok(v) = std::env::var("CLAUDE_PLUGIN_ROOT") {
        if !v.is_empty() {
            return v;
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        // Binary is at <root>/bin/claudey
        if let Some(bin) = exe.parent() {
            if let Some(root) = bin.parent() {
                if root.join("hooks").join("hooks.json").exists() {
                    return root.display().to_string();
                }
            }
        }
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut dir = cwd.clone();
    for _ in 0..10 {
        if dir.join("hooks").join("hooks.json").exists() {
            return dir.display().to_string();
        }
        match dir.parent() {
            Some(p) if p != dir => dir = p.to_path_buf(),
            _ => break,
        }
    }
    cwd.display().to_string()
}

fn print_usage() {
    eprintln!("Usage: claudey <subcommand>");
    eprintln!();
    eprintln!("Hook subcommands:");
    eprintln!("  session-start          Load previous context on new session");
    eprintln!("  session-end            Persist session state on end");
    eprintln!("  pre-compact            Save state before compaction");
    eprintln!("  suggest-compact        Suggest manual compaction at intervals");
    eprintln!("  post-edit-clippy       Run clippy after .rs edits");
    eprintln!("  post-edit-rustfmt      Format .rs files with rustfmt");
    eprintln!("  evaluate-session       Evaluate session for patterns");
    eprintln!();
    eprintln!("Inline hook subcommands:");
    eprintln!("  git-push-reminder      Reminder before git push");
    eprintln!("  block-random-docs      Block random .md/.txt file creation");
    eprintln!("  pr-created-log         Log PR URL after creation");
}
