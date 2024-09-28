// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]
#![deny(clippy::pedantic)]
#![deny(clippy::unwrap_used)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::needless_pass_by_value)]

use std::error::Error;
use std::fmt::Display;
use std::sync::{Arc, Mutex};

use ap_core::cards::CardsDatabase;
use ap_core::match_insights::MatchInsightDB;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::{path::BaseDirectory, App, Manager};
use tracing::{info, Level};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

mod card;
mod commands;
mod deck;
mod ingest;

#[derive(Debug, Deserialize, Serialize)]
pub enum ArenaBuddySetupError {
    CorruptedAppData,
    LogSetupFailure,
    MatchesDatabaseInitializationFailure,
    NoCardsDatabase,
    NoHomeDir,
    NoMathchesDatabase,
    UnsupportedOS,
}

impl Display for ArenaBuddySetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CorruptedAppData => write!(f, "App data is corrupted"),
            Self::LogSetupFailure => write!(f, "Could not setup logging"),
            Self::MatchesDatabaseInitializationFailure => {
                write!(f, "Matches db initialization failure")
            }
            Self::NoCardsDatabase => write!(f, "Cards database not found"),
            Self::NoHomeDir => write!(f, "Home directory not found"),
            Self::NoMathchesDatabase => write!(f, "Matches database not found"),
            Self::UnsupportedOS => write!(f, "Unsupported operating system"),
        }
    }
}

impl Error for ArenaBuddySetupError {}

fn setup(app: &mut App) -> Result<(), Box<dyn Error>> {
    let registry = tracing_subscriber::registry();
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|_| ArenaBuddySetupError::CorruptedAppData)?;
    std::fs::create_dir_all(&app_data_dir).map_err(|_| ArenaBuddySetupError::CorruptedAppData)?;

    let log_dir = app_data_dir.join("logs");
    std::fs::create_dir_all(&log_dir).map_err(|_| ArenaBuddySetupError::CorruptedAppData)?;

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("arena-buddy")
        .build(log_dir)
        .map_err(|_| ArenaBuddySetupError::LogSetupFailure)?
        .with_max_level(Level::INFO);

    registry
        .with(tracing_subscriber::fmt::layer().with_writer(file_appender))
        .init();

    let cards_path = app
        .path()
        .resolve("./data/cards-full.json", BaseDirectory::Resource)
        .map_err(|_| ArenaBuddySetupError::NoCardsDatabase)?;
    let cards_db =
        CardsDatabase::new(cards_path).map_err(|_| ArenaBuddySetupError::NoCardsDatabase)?;

    let db_path = app_data_dir.join("matches.db");
    info!("Database path: {}", db_path.to_string_lossy());
    let conn = Connection::open(db_path).map_err(|_| ArenaBuddySetupError::NoMathchesDatabase)?;
    let mut db = MatchInsightDB::new(conn, cards_db);
    db.init()
        .map_err(|_| ArenaBuddySetupError::MatchesDatabaseInitializationFailure)?;
    let db_arc = Arc::new(Mutex::new(db));

    let home = app
        .path()
        .home_dir()
        .map_err(|_| ArenaBuddySetupError::NoHomeDir)?;
    let os = std::env::consts::OS;
    let player_log_path = match os {
        "macos" => Ok(home.join("Library/Logs/Wizards of the Coast/MTGA/Player.log")),
        "windows" => Ok(home.join("AppData/LocalLow/Wizards of the Coast/MTGA/Player.log")),
        _ => Err(ArenaBuddySetupError::UnsupportedOS),
    }?;

    app.manage(db_arc.clone());
    info!(
        "Processing logs from : {}",
        player_log_path.to_string_lossy()
    );
    ingest::start_processing_logs(db_arc.clone(), player_log_path);
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            commands::matches::command_matches,
            commands::match_details::command_match_details
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
