//! Tauri shell: app state and commands only. Logic lives in `plutus-core`.

use std::sync::{Arc, Mutex, MutexGuard};

use plutus_core::{Database, DbError, MIN_PASSWORD_CHARS, Vault};
use serde::Serialize;
use tauri::{Manager, State};

/// Shared app state: the vault guarding the encrypted database.
struct AppState {
    vault: Arc<Mutex<Vault>>,
}

/// Error shape returned to the UI: a stable `code` plus a readable message.
#[derive(Debug, Serialize)]
struct CommandError {
    code: &'static str,
    message: String,
}

impl From<DbError> for CommandError {
    fn from(e: DbError) -> Self {
        Self {
            code: e.code(),
            message: e.to_string(),
        }
    }
}

impl From<tauri::Error> for CommandError {
    fn from(e: tauri::Error) -> Self {
        Self {
            code: "internal",
            message: e.to_string(),
        }
    }
}

type CmdResult<T> = Result<T, CommandError>;

/// Lock the vault mutex, recovering from poisoning (a panic in another
/// command must not brick the app).
fn guard(vault: &Mutex<Vault>) -> MutexGuard<'_, Vault> {
    vault.lock().unwrap_or_else(|e| e.into_inner())
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
            lock_vault
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
    }
}
