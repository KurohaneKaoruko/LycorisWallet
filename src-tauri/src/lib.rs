//! Lycoris Wallet — Tauri shell.
//!
//! The Rust side owns every secret and every network call. The frontend is a
//! pure view layer that talks to the commands in [`commands`].

mod commands;
mod error;
mod portfolio;
mod state;
mod window;

use state::AppState;
use std::path::PathBuf;
use std::time::Duration;
use tauri::Manager;

/// How often the idle timer is checked. Short enough that the UI reacts
/// promptly, long enough to be free when the wallet is idle.
const AUTO_LOCK_TICK: Duration = Duration::from_secs(20);

fn resolve_data_dir(app: &tauri::AppHandle) -> PathBuf {
    if let Ok(custom) = std::env::var("LYCORIS_VAULT_DIR") {
        if !custom.trim().is_empty() {
            return PathBuf::from(custom);
        }
    }
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("lycoris-wallet"))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = resolve_data_dir(app.handle());
            std::fs::create_dir_all(&dir).ok();
            app.manage(AppState::new(dir));

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(AUTO_LOCK_TICK).await;
                    let Some(state) = handle.try_state::<AppState>() else {
                        continue;
                    };
                    if state.auto_lock_due() {
                        state.lock_now();
                        use tauri::Emitter;
                        let _ = handle.emit("lycoris://locked", ());
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::vault_status,
            commands::create_vault,
            commands::unlock_vault,
            commands::lock_vault,
            commands::wallet,
            commands::add_wallet,
            commands::switch_wallet,
            commands::rename_wallet,
            commands::delete_wallet,
            commands::reset_vault,
            commands::generate_mnemonic,
            commands::preview_derivation,
            commands::reveal_mnemonic,
            commands::export_private_key,
            commands::change_password,
            commands::refresh_portfolio,
            commands::quote_send,
            commands::send_transaction,
            commands::save_contact,
            commands::delete_contact,
            commands::save_settings,
            commands::qr_svg,
            commands::open_external,
            commands::delete_vault,
            commands::touch_session,
            commands::window_minimize,
            commands::window_toggle_maximize,
            commands::window_close,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Lycoris Wallet");
}
