//! Tauri shell: app state and commands only. Logic lives in `plutus-core`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use plutus_core::{
    Account, AccountInput, BackupFile, Category, CategoryGroup, CategoryInput, Database, DbError,
    ExportedFile, GroupInput, LedgerRow, MIN_PASSWORD_CHARS, Money, MoneyError, Transaction,
    TransactionInput, TransactionQuery, Transfer, TransferInput, Vault, accounts, backup, budgets,
    categories, export, import, reports, rules, transactions, transfers,
};
use serde::Serialize;
use tauri::{Manager, State};

/// Shared app state: the vault guarding the encrypted database.
struct AppState {
    vault: Arc<Mutex<Vault>>,
    /// App data directory; backups live in `backups/` inside it.
    data_dir: std::path::PathBuf,
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

/// Run `f` against the vault itself (locked or not) on a blocking thread.
/// Used by backup/restore, which close and reopen the database file.
async fn with_vault<T, F>(vault: &Arc<Mutex<Vault>>, f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce(&mut Vault) -> Result<T, DbError> + Send + 'static,
{
    let vault = Arc::clone(vault);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut v = guard(&vault);
        f(&mut v)
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

/// One page of transactions, newest first, with running balances when the
/// query is for a single account.
#[tauri::command]
async fn list_transactions(
    state: State<'_, AppState>,
    query: TransactionQuery,
) -> CmdResult<Vec<LedgerRow>> {
    with_db(&state.vault, move |db| {
        transactions::list_with_running_balance(db, &query)
    })
    .await
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

/// Set or clear (`categoryId: null`) the category of many transactions.
/// Transfer legs are skipped. Returns how many changed.
#[tauri::command]
async fn set_transaction_category(
    state: State<'_, AppState>,
    ids: Vec<i64>,
    category_id: Option<i64>,
) -> CmdResult<usize> {
    with_db(&state.vault, move |db| {
        transactions::set_category(db, &ids, category_id)
    })
    .await
}

/// Move money between two accounts (creates both legs).
#[tauri::command]
async fn create_transfer(state: State<'_, AppState>, input: TransferInput) -> CmdResult<Transfer> {
    with_db(&state.vault, move |db| transfers::create(db, &input)).await
}

/// Change a transfer's accounts, date, amount or memo (both legs).
#[tauri::command]
async fn update_transfer(
    state: State<'_, AppState>,
    id: i64,
    input: TransferInput,
) -> CmdResult<Transfer> {
    with_db(&state.vault, move |db| transfers::update(db, id, &input)).await
}

/// The category tree (income groups first).
#[tauri::command]
async fn list_categories(
    state: State<'_, AppState>,
    include_archived: bool,
) -> CmdResult<Vec<CategoryGroup>> {
    with_db(&state.vault, move |db| {
        categories::tree(db, include_archived)
    })
    .await
}

/// Transactions per category id (unused categories are absent).
#[tauri::command]
async fn category_usage(state: State<'_, AppState>) -> CmdResult<HashMap<i64, i64>> {
    with_db(&state.vault, categories::usage).await
}

#[tauri::command]
async fn create_category_group(
    state: State<'_, AppState>,
    input: GroupInput,
) -> CmdResult<CategoryGroup> {
    with_db(&state.vault, move |db| categories::create_group(db, &input)).await
}

#[tauri::command]
async fn update_category_group(
    state: State<'_, AppState>,
    id: i64,
    input: GroupInput,
) -> CmdResult<CategoryGroup> {
    with_db(&state.vault, move |db| {
        categories::update_group(db, id, &input)
    })
    .await
}

#[tauri::command]
async fn set_category_group_archived(
    state: State<'_, AppState>,
    id: i64,
    archived: bool,
) -> CmdResult<CategoryGroup> {
    with_db(&state.vault, move |db| {
        categories::set_group_archived(db, id, archived)
    })
    .await
}

#[tauri::command]
async fn create_category(state: State<'_, AppState>, input: CategoryInput) -> CmdResult<Category> {
    with_db(&state.vault, move |db| categories::create(db, &input)).await
}

#[tauri::command]
async fn update_category(
    state: State<'_, AppState>,
    id: i64,
    input: CategoryInput,
) -> CmdResult<Category> {
    with_db(&state.vault, move |db| categories::update(db, id, &input)).await
}

#[tauri::command]
async fn set_category_archived(
    state: State<'_, AppState>,
    id: i64,
    archived: bool,
) -> CmdResult<Category> {
    with_db(&state.vault, move |db| {
        categories::set_archived(db, id, archived)
    })
    .await
}

/// Read a Bank of Melbourne CSV export (as text) and show what importing it
/// into `account_id` would do. Writes nothing.
#[tauri::command]
async fn preview_bom_import(
    state: State<'_, AppState>,
    account_id: i64,
    text: String,
) -> CmdResult<import::Preview> {
    with_db(&state.vault, move |db| {
        let rows = import::bom::parse(&text)?;
        import::preview(db, account_id, rows)
    })
    .await
}

/// Import the rows the owner kept from a preview (one SQL transaction).
#[tauri::command]
async fn commit_import(
    state: State<'_, AppState>,
    account_id: i64,
    rows: Vec<import::ImportRow>,
) -> CmdResult<import::CommitResult> {
    with_db(&state.vault, move |db| {
        import::commit(db, account_id, &rows)
    })
    .await
}

/// The header row and a guessed column mapping for a generic CSV file.
/// Writes nothing, so it needs no database.
#[tauri::command]
fn import_shape(text: String) -> CmdResult<import::generic::FileShape> {
    Ok(import::generic::shape(&text)?)
}

/// Read a generic CSV file with the owner's column mapping and show what
/// importing it into `account_id` would do. Writes nothing.
#[tauri::command]
async fn preview_generic_import(
    state: State<'_, AppState>,
    account_id: i64,
    text: String,
    mapping: import::generic::Mapping,
) -> CmdResult<import::Preview> {
    with_db(&state.vault, move |db| {
        let rows = import::generic::parse(&text, &mapping)?;
        import::preview(db, account_id, rows)
    })
    .await
}

/// Saved column mappings, alphabetically.
#[tauri::command]
async fn list_import_presets(
    state: State<'_, AppState>,
) -> CmdResult<Vec<import::presets::Preset>> {
    with_db(&state.vault, import::presets::list).await
}

/// Save a mapping under a name; an existing name is replaced.
#[tauri::command]
async fn save_import_preset(
    state: State<'_, AppState>,
    name: String,
    mapping: import::generic::Mapping,
) -> CmdResult<import::presets::Preset> {
    with_db(&state.vault, move |db| {
        import::presets::save(db, &name, &mapping)
    })
    .await
}

#[tauri::command]
async fn delete_import_preset(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    with_db(&state.vault, move |db| import::presets::delete(db, id)).await
}

/// Auto-categorisation rules in the order they're tried.
#[tauri::command]
async fn list_rules(state: State<'_, AppState>) -> CmdResult<Vec<rules::Rule>> {
    with_db(&state.vault, rules::list).await
}

#[tauri::command]
async fn create_rule(
    state: State<'_, AppState>,
    input: rules::RuleInput,
) -> CmdResult<rules::Rule> {
    with_db(&state.vault, move |db| rules::create(db, &input)).await
}

#[tauri::command]
async fn update_rule(
    state: State<'_, AppState>,
    id: i64,
    input: rules::RuleInput,
) -> CmdResult<rules::Rule> {
    with_db(&state.vault, move |db| rules::update(db, id, &input)).await
}

#[tauri::command]
async fn delete_rule(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    with_db(&state.vault, move |db| rules::delete(db, id)).await
}

/// Move a rule one place earlier (`up`) or later; returns the new order.
#[tauri::command]
async fn move_rule(state: State<'_, AppState>, id: i64, up: bool) -> CmdResult<Vec<rules::Rule>> {
    with_db(&state.vault, move |db| rules::move_rule(db, id, up)).await
}

/// Categorise uncategorised transactions that a rule matches; returns the count.
#[tauri::command]
async fn apply_rules(state: State<'_, AppState>) -> CmdResult<usize> {
    with_db(&state.vault, |db| rules::apply(db, None)).await
}

/// Budgeted vs spent for every expense category in a month (`YYYY-MM`).
#[tauri::command]
async fn budget_month(
    state: State<'_, AppState>,
    month: budgets::Month,
) -> CmdResult<budgets::MonthBudget> {
    with_db(&state.vault, move |db| budgets::month(db, month)).await
}

/// Set a category's monthly budget from `month` onwards (0 stops it).
#[tauri::command]
async fn set_budget(
    state: State<'_, AppState>,
    category_id: i64,
    month: budgets::Month,
    amount: Money,
) -> CmdResult<()> {
    with_db(&state.vault, move |db| {
        budgets::set(db, category_id, month, amount)
    })
    .await
}

/// Turn budget rollover on or off for a category.
#[tauri::command]
async fn set_budget_rollover(
    state: State<'_, AppState>,
    category_id: i64,
    on: bool,
) -> CmdResult<()> {
    with_db(&state.vault, move |db| {
        budgets::set_rollover(db, category_id, on)
    })
    .await
}

/// Undo the budget change made in exactly `month`.
#[tauri::command]
async fn clear_budget(
    state: State<'_, AppState>,
    category_id: i64,
    month: budgets::Month,
) -> CmdResult<()> {
    with_db(&state.vault, move |db| {
        budgets::clear(db, category_id, month)
    })
    .await
}

/// Everything the dashboard shows for a month.
#[tauri::command]
async fn dashboard(
    state: State<'_, AppState>,
    month: budgets::Month,
) -> CmdResult<reports::Dashboard> {
    with_db(&state.vault, move |db| reports::dashboard(db, month)).await
}

/// Month-by-month figures and spending by category for a range of months.
#[tauri::command]
async fn report(
    state: State<'_, AppState>,
    from: budgets::Month,
    to: budgets::Month,
) -> CmdResult<reports::Report> {
    with_db(&state.vault, move |db| reports::report(db, from, to)).await
}

/// Copy the database into the backups folder and return the new file.
#[tauri::command]
async fn backup_now(state: State<'_, AppState>) -> CmdResult<BackupFile> {
    let dir = state.data_dir.clone();
    with_vault(&state.vault, move |v| backup::backup_now(v, &dir)).await
}

/// Every backup on disk, newest first.
#[tauri::command]
async fn list_backups(state: State<'_, AppState>) -> CmdResult<Vec<BackupFile>> {
    let dir = state.data_dir.clone();
    let files = tauri::async_runtime::spawn_blocking(move || backup::list(&dir)).await?;
    Ok(files?)
}

/// Where backups are kept, for display in Settings.
#[tauri::command]
fn backup_folder(state: State<'_, AppState>) -> String {
    backup::dir_in(&state.data_dir).display().to_string()
}

/// Replace the database with the named backup. The app stays unlocked.
#[tauri::command]
async fn restore_backup(state: State<'_, AppState>, name: String) -> CmdResult<()> {
    let dir = state.data_dir.clone();
    with_vault(&state.vault, move |v| backup::restore(v, &dir, &name)).await
}

/// Write the transactions matching `query` to a CSV file in the exports folder.
#[tauri::command]
async fn export_transactions(
    state: State<'_, AppState>,
    query: TransactionQuery,
) -> CmdResult<ExportedFile> {
    let dir = state.data_dir.clone();
    with_db(&state.vault, move |db| export::export_now(db, &dir, &query)).await
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
                data_dir,
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
            delete_transaction,
            set_transaction_category,
            create_transfer,
            update_transfer,
            list_categories,
            category_usage,
            create_category_group,
            update_category_group,
            set_category_group_archived,
            create_category,
            update_category,
            set_category_archived,
            preview_bom_import,
            import_shape,
            preview_generic_import,
            list_import_presets,
            save_import_preset,
            delete_import_preset,
            commit_import,
            list_rules,
            create_rule,
            update_rule,
            delete_rule,
            move_rule,
            apply_rules,
            budget_month,
            set_budget,
            clear_budget,
            set_budget_rollover,
            dashboard,
            report,
            export_transactions,
            backup_now,
            list_backups,
            backup_folder,
            restore_backup
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

    #[test]
    fn transaction_commands_round_trip_and_balances_serialise_by_id() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Mutex::new(Vault::new(Database::path_in(dir.path()))));
        guard(&vault).create("long enough pw").unwrap();
        tauri::async_runtime::block_on(async {
            let acct = with_db(&vault, |db| accounts::create(db, &input("Everyday")))
                .await
                .unwrap();
            let tx: TransactionInput = serde_json::from_value(serde_json::json!({
                "account_id": acct.id, "date": "2026-11-15", "amount": -845, "payee": "Coffee"
            }))
            .unwrap();
            let added = with_db(&vault, move |db| transactions::add(db, &tx))
                .await
                .unwrap();

            let one = TransactionQuery {
                account_id: Some(acct.id),
                ..Default::default()
            };
            let rows = with_db(&vault, move |db| {
                transactions::list_with_running_balance(db, &one)
            })
            .await
            .unwrap();
            assert_eq!(rows[0].running_balance.map(Money::cents), Some(12345 - 845));

            let n = with_db(&vault, |db| {
                transactions::count(db, &TransactionQuery::default())
            })
            .await
            .unwrap();
            assert_eq!(n, 1);

            let balances = with_db(&vault, accounts::balances).await.unwrap();
            let json = serde_json::to_value(&balances).unwrap();
            assert_eq!(
                json[acct.id.to_string()],
                12345 - 845,
                "keys are id strings, values cents"
            );

            let bad: TransactionInput = serde_json::from_value(serde_json::json!({
                "account_id": acct.id, "date": "2026-11-15", "amount": 1
            }))
            .unwrap();
            let e = with_db(&vault, move |db| transactions::add(db, &bad))
                .await
                .unwrap_err();
            assert_eq!((e.code, e.field), ("invalid", Some("payee")));

            let id = added.id;
            with_db(&vault, move |db| transactions::delete(db, id))
                .await
                .unwrap();
            let e = with_db(&vault, move |db| transactions::delete(db, id))
                .await
                .unwrap_err();
            assert_eq!(e.code, "record_not_found");
        });
    }

    #[test]
    fn transfer_commands_create_update_and_report_field_errors() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Mutex::new(Vault::new(Database::path_in(dir.path()))));
        guard(&vault).create("long enough pw").unwrap();
        tauri::async_runtime::block_on(async {
            let a = with_db(&vault, |db| accounts::create(db, &input("Everyday")))
                .await
                .unwrap();
            let b = with_db(&vault, |db| accounts::create(db, &input("Bills")))
                .await
                .unwrap();
            let make = |from: i64, to: i64, cents: i64| -> TransferInput {
                serde_json::from_value(serde_json::json!({
                    "from_account": from, "to_account": to,
                    "date": "2026-11-15", "amount": cents
                }))
                .unwrap()
            };
            let t_in = make(a.id, b.id, 1000);
            let t = with_db(&vault, move |db| transfers::create(db, &t_in))
                .await
                .unwrap();
            let json = serde_json::to_value(&t).unwrap();
            assert_eq!(json["from"]["amount"], -1000);
            assert_eq!(json["to"]["transfer_account_id"], a.id);

            let id = t.id;
            let up = make(a.id, b.id, 2500);
            let u = with_db(&vault, move |db| transfers::update(db, id, &up))
                .await
                .unwrap();
            assert_eq!(u.to.amount.cents(), 2500);

            let same = make(a.id, a.id, 100);
            let e = with_db(&vault, move |db| transfers::create(db, &same))
                .await
                .unwrap_err();
            assert_eq!((e.code, e.field), ("invalid", Some("to_account")));
        });
    }

    #[test]
    fn category_commands_list_create_and_report_conflicts() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Mutex::new(Vault::new(Database::path_in(dir.path()))));
        guard(&vault).create("long enough pw").unwrap();
        tauri::async_runtime::block_on(async {
            let tree = with_db(&vault, |db| categories::tree(db, false))
                .await
                .unwrap();
            let json = serde_json::to_value(&tree).unwrap();
            assert_eq!(json[0]["kind"], "income");
            let food = tree.iter().find(|g| g.name == "Food").unwrap().id;

            let ok: CategoryInput =
                serde_json::from_value(serde_json::json!({"group_id": food, "name": "Snacks"}))
                    .unwrap();
            let c = with_db(&vault, move |db| categories::create(db, &ok))
                .await
                .unwrap();
            assert_eq!(c.group_id, food);

            let dup: CategoryInput =
                serde_json::from_value(serde_json::json!({"group_id": food, "name": "snacks"}))
                    .unwrap();
            let e = with_db(&vault, move |db| categories::create(db, &dup))
                .await
                .unwrap_err();
            assert_eq!(e.field, Some("name"));
            assert_ne!(e.code, "invalid", "a clash is a conflict, not bad input");
        });
    }

    #[test]
    fn import_commands_preview_then_commit() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Mutex::new(Vault::new(Database::path_in(dir.path()))));
        guard(&vault).create("long enough pw").unwrap();
        tauri::async_runtime::block_on(async {
            let a = with_db(&vault, |db| accounts::create(db, &input("Everyday")))
                .await
                .unwrap();
            let text = "Date,Description,Debit,Credit,Balance\r\n\
                        11/20/2026,Eftpos Debit    20Nov15:37  Gym Co  \\  Melbourne  Au,7.99,,1.00\r\n,,,,\r\n";
            let id = a.id;
            let p = with_db(&vault, move |db| {
                import::preview(db, id, import::bom::parse(text)?)
            })
            .await
            .unwrap();
            let json = serde_json::to_value(&p).unwrap();
            assert_eq!(json["rows"][0]["status"], "new");
            assert_eq!(json["rows"][0]["payee"], "Gym Co");
            let rows: Vec<import::ImportRow> =
                serde_json::from_value(serde_json::json!([json["rows"][0].clone()])).unwrap();
            let r = with_db(&vault, move |db| import::commit(db, id, &rows))
                .await
                .unwrap();
            assert_eq!(serde_json::to_value(r).unwrap()["imported"], 1);

            let e = with_db(&vault, |_db| import::bom::parse("nope").map(|_| ()))
                .await
                .unwrap_err();
            assert_eq!((e.code, e.field), ("invalid", Some("file")));
        });
    }

    #[test]
    fn set_transaction_category_command_counts_changes() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Mutex::new(Vault::new(Database::path_in(dir.path()))));
        guard(&vault).create("long enough pw").unwrap();
        tauri::async_runtime::block_on(async {
            let a = with_db(&vault, |db| accounts::create(db, &input("Everyday")))
                .await
                .unwrap();
            let t_in: TransactionInput = serde_json::from_value(serde_json::json!({
                "account_id": a.id, "date": "2026-11-05", "amount": -500, "payee": "Vet"
            }))
            .unwrap();
            let t = with_db(&vault, move |db| transactions::add(db, &t_in))
                .await
                .unwrap();
            assert!(serde_json::to_value(&t).unwrap()["category_id"].is_null());
            let pets = with_db(&vault, |db| categories::tree(db, false))
                .await
                .unwrap()
                .into_iter()
                .flat_map(|g| g.categories)
                .find(|c| c.name == "Pets")
                .unwrap()
                .id;
            let id = t.id;
            let n = with_db(&vault, move |db| {
                transactions::set_category(db, &[id], Some(pets))
            })
            .await
            .unwrap();
            assert_eq!(n, 1);
            let e = with_db(&vault, move |db| {
                transactions::set_category(db, &[id], Some(99_999))
            })
            .await
            .unwrap_err();
            assert_eq!(e.field, Some("category_id"));
        });
    }

    #[test]
    fn rule_commands_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Mutex::new(Vault::new(Database::path_in(dir.path()))));
        guard(&vault).create("long enough pw").unwrap();
        tauri::async_runtime::block_on(async {
            let fuel = with_db(&vault, |db| categories::tree(db, false))
                .await
                .unwrap()
                .into_iter()
                .flat_map(|g| g.categories)
                .find(|c| c.name == "Fuel")
                .unwrap()
                .id;
            let input: rules::RuleInput = serde_json::from_value(serde_json::json!({
                "pattern": "shell", "match_field": "payee", "direction": "out", "category_id": fuel
            }))
            .unwrap();
            let r = with_db(&vault, move |db| rules::create(db, &input))
                .await
                .unwrap();
            assert_eq!(serde_json::to_value(&r).unwrap()["direction"], "out");
            let n = with_db(&vault, |db| rules::apply(db, None)).await.unwrap();
            assert_eq!(n, 0);
            let bad: rules::RuleInput = serde_json::from_value(serde_json::json!({
                "pattern": " ", "match_field": "either", "category_id": fuel
            }))
            .unwrap();
            let e = with_db(&vault, move |db| rules::create(db, &bad))
                .await
                .unwrap_err();
            assert_eq!(e.field, Some("pattern"));
        });
    }

    #[test]
    fn budget_commands_set_and_report() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Mutex::new(Vault::new(Database::path_in(dir.path()))));
        guard(&vault).create("long enough pw").unwrap();
        tauri::async_runtime::block_on(async {
            let g = with_db(&vault, |db| categories::tree(db, false))
                .await
                .unwrap()
                .into_iter()
                .flat_map(|g| g.categories)
                .find(|c| c.name == "Groceries")
                .unwrap()
                .id;
            let month: budgets::Month =
                serde_json::from_value(serde_json::json!("2026-11")).unwrap();
            with_db(&vault, move |db| {
                budgets::set(db, g, month, Money::from_cents(50_000))
            })
            .await
            .unwrap();
            let b = with_db(&vault, move |db| budgets::month(db, month))
                .await
                .unwrap();
            let v = serde_json::to_value(&b).unwrap();
            assert_eq!(v["total_budgeted"], 50_000);
            let e = with_db(&vault, move |db| {
                budgets::set(db, g, month, Money::from_cents(-1))
            })
            .await
            .unwrap_err();
            assert_eq!(e.field, Some("amount"));
            assert!(
                serde_json::from_value::<budgets::Month>(serde_json::json!("2026-13")).is_err()
            );
        });
    }

    #[test]
    fn dashboard_command_serialises() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Mutex::new(Vault::new(Database::path_in(dir.path()))));
        guard(&vault).create("long enough pw").unwrap();
        tauri::async_runtime::block_on(async {
            with_db(&vault, |db| accounts::create(db, &input("Everyday")))
                .await
                .unwrap();
            let month: budgets::Month =
                serde_json::from_value(serde_json::json!("2026-11")).unwrap();
            let d = with_db(&vault, move |db| reports::dashboard(db, month))
                .await
                .unwrap();
            let v = serde_json::to_value(&d).unwrap();
            assert_eq!(v["net_worth"]["net"], 12345);
            assert!(v["recent"].as_array().unwrap().is_empty());
        });
    }

    #[test]
    fn set_budget_rollover_reports_missing_category() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Mutex::new(Vault::new(Database::path_in(dir.path()))));
        guard(&vault).create("long enough pw").unwrap();
        tauri::async_runtime::block_on(async {
            let e = with_db(&vault, |db| budgets::set_rollover(db, 99_999, true))
                .await
                .unwrap_err();
            assert_eq!(e.field, Some("category_id"));
        });
    }

    #[test]
    fn report_command_checks_the_range() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Mutex::new(Vault::new(Database::path_in(dir.path()))));
        guard(&vault).create("long enough pw").unwrap();
        tauri::async_runtime::block_on(async {
            let m = |s: &str| -> budgets::Month {
                serde_json::from_value(serde_json::json!(s)).unwrap()
            };
            let (a, b) = (m("2026-10"), m("2026-11"));
            let r = with_db(&vault, move |db| reports::report(db, a, b))
                .await
                .unwrap();
            assert_eq!(
                serde_json::to_value(&r).unwrap()["months"]
                    .as_array()
                    .unwrap()
                    .len(),
                2
            );
            let e = with_db(&vault, move |db| reports::report(db, b, a))
                .await
                .unwrap_err();
            assert_eq!(e.field, Some("to"));
        });
    }

    #[test]
    fn backup_then_restore_keeps_the_data() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Mutex::new(Vault::new(Database::path_in(dir.path()))));
        guard(&vault).create("long enough pw").unwrap();
        let data_dir = dir.path().to_path_buf();
        tauri::async_runtime::block_on(async {
            let file = with_vault(&vault, {
                let data_dir = data_dir.clone();
                move |v| backup::backup_now(v, &data_dir)
            })
            .await
            .unwrap();
            assert!(std::path::Path::new(&file.path).is_file());
            assert!(file.bytes > 0);

            let listed = with_vault(&vault, {
                let data_dir = data_dir.clone();
                move |_v| backup::list(&data_dir)
            })
            .await
            .unwrap();
            assert_eq!(listed.len(), 1);
            assert_eq!(listed[0].name, file.name);

            with_vault(&vault, {
                let data_dir = data_dir.clone();
                let name = file.name.clone();
                move |v| backup::restore(v, &data_dir, &name)
            })
            .await
            .unwrap();
            // Still unlocked and usable after the swap.
            assert_eq!(guard(&vault).status().as_str(), "unlocked");
        });
    }

    #[test]
    fn import_shape_guesses_the_mapping() {
        let shape =
            import_shape("Date,Description,Amount\n11/20/2026,COFFEE,-4.50\n".into()).unwrap();
        assert_eq!(shape.headers, ["Date", "Description", "Amount"]);
        assert_eq!(shape.mapping.date, "Date");
        assert_eq!(shape.mapping.amount.as_deref(), Some("Amount"));
        let json = serde_json::to_value(&shape).unwrap();
        assert_eq!(json["mapping"]["date_format"], "mdy");
    }

    #[test]
    fn generic_import_previews_then_commits_with_a_mapping() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Mutex::new(Vault::new(Database::path_in(dir.path()))));
        guard(&vault).create("long enough pw").unwrap();
        let csv = "When,Details,Value\n18/11/2026,BAKERY,-6.50\n";
        let mapping: import::generic::Mapping = serde_json::from_value(serde_json::json!({
            "date": "When",
            "date_format": "dmy",
            "description": "Details",
            "amount": "Value"
        }))
        .unwrap();
        tauri::async_runtime::block_on(async {
            let account = with_db(&vault, |db| accounts::create(db, &input("Everyday")))
                .await
                .unwrap();
            let m = mapping.clone();
            let p = with_db(&vault, move |db| {
                let rows = import::generic::parse(csv, &m)?;
                import::preview(db, account.id, rows)
            })
            .await
            .unwrap();
            assert_eq!(p.rows.len(), 1);
            assert_eq!(p.rows[0].amount.cents(), -650);
            assert_eq!(p.rows[0].date.to_string(), "2026-11-18");

            let rows: Vec<import::ImportRow> =
                serde_json::from_value(serde_json::to_value(&p.rows).unwrap()).unwrap();
            let r = with_db(&vault, move |db| import::commit(db, account.id, &rows))
                .await
                .unwrap();
            assert_eq!(r.imported, 1);
        });
    }

    #[test]
    fn import_presets_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Mutex::new(Vault::new(Database::path_in(dir.path()))));
        guard(&vault).create("long enough pw").unwrap();
        let mapping: import::generic::Mapping = serde_json::from_value(serde_json::json!({
            "date": "When",
            "date_format": "dmy",
            "description": "Details",
            "amount": "Value"
        }))
        .unwrap();
        tauri::async_runtime::block_on(async {
            let saved = with_db(&vault, {
                let m = mapping.clone();
                move |db| import::presets::save(db, "ANZ", &m)
            })
            .await
            .unwrap();
            assert_eq!(saved.name, "ANZ");
            let listed = with_db(&vault, import::presets::list).await.unwrap();
            assert_eq!(listed.len(), 1);
            assert_eq!(listed[0].mapping.amount.as_deref(), Some("Value"));
            with_db(&vault, move |db| import::presets::delete(db, saved.id))
                .await
                .unwrap();
            assert!(
                with_db(&vault, import::presets::list)
                    .await
                    .unwrap()
                    .is_empty()
            );
        });
    }

    #[test]
    fn restore_backup_requires_an_unlocked_vault() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Mutex::new(Vault::new(Database::path_in(dir.path()))));
        guard(&vault).create("long enough pw").unwrap();
        let data_dir = dir.path().to_path_buf();
        tauri::async_runtime::block_on(async {
            let file = with_vault(&vault, {
                let data_dir = data_dir.clone();
                move |v| backup::backup_now(v, &data_dir)
            })
            .await
            .unwrap();
            guard(&vault).lock();
            let e = with_vault(&vault, {
                let data_dir = data_dir.clone();
                move |v| backup::restore(v, &data_dir, &file.name)
            })
            .await
            .unwrap_err();
            assert_eq!(e.code, "locked");
        });
    }
}
