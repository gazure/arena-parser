// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use ap_core::cards::CardsDatabase;
use ap_core::match_insights::MatchInsightDB;
use ap_core::processor::{ArenaEventSource, PlayerLogProcessor};
use ap_core::replay::MatchReplayBuilder;
use ap_core::storage_backends::ArenaMatchStorageBackend;
use crossbeam_channel::{select, unbounded};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::api::path::home_dir;
use tauri::{Manager, State};
use tracing::{error, info};

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
    deck: Vec<String>,
    sideboard: Vec<String>,
}

impl DeckList {
    fn new(game_number: i32, main: &str, sideboard: &str) -> Self {
        Self {
            game_number,
            deck: main.split(',').map(|s| s.to_string()).collect(),
            sideboard: sideboard.split(',').map(|s| s.to_string()).collect(),
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

#[tauri::command]
fn get_match_details(match_id: String, db: State<'_, Arc<Mutex<MatchInsightDB>>>) -> MatchDetails {
    let db = db.inner().lock().unwrap();
    let mut statement = db.conn.prepare("\
        SELECT \
            m.id, m.controller_player_name, m.opponent_player_name, m.controller_seat_id = mr.winning_team_id \
        FROM matches m JOIN match_results mr ON m.id = mr.match_id \
        WHERE m.id = ?1 AND mr.result_scope = \"MatchScope_Match\" LIMIT 1
    ").unwrap();
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
                decklists: Vec::new(),
                mulligans: Vec::new(),
            })
        })
        .unwrap();

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

            Ok(
                DeckList::new(game_number, &maindeck_string, &sideboard_string),
            )
        })
        .unwrap()
        .for_each(|row| {
            match_details.decklists.push(row.unwrap())
        });

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
            match_details
                .mulligans
                .push(mulligan);
        });

    match_details
}

#[tauri::command]
fn hello_next_tauri() -> String {
    "Hello Next Tauri App!".to_string()
}

fn log_process_start(db: Arc<Mutex<MatchInsightDB>>, player_log_path: PathBuf) {
    let (_notify_tx, notify_rx) = unbounded::<()>();
    let mut processor = PlayerLogProcessor::try_new(player_log_path.clone())
        .expect("Could not build player log processor");
    let mut match_replay_builder = MatchReplayBuilder::new();
    info!("Player log: {:?}", player_log_path);
    loop {
        select! {
            recv(notify_rx) -> _ => {
                info!("do something with notify");
            }
            default(Duration::from_secs(1)) => {
                while let Some(parse_output) = processor.get_next_event() {
                    if match_replay_builder.ingest_event(parse_output) {
                        let match_replay = match_replay_builder.build();
                        match match_replay {
                            Ok(mr) => {
                                let mut db = db.lock().unwrap();
                                db.write(&mr).expect("Could not write match replay to db");
                            }
                            Err(e) => {
                                error!("Error building match replay: {}", e);
                            }
                        }
                        match_replay_builder = MatchReplayBuilder::new();
                    }
                }
            }
        }
    }
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

            let home_dir = home_dir()
                .expect("Could not find player.log in home dir")
                .join("AppData/LocalLow/Wizards of the Coast/MTGA/Player.log");

            app.manage(db_arc.clone());

            std::thread::spawn(move || {
                info!("Spawning processor thread");
                log_process_start(db_arc, home_dir);
            });

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
