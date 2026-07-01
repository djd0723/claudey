//! hooks — subcommand implementations dispatched by `main.rs`.
//!
//! Every public fn here mirrors a Go hook from `internal/hooks/*.go`.

#![allow(dead_code)]

mod evaluate_session;
mod inline;
mod post_edit_clippy;
mod post_edit_rustfmt;
mod pre_compact;
mod session_end;
mod session_start;
mod suggest_compact;
pub use evaluate_session::evaluate_session;
pub use inline::{block_random_docs, git_push_reminder, pr_created_log};
pub use post_edit_clippy::post_edit_clippy;
pub use post_edit_rustfmt::post_edit_rustfmt;
pub use pre_compact::pre_compact;
pub use session_end::session_end;
pub use session_start::session_start;
pub use suggest_compact::suggest_compact;
