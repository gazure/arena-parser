use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ap_core::match_insights::MatchInsightDB;
use ap_core::processor::{ArenaEventSource, PlayerLogProcessor};
use ap_core::replay::MatchReplayBuilder;
use ap_core::storage_backends::ArenaMatchStorageBackend;
use crossbeam_channel::{select, unbounded, Sender};
use notify::{Event, Watcher};
use tracing::{error, info};

fn watch_player_log_rotation(notify_tx: Sender<Event>, player_log_path: &PathBuf) {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| match res {
        Ok(event) => {
            notify_tx.send(event).unwrap_or(());
        }
        Err(e) => {
            error!("watch error: {:?}", e);
        }
    })
    .expect("Could not create watcher");
    watcher
        .watch(player_log_path, notify::RecursiveMode::NonRecursive)
        .expect("Could not watch player log path");
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn log_process_start(db: Arc<Mutex<MatchInsightDB>>, player_log_path: &PathBuf) {
    let (notify_tx, notify_rx) = unbounded::<Event>();
    let mut processor = PlayerLogProcessor::try_new(player_log_path.clone())
        .expect("Could not build player log processor");
    let mut match_replay_builder = MatchReplayBuilder::new();
    info!("Player log: {:?}", player_log_path);
    let plp = player_log_path.clone();

    std::thread::spawn(move || {
        watch_player_log_rotation(notify_tx, &plp);
    });

    loop {
        select! {
            recv(notify_rx) -> event => {
                if let Ok(event) = event {
                    info!("log file rotated!, {:?}", event);
                    processor = PlayerLogProcessor::try_new(player_log_path.clone())
                        .expect("Could not build player log processor");
                }
            }
            default(Duration::from_secs(1)) => {
                while let Some(parse_output) = processor.get_next_event() {
                    if match_replay_builder.ingest_event(parse_output) {
                        let match_replay = match_replay_builder.build();
                        match match_replay {
                            Ok(mr) => {
                                let mut db = db.lock().expect("Could not lock db");
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

pub fn start_processing_logs(db: Arc<Mutex<MatchInsightDB>>, player_log_path: PathBuf) {
    std::thread::spawn(move || {
        log_process_start(db, &player_log_path);
    });
}
