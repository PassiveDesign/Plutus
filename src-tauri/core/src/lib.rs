//! Plutus core: storage and business logic, independent of the GUI.
//!
//! Everything here must compile and test without Tauri/GTK so it can be
//! verified on any host with just a Rust toolchain.

pub mod db;

pub use db::{Database, DbError};
