use std::sync::Mutex;

use maud::{html, Render};
use server_side_rendering::{ChainOption, ChainSelector};
use tauri::{Manager, State};

mod errors;
mod migrations;

use errors::Error;

#[tauri::command]
fn list_nodes(state: State<'_, Mutex<AppState>>, selected_chain: &str) -> Result<String, Error> {
    let nodes = {
        let state = state.lock().map_err(|_| Error::StateMutexPoisoned)?;
        wallet::db::node::fetch_all(&state.db_conn)
    }?;

    Ok("ok dude".to_string())
}

#[tauri::command]
fn chain_selector() -> String {
    let parent_div_id = "parent_container";
    let chain_picker = ChainSelector {
        id: "base_chain_picker".to_string(),
        hx_target: format!("#{}", parent_div_id),
        tauri_invoke: "greet".to_string(),
        chain_options: vec![
            ChainOption {
                label: "Starknet".to_string(),
                value: "starknet".to_string(),
            },
            ChainOption {
                label: "Celo".to_string(),
                value: "celo".to_string(),
            },
        ],
    };

    let div = html! {
        div id=(parent_div_id) {
            (chain_picker)
        }
    };

    div.render().into_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // let db_path =
    // let mut db_conn = rusqlite::Connection::open(db_path)?;

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations("sqlite:wallet.sqlite3", migrations::migrations())
                .build(),
        )
        .setup(|app| {
            let db_path = dirs::data_dir()
                .map(|mut dp| {
                    dp.push("wallet.sqlite3");
                    dp
                })
                .expect("dirs::data_dir should map to a valid path on this machine");
            let mut db_conn = rusqlite::Connection::open(db_path)?;
            // Initialize the database.
            wallet::db::create_tables(&mut db_conn);

            app.manage(Mutex::new(AppState { db_conn }));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![list_nodes, chain_selector])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[derive(Debug)]
struct AppState {
    db_conn: rusqlite::Connection,
}
