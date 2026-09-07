//! OS and process adapters. No OrangeDeck network or GUI types live in this crate.

pub mod codex;
pub mod config;
pub mod git;
pub mod job;
pub mod system;
pub mod ui_preferences;

pub use codex::*;
pub use config::*;
pub use git::*;
pub use job::*;
pub use system::*;
pub use ui_preferences::*;
