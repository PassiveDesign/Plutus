//! Plutus core: storage and business logic, independent of the GUI.
//!
//! Everything here must compile and test without Tauri/GTK so it can be
//! verified on any host with just a Rust toolchain.

pub mod accounts;
pub mod categories;
pub mod db;
pub mod money;
pub mod transactions;
pub mod transfers;
pub mod vault;

pub use accounts::{Account, AccountInput, AccountKind};
pub use categories::{Category, CategoryGroup, CategoryInput, GroupInput, GroupKind};
pub use db::{Database, DbError};
pub use money::{Money, MoneyError};
pub use transactions::{LedgerRow, Transaction, TransactionInput, TransactionQuery};
pub use transfers::{Transfer, TransferInput};
pub use vault::{MIN_PASSWORD_CHARS, Vault, VaultStatus};
