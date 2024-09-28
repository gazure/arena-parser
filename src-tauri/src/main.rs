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
use tauri::{App, Manager, path::BaseDirectory};
use tracing::{info, Level};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

mod deck;
mod ingest;
mod card;
mod commands;


fn setup(app: &mut App) -> Result<(), Box<dyn Error>> {
    let registry = tracing_subscriber::registry();
    let app_data_dir = app
        .path()
        .app_data_dir()
        .expect("Failed to get app data dir");
    std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");

    let log_dir = app_data_dir.join("logs");
    std::fs::create_dir_all(&log_dir).expect("Failed to create log directory");

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("arena-buddy")
        .build(log_dir)
        .expect("log file appender")
        .with_max_level(Level::INFO);

    registry
        .with(tracing_subscriber::fmt::layer().with_writer(file_appender))
        .init();

    let cards_path = app.path()
        .resolve("./data/cards-full.json", BaseDirectory::Resource)
        .expect("Failed to find cards database");
    let cards_db = CardsDatabase::new(cards_path).expect("Failed to load cards database");

    let db_path = app_data_dir.join("matches.db");
    info!("Database path: {}", db_path.to_string_lossy());
    let conn = Connection::open(db_path).expect("Failed to open database");
    let mut db = MatchInsightDB::new(conn, cards_db);
    db.init().expect("Failed to initialize database");
    let db_arc = Arc::new(Mutex::new(db));

    let home = app.path().home_dir().expect("could not find home directory");
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
    tauri::Builder::default()
        .setup(setup)
        .invoke_handler(tauri::generate_handler![commands::matches::command_matches, commands::match_details::command_match_details])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
