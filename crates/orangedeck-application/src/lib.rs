//! OrangeDeck use-case helpers shared by the headless agent and native UI.

mod backoff;
mod deck;
mod mapping;
mod security;
mod store;

pub use backoff::ReconnectBackoff;
pub use deck::*;
pub use mapping::*;
pub use security::{CommandValidationError, ValidatedCommand, validate_command};
pub use store::SharedDashboard;
