//! Plutus core: storage and business logic, independent of the GUI.
//!
//! Everything here must compile and test without Tauri/GTK so it can be
//! verified on any host with just a Rust toolchain.

pub mod accounts;
pub mod backup;
pub mod budgets;
pub mod categories;
pub mod db;
pub mod export;
pub mod import;
pub mod money;
pub mod reports;
pub mod rules;
pub mod transactions;
pub mod transfers;
pub mod vault;

pub use accounts::{Account, AccountInput, AccountKind};
pub use backup::BackupFile;
pub use categories::{Category, CategoryGroup, CategoryInput, GroupInput, GroupKind};
pub use db::{Database, DbError};
pub use export::ExportedFile;
pub use money::{Money, MoneyError};
pub use transactions::{LedgerRow, Transaction, TransactionInput, TransactionQuery};
pub use transfers::{Transfer, TransferInput};
pub use vault::{MIN_PASSWORD_CHARS, Vault, VaultStatus};
