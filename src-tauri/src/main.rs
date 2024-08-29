// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]
#![deny(clippy::pedantic)]
#![deny(clippy::unwrap_used)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::too_many_arguments)]

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::{Arc, Mutex};

use ap_core::cards::CardsDatabase;
use ap_core::match_insights::MatchInsightDB;
use ap_core::models::deck::Deck;
use ap_core::models::match_result::MatchResult;
use ap_core::models::mtga_match::MTGAMatch;
use ap_core::models::mulligan::MulliganInfo;
use chrono::DateTime;
use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::api::path::home_dir;
use tauri::{App, Manager, State};
use tracing::{error, info};

use crate::deck::{DeckDifference, GoldfishDeckDisplayRecord};
use crate::scryfall::Card;

mod deck;
mod ingest;
mod scryfall;

#[derive(Debug, Deserialize, Serialize)]
struct APError {
    message: String,
}

impl Display for APError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for APError {}

// TODO: Unify this with MulliganInfo in library crate
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
struct Mulligan {
    hand: Vec<Card>,
    opponent_identity: String,
    game_number: i32,
    number_to_keep: i32,
    play_draw: String,
    decision: String,
}

impl Mulligan {
    pub fn new(
        hand: &str,
        opponent_identity: String,
        game_number: i32,
        number_to_keep: i32,
        play_draw: String,
        decision: String,
        cards_database: &CardsDatabase,
    ) -> Self {
        let hand = hand
            .split(',')
            .filter_map(|card_id_str| card_id_str.parse::<i32>().ok())
            .map(|card_id| {
                let mut card: Card = cards_database
                    .get(&card_id)
                    .map(|db_entry| db_entry.into())
                    .unwrap_or_else(|| {
                        let mut card = Card::default();
                        card.name = card_id.to_string();
                        card
                    });
                card.quantity = 1;
                card
            })
            .collect();

        Self {
            hand,
            opponent_identity,
            game_number,
            number_to_keep,
            play_draw,
            decision,
        }
    }

    pub fn from_mulligan_info(
        mulligan_info: &MulliganInfo,
        cards_database: &CardsDatabase,
    ) -> Self {
        Self::new(
            &mulligan_info.hand,
            mulligan_info.opponent_identity.clone(),
            mulligan_info.game_number,
            mulligan_info.number_to_keep,
            mulligan_info.play_draw.clone(),
            mulligan_info.decision.clone(),
            cards_database,
        )
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct GameResultDisplay {
    pub game_number: i32,
    pub winning_player: String,
}

impl GameResultDisplay {
    pub fn from_match_result(
        mr: &MatchResult,
        controller_seat_id: i32,
        controller_player_name: &str,
        opponent_player_name: &str,
    ) -> Self {
        Self {
            game_number: mr.game_number.unwrap_or_default(),
            winning_player: if mr.winning_team_id == controller_seat_id {
                controller_player_name.into()
            } else {
                opponent_player_name.into()
            },
        }
    }
}

// TODO: Builder pattern, lol
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
struct MatchDetails {
    id: String,
    did_controller_win: bool,
    controller_seat_id: i32,
    controller_player_name: String,
    opponent_player_name: String,
    created_at: DateTime<Utc>,
    primary_decklist: Option<GoldfishDeckDisplayRecord>,
    differences: Option<Vec<DeckDifference>>,
    game_results: Vec<GameResultDisplay>,
    decklists: Vec<Deck>,
    mulligans: Vec<Mulligan>,
}

#[tauri::command]
fn get_matches(db: State<'_, Arc<Mutex<MatchInsightDB>>>) -> Vec<MTGAMatch> {
    let mut db = db.inner().lock().expect("Failed to lock db");
    db.get_matches().unwrap_or_else(|e| {
        error!("error retrieving matches {}", e);
        Vec::default()
    })
}

#[tauri::command]
fn get_match_details(match_id: String, db: State<'_, Arc<Mutex<MatchInsightDB>>>) -> MatchDetails {
    let mut db = db.inner().lock().unwrap();
    let mut match_details = {
        let mut statement = db.conn.prepare("\
            SELECT \
                m.id, m.controller_player_name, m.opponent_player_name, m.controller_seat_id = mr.winning_team_id, m.controller_seat_id, m.created_at \
            FROM matches m JOIN match_results mr ON m.id = mr.match_id \
            WHERE m.id = ?1 AND mr.result_scope = \"MatchScope_Match\" LIMIT 1
        ").unwrap();

        info!("Getting match details for match_id: {}", match_id);
        statement
            .query_row([&match_id], |row| {
                let id: String = row.get(0)?;
                let controller_player_name: String = row.get(1)?;
                let opponent_player_name: String = row.get(2)?;
                let did_controller_win: bool = row.get(3)?;
                let controller_seat_id: i32 = row.get(4)?;
                let created_at: DateTime<Utc> = row.get(5)?;
                Ok(MatchDetails {
                    id,
                    did_controller_win,
                    controller_seat_id,
                    controller_player_name,
                    opponent_player_name,
                    created_at,
                    primary_decklist: None,
                    differences: None,
                    game_results: Vec::new(),
                    decklists: Vec::new(),
                    mulligans: Vec::new(),
                })
            })
            .unwrap_or_else(|e| {
                error!("Error getting match details: {:?}", e);
                MatchDetails::default()
            })
    };

    match_details.decklists = db.get_decklists(&match_id).unwrap_or_default();

    match_details.primary_decklist = match_details.decklists.first().map(|primary_decklist| {
        GoldfishDeckDisplayRecord::from_decklist(primary_decklist, &db.cards_database)
    });

    match_details.decklists.windows(2).for_each(|pair| {
        if let [prev, next] = pair {
            let diff = DeckDifference::difference(prev, next, &db.cards_database);
            match_details
                .differences
                .get_or_insert_with(Vec::new)
                .push(diff);
        }
    });

    let raw_mulligans = db.get_mulligans(&match_id).unwrap_or_else(|e| {
        error!("Error retrieving Mulligans: {}", e);
        Vec::default()
    });

    match_details.mulligans = raw_mulligans
        .iter()
        .map(|mulligan| Mulligan::from_mulligan_info(mulligan, &db.cards_database))
        .collect();

    match_details.game_results = db
        .get_match_results(&match_id)
        .unwrap_or_else(|e| {
            error!("Error retrieving game results: {}", e);
            Vec::default()
        })
        .iter()
        .map(|mr| {
            GameResultDisplay::from_match_result(
                mr,
                match_details.controller_seat_id,
                &match_details.controller_player_name,
                &match_details.opponent_player_name,
            )
        })
        .collect();

    match_details
}

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
    ingest::start_processing_logs(db_arc.clone(), player_log_path);
    Ok(())
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    tauri::Builder::default()
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            get_matches,
            get_match_details
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
