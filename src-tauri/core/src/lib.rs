//! Plutus core: storage and business logic, independent of the GUI.
//!
//! Everything here must compile and test without Tauri/GTK so it can be
//! verified on any host with just a Rust toolchain.

pub mod db;
pub mod vault;

pub use db::{Database, DbError};
pub use vault::{MIN_PASSWORD_CHARS, Vault, VaultStatus};
