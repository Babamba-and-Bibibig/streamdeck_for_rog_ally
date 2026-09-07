//! Pure OrangeDeck domain types. This crate deliberately has no UI, network, or process runtime.

pub mod codex;
pub mod deck;
pub mod git;
pub mod host;
pub mod job;
pub mod project;
pub mod state;

pub use codex::*;
pub use deck::*;
pub use git::*;
pub use host::*;
pub use job::*;
pub use project::*;
pub use state::*;
