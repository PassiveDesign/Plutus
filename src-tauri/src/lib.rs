use serde::Serialize;

#[derive(Serialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
    os: &'static str,
}

/// Basic app metadata, exposed to the UI. Also serves as a smoke test for the
/// Rust <-> UI bridge.
#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "Plutus",
        version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![app_info])
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
    }
}
