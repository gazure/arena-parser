// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]
#![deny(clippy::pedantic)]
#![deny(clippy::unwrap_used)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::too_many_arguments)]

use std::error::Error;
use std::sync::{Arc, Mutex};

use ap_core::cards::CardsDatabase;
use ap_core::match_insights::MatchInsightDB;
use rusqlite::Connection;
use tauri::api::path::home_dir;
use tauri::{App, Manager};
use tracing::info;

mod deck;
mod ingest;
mod scryfall;
mod commands;


fn setup(app: &mut App) -> Result<(), Box<dyn Error>> {
    let cards_path = app
        .path_resolver()
        .resolve_resource("./data/cards-full.json")
        .unwrap();
    let cards_db = CardsDatabase::new(cards_path).expect("Failed to load cards database");

    let app_data_dir = app
        .path_resolver()
        .app_data_dir()
        .expect("Failed to get app data dir");
    std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");

    let db_path = app_data_dir.join("matches.db");
    info!("Database path: {}", db_path.to_string_lossy());
    let conn = Connection::open(db_path).expect("Failed to open database");
    let mut db = MatchInsightDB::new(conn, cards_db);
    db.init().expect("Failed to initialize database");
    let db_arc = Arc::new(Mutex::new(db));

    let home = home_dir().expect("could not find home directory");
    let os = std::env::consts::OS;
    let player_log_path = match os {
        "macos" => home.join("Library/Logs/Wizards of the Coast/MTGA/Player.log"),
        "windows" => home.join("AppData/LocalLow/Wizards of the Coast/MTGA/Player.log"),
        _ => panic!("Unsupported OS: {}", os),
    };

    app.manage(db_arc.clone());
    info!("Processing logs from : {}", player_log_path.to_string_lossy());
    ingest::start_processing_logs(db_arc.clone(), player_log_path);
    Ok(())
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tauri::Builder::default()
        .setup(setup)
        .invoke_handler(tauri::generate_handler![commands::matches::command_matches, commands::match_details::command_match_details])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
