//! Tauri shell: app state and commands only. Logic lives in `plutus-core`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use plutus_core::{
    Account, AccountInput, Database, DbError, MIN_PASSWORD_CHARS, Money, MoneyError, Transaction,
    TransactionInput, TransactionQuery, Vault, accounts, transactions,
};
use serde::Serialize;
use tauri::{Manager, State};

/// Shared app state: the vault guarding the encrypted database.
struct AppState {
    vault: Arc<Mutex<Vault>>,
}

/// Error shape returned to the UI: a stable `code`, a readable message, and
/// for validation errors the form `field` the message belongs to.
#[derive(Debug, Serialize)]
struct CommandError {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<&'static str>,
}

impl From<DbError> for CommandError {
    fn from(e: DbError) -> Self {
        let field = match &e {
            DbError::Invalid { field, .. } | DbError::Conflict { field, .. } => Some(*field),
            _ => None,
        };
        Self {
            code: e.code(),
            message: e.to_string(),
            field,
        }
    }
}

impl From<MoneyError> for CommandError {
    fn from(e: MoneyError) -> Self {
        Self {
            code: "invalid_amount",
            message: e.friendly().to_string(),
            field: None,
        }
    }
}

impl From<tauri::Error> for CommandError {
    fn from(e: tauri::Error) -> Self {
        Self {
            code: "internal",
            message: e.to_string(),
            field: None,
        }
    }
}

type CmdResult<T> = Result<T, CommandError>;

/// Lock the vault mutex, recovering from poisoning (a panic in another
/// command must not brick the app).
fn guard(vault: &Mutex<Vault>) -> MutexGuard<'_, Vault> {
    vault.lock().unwrap_or_else(|e| e.into_inner())
}

/// Run `f` against the unlocked database on a blocking thread, so the main
/// (UI event) thread never waits on the vault lock or disk I/O.
/// Returns `locked` if the vault isn't unlocked.
async fn with_db<T, F>(vault: &Arc<Mutex<Vault>>, f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce(&Database) -> Result<T, DbError> + Send + 'static,
{
    let vault = Arc::clone(vault);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let v = guard(&vault);
        f(v.db()?)
    })
    .await?;
    Ok(result?)
}

#[derive(Serialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
    os: &'static str,
    min_password_chars: usize,
}

/// Basic app metadata for the UI.
#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "Plutus",
        version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        min_password_chars: MIN_PASSWORD_CHARS,
    }
}

/// "new" (first run), "locked" or "unlocked".
#[tauri::command]
fn vault_status(state: State<'_, AppState>) -> &'static str {
    guard(&state.vault).status().as_str()
}

/// First run: create the encrypted database with `password`.
/// Runs off the main thread because key derivation is deliberately slow.
#[tauri::command]
async fn create_vault(state: State<'_, AppState>, password: String) -> CmdResult<()> {
    let vault = Arc::clone(&state.vault);
    tauri::async_runtime::spawn_blocking(move || guard(&vault).create(&password)).await??;
    Ok(())
}

/// Unlock the existing database with `password`.
#[tauri::command]
async fn unlock_vault(state: State<'_, AppState>, password: String) -> CmdResult<()> {
    let vault = Arc::clone(&state.vault);
    tauri::async_runtime::spawn_blocking(move || guard(&vault).unlock(&password)).await??;
    Ok(())
}

/// Re-encrypt the database under `new_password` after re-checking `current`.
#[tauri::command]
async fn change_password(
    state: State<'_, AppState>,
    current: String,
    new_password: String,
) -> CmdResult<()> {
    let vault = Arc::clone(&state.vault);
    tauri::async_runtime::spawn_blocking(move || {
        guard(&vault).change_password(&current, &new_password)
    })
    .await??;
    Ok(())
}

/// Parse a typed amount (`$1,234.56`, `(12.30)`, `-5`) into cents using the
/// same strict parser as everything else. The UI never parses money itself.
#[tauri::command]
fn parse_money(text: String) -> CmdResult<i64> {
    Ok(Money::parse(&text)?.cents())
}

/// All accounts in display order (active first).
#[tauri::command]
async fn list_accounts(
    state: State<'_, AppState>,
    include_archived: bool,
) -> CmdResult<Vec<Account>> {
    with_db(&state.vault, move |db| accounts::list(db, include_archived)).await
}

#[tauri::command]
async fn create_account(state: State<'_, AppState>, input: AccountInput) -> CmdResult<Account> {
    with_db(&state.vault, move |db| accounts::create(db, &input)).await
}

#[tauri::command]
async fn update_account(
    state: State<'_, AppState>,
    id: i64,
    input: AccountInput,
) -> CmdResult<Account> {
    with_db(&state.vault, move |db| accounts::update(db, id, &input)).await
}

#[tauri::command]
async fn set_account_archived(
    state: State<'_, AppState>,
    id: i64,
    archived: bool,
) -> CmdResult<Account> {
    with_db(&state.vault, move |db| {
        accounts::set_archived(db, id, archived)
    })
    .await
}

/// Current balance of every account (opening + transactions on/after the
/// opening date), keyed by account id. JSON keys arrive as strings.
#[tauri::command]
async fn account_balances(state: State<'_, AppState>) -> CmdResult<HashMap<i64, Money>> {
    with_db(&state.vault, accounts::balances).await
}

/// One page of transactions, newest first.
#[tauri::command]
async fn list_transactions(
    state: State<'_, AppState>,
    query: TransactionQuery,
) -> CmdResult<Vec<Transaction>> {
    with_db(&state.vault, move |db| transactions::list(db, &query)).await
}

/// Total matching `query` (ignores paging), for page counts.
#[tauri::command]
async fn count_transactions(state: State<'_, AppState>, query: TransactionQuery) -> CmdResult<u64> {
    with_db(&state.vault, move |db| transactions::count(db, &query)).await
}

#[tauri::command]
async fn add_transaction(
    state: State<'_, AppState>,
    input: TransactionInput,
) -> CmdResult<Transaction> {
    with_db(&state.vault, move |db| transactions::add(db, &input)).await
}

#[tauri::command]
async fn update_transaction(
    state: State<'_, AppState>,
    id: i64,
    input: TransactionInput,
) -> CmdResult<Transaction> {
    with_db(&state.vault, move |db| transactions::update(db, id, &input)).await
}

#[tauri::command]
async fn delete_transaction(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    with_db(&state.vault, move |db| transactions::delete(db, id)).await
}

/// Close the database; the UI returns to the unlock screen.
#[tauri::command]
fn lock_vault(state: State<'_, AppState>) {
    guard(&state.vault).lock();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let vault = Vault::new(Database::path_in(&data_dir));
            app.manage(AppState {
                vault: Arc::new(Mutex::new(vault)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            vault_status,
            create_vault,
            unlock_vault,
            lock_vault,
            change_password,
            parse_money,
            list_accounts,
            create_account,
            update_account,
            set_account_archived,
            account_balances,
            list_transactions,
            count_transactions,
            add_transaction,
            update_transaction,
            delete_transaction
        ])
        .run(tauri::generate_context!())
        .expect("error while running Plutus");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_reports_crate_version() {
        let info = app_info();
        assert_eq!(info.name, "Plutus");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.min_password_chars, MIN_PASSWORD_CHARS);
    }

    #[test]
    fn db_errors_map_to_stable_codes() {
        let e: CommandError = DbError::WrongPassword.into();
        assert_eq!(e.code, "wrong_password");
        assert_eq!(e.message, "incorrect password");
        assert_eq!(e.field, None);
        let json = serde_json::to_value(&e).unwrap();
        assert!(json.get("field").is_none(), "field omitted when absent");
    }

    #[test]
    fn validation_errors_carry_their_field() {
        let e: CommandError = DbError::Invalid {
            field: "bsb",
            message: "BSB must be 6 digits, e.g. 063-000.".into(),
        }
        .into();
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["code"], "invalid");
        assert_eq!(json["field"], "bsb");
        assert_eq!(json["message"], "BSB must be 6 digits, e.g. 063-000.");
    }

    #[test]
    fn parse_money_uses_the_core_parser() {
        assert_eq!(parse_money("$1,234.56".into()).unwrap(), 123_456);
        assert_eq!(parse_money("(12.30)".into()).unwrap(), -1230);
        let e = parse_money("1.234".into()).unwrap_err();
        assert_eq!(e.code, "invalid_amount");
        assert_eq!(e.message, "Amounts can have at most 2 decimal places.");
    }

    fn input(name: &str) -> AccountInput {
        serde_json::from_value(serde_json::json!({
            "name": name, "kind": "offset",
            "opening_balance": 12345, "opening_date": "2026-11-01"
        }))
        .unwrap()
    }

    #[test]
    fn with_db_requires_unlock_then_runs_account_operations() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Mutex::new(Vault::new(Database::path_in(dir.path()))));
        tauri::async_runtime::block_on(async {
            let err = with_db(&vault, |db| accounts::list(db, false))
                .await
                .unwrap_err();
            assert_eq!(err.code, "locked");

            guard(&vault).create("long enough pw").unwrap();
            let created = with_db(&vault, |db| accounts::create(db, &input("Everyday")))
                .await
                .unwrap();
            assert_eq!(created.opening_balance.cents(), 12345);

            let dup = with_db(&vault, |db| accounts::create(db, &input("everyday")))
                .await
                .unwrap_err();
            assert_eq!((dup.code, dup.field), ("conflict", Some("name")));

            let listed = with_db(&vault, |db| accounts::list(db, false))
                .await
                .unwrap();
            assert_eq!(listed.len(), 1);
        });
    }
}
