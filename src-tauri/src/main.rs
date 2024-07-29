// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::{Arc, Mutex};

use ap_core::cards::CardsDatabase;
use ap_core::match_insights::MatchInsightDB;
use ap_core::models::deck::Deck;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::api::path::home_dir;
use tauri::{App, Manager, State};
use tracing::{error, info, warn};

use crate::deck::{DeckDifference, GoldfishDeckDisplayRecord};
use crate::scryfall::{Card, ScryfallDataManager};

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

#[derive(Debug, Deserialize, Serialize)]
struct MTGAMatch {
    id: String,
    controller_seat_id: i32,
    controller_player_name: String,
    opponent_player_name: String,
    created_at: String,
}

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
        hand: String,
        opponent_identity: String,
        game_number: i32,
        number_to_keep: i32,
        play_draw: String,
        decision: String,
        scryfall: &ScryfallDataManager,
        cards_database: &CardsDatabase,
    ) -> Self {
        let hand = hand
            .split(',')
            .filter_map(|card_id_str| card_id_str.parse::<i32>().ok())
            .map(|card_id| {
                let mut card = scryfall.get_card_info(card_id, cards_database);
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
}

// TODO: Builder pattern, lol
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
struct MatchDetails {
    id: String,
    did_controller_win: bool,
    controller_player_name: String,
    opponent_player_name: String,
    primary_decklist: Option<GoldfishDeckDisplayRecord>,
    differences: Option<Vec<DeckDifference>>,
    decklists: Vec<Deck>,
    mulligans: Vec<Mulligan>,
}

#[tauri::command]
fn get_matches(db: State<'_, Arc<Mutex<MatchInsightDB>>>) -> Vec<MTGAMatch> {
    let mut matches = Vec::new();
    let db = db.inner().lock().unwrap();
    let mut statement = db.conn.prepare("SELECT * FROM matches").unwrap();
    statement
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
        .map(|rows| rows.collect::<Vec<Result<MTGAMatch, _>>>())
        .unwrap_or_default()
        .into_iter()
        .for_each(|m| {
            if let Ok(m) = m {
                matches.push(m);
            }
        });
    matches
}

#[tauri::command]
fn get_match_details(
    match_id: String,
    scryfall: State<'_, Arc<Mutex<ScryfallDataManager>>>,
    db: State<'_, Arc<Mutex<MatchInsightDB>>>,
) -> MatchDetails {
    let mut db = db.inner().lock().unwrap();
    let scryfall = scryfall.inner().lock().unwrap();
    let mut match_details = {
        let mut statement = db.conn.prepare("\
            SELECT \
                m.id, m.controller_player_name, m.opponent_player_name, m.controller_seat_id = mr.winning_team_id \
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
                Ok(MatchDetails {
                    id,
                    did_controller_win,
                    controller_player_name,
                    opponent_player_name,
                    primary_decklist: None,
                    differences: None,
                    decklists: Vec::new(),
                    mulligans: Vec::new(),
                })
            })
            .unwrap_or_else(|e| {
                error!("Error getting match details: {:?}", e);
                MatchDetails::default()
            })
    };

    let decklists = db.get_decklists(&match_id).unwrap_or(Vec::default());
    match_details.decklists = decklists;

    let primary_decklist = match_details.decklists.first();
    match_details.primary_decklist = if let Some(primary_deck) = primary_decklist {
        Some(GoldfishDeckDisplayRecord::from_decklist(
            primary_deck.clone(),
            &scryfall,
            &db.cards_database,
        ))
    } else {
        None
    };

    match_details.decklists.windows(2).for_each(|pair| {
        if let [prev, next] = pair {
            let diff = DeckDifference::difference(prev, next, &*scryfall, &db.cards_database);
            match_details
                .differences
                .get_or_insert_with(Vec::new)
                .push(diff);
        }
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
            Ok(Mulligan::new(
                hand,
                opponent_identity,
                game_number,
                number_to_keep,
                play_draw,
                decision,
                &scryfall,
                &db.cards_database,
            ))
        })
        .map(|rows| rows.collect::<Vec<Result<Mulligan, _>>>())
        .unwrap_or_default()
        .into_iter()
        .for_each(|mulligan| {
            if let Ok(mulligan) = mulligan {
                match_details.mulligans.push(mulligan);
            }
        });

    match_details
}
#[tauri::command]
fn clear_scryfall_cache(
    scryfall: State<'_, Arc<Mutex<ScryfallDataManager>>>,
) -> Result<(), APError> {
    let scryfall = scryfall.inner().lock().unwrap();
    scryfall.clear().map_err(|_| APError {
        message: "Failed to clear scryfall cache".to_string(),
    })
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

    let scryfall_cache_db_path = app_data_dir.join("scryfall_cache.db");
    let conn =
        Connection::open(scryfall_cache_db_path).expect("Failed to open scryfall cache database");
    let scryfall_manager = ScryfallDataManager::new(conn);
    scryfall_manager
        .init()
        .expect("Failed to initialize scryfall cache database");

    let sm_arc = Arc::new(Mutex::new(scryfall_manager));

    let home = home_dir().expect("could not find home directory");
    let os = std::env::consts::OS;
    let player_log_path = match os {
        "macos" => home.join("Library/Logs/Wizards of the Coast/MTGA/Player.log"),
        "windows" => home.join("AppData/LocalLow/Wizards of the Coast/MTGA/Player.log"),
        _ => panic!("Unsupported OS: {}", os),
    };
    warn!("{player_log_path:?}");

    app.manage(sm_arc.clone());
    app.manage(db_arc.clone());

    ingest::start_processing_logs(db_arc.clone(), player_log_path);

    Ok(())
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tauri::Builder::default()
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            get_matches,
            get_match_details,
            clear_scryfall_cache
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
