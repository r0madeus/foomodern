pub mod audio;
mod commands;
pub mod library;
pub mod lyrics;
pub mod media_keys;
pub mod player;
pub mod queue;

use audio::mpv::{MpvEngine, MpvOptions};
use library::{Hooks, Library};
use player::{Player, PlayerEvent};
use serde::Serialize;
use std::sync::{Arc, mpsc};
use tauri::{Emitter, Manager};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PositionPayload {
    seconds: f64,
    duration: Option<f64>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let (tx, rx) = mpsc::channel();
            let engine = MpvEngine::new(MpvOptions::default(), tx).map_err(|e| {
                log::error!("failed to start audio engine: {e}");
                e
            })?;

            let handle = app.handle().clone();
            let player = Arc::new(Player::new(
                Box::new(engine),
                Box::new(move |event| {
                    let result = match event {
                        PlayerEvent::State(snap) => handle.emit("player://state", snap),
                        PlayerEvent::Position { seconds, duration } => {
                            handle.emit("player://position", PositionPayload { seconds, duration })
                        }
                        PlayerEvent::Error(message) => handle.emit("player://error", message),
                    };
                    if let Err(e) = result {
                        log::warn!("failed to emit player event: {e}");
                    }
                }),
            ));

            let worker = player.clone();
            std::thread::Builder::new()
                .name("engine-events".into())
                .spawn(move || {
                    while let Ok(ev) = rx.recv() {
                        worker.handle_engine_event(ev);
                    }
                })?;

            app.manage(player);

            let data_dir = app.path().app_data_dir()?;
            let cache_dir = app.path().app_cache_dir()?;
            let (progress_handle, changed_handle) = (app.handle().clone(), app.handle().clone());
            let library = Arc::new(
                Library::open(
                    &data_dir.join("library.sqlite3"),
                    &cache_dir.join("covers"),
                    Hooks {
                        on_progress: Box::new(move |p| {
                            if let Err(e) = progress_handle.emit("library://scan", p) {
                                log::warn!("failed to emit library scan event: {e}");
                            }
                        }),
                        on_changed: Box::new(move |version| {
                            if let Err(e) = changed_handle.emit("library://changed", version) {
                                log::warn!("failed to emit library change event: {e}");
                            }
                        }),
                    },
                )
                .map_err(|e| {
                    log::error!("failed to open library: {e}");
                    e
                })?,
            );
            app.manage(library);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::queue_replace,
            commands::queue_append,
            commands::queue_remove,
            commands::queue_move,
            commands::queue_page,
            commands::player_play_index,
            commands::player_play,
            commands::player_pause,
            commands::player_toggle_pause,
            commands::player_next,
            commands::player_previous,
            commands::player_seek,
            commands::player_set_volume,
            commands::player_snapshot,
            commands::library_add_folder,
            commands::library_remove_folder,
            commands::library_folders,
            commands::library_rescan,
            commands::library_stats,
            commands::library_tracks_page,
            commands::library_albums_page,
            commands::library_album,
            commands::library_play_album,
            commands::library_play_all,
        ])
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
