
use std::sync::{Arc, Mutex};
use tracing::error;
use ap_core::models::mtga_match::MTGAMatch;
use ap_core::match_insights::MatchInsightDB;
use tauri::State;

#[tauri::command]
pub fn command_matches(db: State<'_, Arc<Mutex<MatchInsightDB>>>) -> Vec<MTGAMatch> {
    let mut db = db.inner().lock().expect("Failed to lock db");
    db.get_matches()
        .unwrap_or_else(|e| {
            error!("error retrieving matches {}", e);
            Vec::default()
        })
        .into_iter()
        .rev()
        .collect()
}
