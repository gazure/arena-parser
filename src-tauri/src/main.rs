// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};

use ap_core::cards::CardsDatabase;
use ap_core::match_insights::MatchInsightDB;
use notify::Watcher;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};
use tauri::api::path::home_dir;
use tracing::{error, info};

use crate::deck::GoldfishDeckDisplayRecord;
use crate::scryfall::ScryfallDataManager;

mod deck;
mod ingest;
mod scryfall;

#[derive(Debug, Deserialize, Serialize)]
struct MTGAMatch {
    id: String,
    controller_seat_id: i32,
    controller_player_name: String,
    opponent_player_name: String,
    created_at: String,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
struct DeckList {
    game_number: i32,
    deck: Vec<i32>,
    sideboard: Vec<i32>,
}

impl DeckList {
    fn new(game_number: i32, deck: Vec<i32>, sideboard: Vec<i32>) -> Self {
        Self {
            game_number,
            deck,
            sideboard,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
struct Mulligan {
    hand: Vec<String>,
    opponent_identity: String,
    game_number: i32,
    number_to_keep: i32,
    play_draw: String,
    decision: String,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
struct MatchDetails {
    id: String,
    did_controller_win: bool,
    controller_player_name: String,
    opponent_player_name: String,
    primary_decklist: Option<GoldfishDeckDisplayRecord>,
    decklists: Vec<DeckList>,
    mulligans: Vec<Mulligan>,
}

#[tauri::command]
fn get_matches(db: State<'_, Arc<Mutex<MatchInsightDB>>>) -> Vec<MTGAMatch> {
    let mut matches = Vec::new();
    let db = db.inner().lock().unwrap();
    let mut statement = db.conn.prepare("SELECT * FROM matches").unwrap();
    let rows = statement
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let controller_seat_id: i32 = row.get(1)?;
            let controller_player_name: String = row.get(2)?;
            let opponent_player_name: String = row.get(3)?;
            Ok(MTGAMatch {
                id,
                controller_seat_id,
                controller_player_name,
                opponent_player_name,
                created_at: "".to_string(),
            })
        })
        .unwrap();
    for row in rows {
        matches.push(row.unwrap());
    }
    matches
}

fn process_raw_decklist(raw_decklist: &str) -> Vec<i32> {
    let parsed = serde_json::from_str(raw_decklist).unwrap();
    match parsed {
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|v| v.as_i64())
            .map(|v| v as i32)
            .collect(),
        _ => Vec::new(),
    }
}

#[tauri::command]
fn get_match_details(match_id: String, scryfall: State<'_, Arc<Mutex<ScryfallDataManager>>>, db: State<'_, Arc<Mutex<MatchInsightDB>>>) -> MatchDetails {
    let db = db.inner().lock().unwrap();
    let scryfall = scryfall.inner().lock().unwrap();
    let mut statement = db.conn.prepare("\
        SELECT \
            m.id, m.controller_player_name, m.opponent_player_name, m.controller_seat_id = mr.winning_team_id \
        FROM matches m JOIN match_results mr ON m.id = mr.match_id \
        WHERE m.id = ?1 AND mr.result_scope = \"MatchScope_Match\" LIMIT 1
    ").unwrap();
    info!("Getting match details for match_id: {}", match_id);
    let mut match_details = statement
        .query_row([&match_id], |row| {
            let id: String = row.get(0)?;
            let controller_player_name: String = row.get(1)?;
            let opponent_player_name: String = row.get(2)?;
            let did_controller_win: bool = row.get(3)?;
            Ok(MatchDetails {
                id,
                did_controller_win,
                controller_player_name,
                opponent_player_name,
                primary_decklist: None,
                decklists: Vec::new(),
                mulligans: Vec::new(),
            })
        }).unwrap_or_else(|e| {
            error!("Error getting match details: {:?}", e);
            MatchDetails::default()
        });

    let mut decklists_statement = db
        .conn
        .prepare(
            "\
        SELECT d.game_number, d.deck_cards, d.sideboard_cards FROM decks d WHERE d.match_id = ?1
    ",
        )
        .unwrap();

    decklists_statement
        .query_map([&match_id], |row| {
            let game_number: i32 = row.get(0)?;
            let maindeck_string: String = row.get(1)?;
            let sideboard_string: String = row.get(2)?;

            let maindeck_parsed = process_raw_decklist(&maindeck_string);
            let sideboard_parsed = process_raw_decklist(&sideboard_string);

            Ok(DeckList::new(
                game_number,
                maindeck_parsed,
                sideboard_parsed,
            ))
        })
        .unwrap()
        .for_each(|row| match_details.decklists.push(row.unwrap()));

    let primary_decklist = match_details.decklists.first().unwrap();
    let goldfish_dr_res = GoldfishDeckDisplayRecord::from_decklist(
        primary_decklist.clone(),
        &scryfall,
        &db.cards_database,
    );
    match_details.primary_decklist = goldfish_dr_res.ok();

    let mut mulligans_statement = db.conn.prepare("\
        SELECT m.game_number, m.number_to_keep, m.hand, m.play_draw, m.opponent_identity, m.decision \
        FROM mulligans m where m.match_id = ?1 \
    ").unwrap();

    mulligans_statement
        .query_map([&match_id], |row| {
            let game_number = row.get(0)?;
            let number_to_keep = row.get(1)?;
            let hand: String = row.get(2)?;
            let play_draw: String = row.get(3)?;
            let opponent_identity: String = row.get(4)?;
            let decision: String = row.get(5)?;

            Ok(Mulligan {
                game_number,
                number_to_keep,
                hand: hand.split(',').map(|s| s.to_string()).collect(),
                play_draw,
                opponent_identity,
                decision,
            })
        })
        .unwrap()
        .for_each(|mulligan| {
            let mulligan = mulligan.unwrap();
            match_details.mulligans.push(mulligan);
        });

    match_details
}

#[tauri::command]
fn hello_next_tauri() -> String {
    "Hello Next Tauri App!".to_string()
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tauri::Builder::default()
        .setup(|app| {
            let cards_path = app
                .path_resolver()
                .resolve_resource("./data/cards-full.json")
                .unwrap();
            let cards_db = CardsDatabase::new(cards_path).expect("Failed to load cards database");

            let app_data_dir = app.path_resolver().app_data_dir().unwrap();
            std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");

            let db_path = app_data_dir.join("matches.db");
            let conn = Connection::open(db_path).expect("Failed to open database");
            let mut db = MatchInsightDB::new(conn, cards_db);
            db.init().expect("Failed to initialize database");
            let db_arc = Arc::new(Mutex::new(db));

            let scryfall_cache_db_path = app_data_dir.join("scryfall_cache.db");
            let conn = Connection::open(scryfall_cache_db_path)
                .expect("Failed to open scryfall cache database");
            let scryfall_manager = scryfall::ScryfallDataManager::new(conn);
            scryfall_manager
                .init()
                .expect("Failed to initialize scryfall cache database");

            let sm_arc = Arc::new(Mutex::new(scryfall_manager));

            let player_log_path = home_dir()
                .expect("Could not find player.log in home dir")
                .join("AppData/LocalLow/Wizards of the Coast/MTGA/Player.log");

            app.manage(sm_arc.clone());
            app.manage(db_arc.clone());

            ingest::start_processing_logs(db_arc.clone(), player_log_path);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            hello_next_tauri,
            get_matches,
            get_match_details
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
