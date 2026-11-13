pub mod audio;
mod commands;
pub mod library;
pub mod lyrics;
#[cfg(not(target_os = "android"))]
pub mod media_keys;
mod net;
pub mod player;
pub mod queue;
pub mod session;
pub mod tray;
pub mod update;

#[cfg(not(target_os = "android"))]
use audio::mpv::{MpvEngine, MpvOptions};
use library::{Hooks, Library};
use lyrics::LyricsService;
use player::{Player, PlayerEvent};
use serde::Serialize;
use session::SessionStore;
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
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());
    #[cfg(target_os = "android")]
    let builder = builder.plugin(tauri_plugin_android_audio::init());
    builder
        .setup(|app| {
            // Logs go to stdout and to a file in the app log folder (shown in
            // Settings), in release builds too, so problems on an installed
            // copy can be diagnosed. Two old files are kept.
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .max_file_size(2 * 1024 * 1024)
                    .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(2))
                    .build(),
            )?;
            let default_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                log::error!("panic: {info}");
                default_hook(info);
            }));

            let data_dir = app.path().app_data_dir()?;
            let cache_dir = app.path().app_cache_dir()?;
            let session_store = Arc::new(SessionStore::new(data_dir.join("session.json")));

            let (tx, rx) = mpsc::channel();
            #[cfg(not(target_os = "android"))]
            let engine = MpvEngine::new(MpvOptions::default(), tx);
            #[cfg(target_os = "android")]
            let engine = audio::android::AndroidEngine::new(
                app.state::<tauri_plugin_android_audio::AndroidAudio<tauri::Wry>>()
                    .0
                    .clone(),
                tx,
            );
            let engine = engine.map_err(|e| {
                log::error!("failed to start audio engine: {e}");
                e
            })?;

            let handle = app.handle().clone();
            let observer_session = Arc::clone(&session_store);
            let player = Arc::new(Player::new(
                Box::new(engine),
                Box::new(move |event| {
                    let result = match event {
                        PlayerEvent::State(snap) => {
                            if let Some(p) = handle.try_state::<Arc<Player>>() {
                                observer_session.on_player_state(&snap, || p.export_session());
                            }
                            #[cfg(not(target_os = "android"))]
                            if let Some(keys) = handle.try_state::<media_keys::MediaKeys>() {
                                keys.publish(&snap, |path| {
                                    let Some(lib) = handle.try_state::<Arc<Library>>() else {
                                        return media_keys::Extra::default();
                                    };
                                    media_keys::Extra {
                                        album: lib.track_meta(path).ok().flatten().map(|m| m.album),
                                        cover: lib.track_cover(path).ok().flatten(),
                                    }
                                });
                            }
                            handle.emit("player://state", snap)
                        }
                        PlayerEvent::Position { seconds, duration } => {
                            if let Some(p) = handle.try_state::<Arc<Player>>() {
                                observer_session.maybe_save_position(|| p.export_session());
                            }
                            handle.emit("player://position", PositionPayload { seconds, duration })
                        }
                        PlayerEvent::Error(message) => handle.emit("player://error", message),
                    };
                    if let Err(e) = result {
                        log::warn!("failed to emit player event: {e}");
                    }
                }),
            ));

            player.restore_session(session_store.load());

            let worker = player.clone();
            std::thread::Builder::new()
                .name("engine-events".into())
                .spawn(move || {
                    while let Ok(ev) = rx.recv() {
                        worker.handle_engine_event(ev);
                    }
                })?;

            #[cfg(not(target_os = "android"))]
            {
                #[cfg(target_os = "windows")]
                let hwnd = app
                    .get_webview_window("main")
                    .and_then(|w| w.hwnd().ok())
                    .map(|h| h.0 as *mut std::ffi::c_void);
                #[cfg(not(target_os = "windows"))]
                let hwnd = None;
                if let Some(keys) = media_keys::create(hwnd, &player) {
                    app.manage(keys);
                }
            }

            app.manage(player);
            app.manage(session_store);

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

            let lyrics = Arc::new(LyricsService::new(&cache_dir.join("lyrics")).map_err(|e| {
                log::error!("failed to start lyrics service: {e}");
                e
            })?);
            app.manage(lyrics);

            app.manage(tray::TrayStatus::default());
            tray::install(app.handle());
            Ok(())
        })
        .on_window_event(tray::on_window_event)
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
            commands::player_set_shuffle,
            commands::player_set_repeat,
            commands::player_set_replaygain,
            commands::player_snapshot,
            commands::library_add_folder,
            commands::library_android_music_folder,
            commands::library_track_cover,
            commands::library_cover_color,
            commands::library_remove_folder,
            commands::library_folders,
            commands::library_rescan,
            commands::library_stats,
            commands::library_tracks_page,
            commands::library_albums_page,
            commands::library_album,
            commands::library_play_album,
            commands::library_play_all,
            commands::playlists_list,
            commands::playlist_create,
            commands::playlist_rename,
            commands::playlist_delete,
            commands::playlist_tracks,
            commands::playlist_add_tracks,
            commands::playlist_add_album,
            commands::playlist_remove,
            commands::playlist_move,
            commands::playlist_play,
            commands::playlist_from_queue,
            commands::app_settings,
            commands::app_window_insets,
            commands::app_log_dir,
            commands::app_check_update,
            commands::app_open_release,
            commands::app_set_close_to_tray,
            commands::tags_read,
            commands::tags_write,
            commands::lyrics_for_track,
            commands::lyrics_attach_file,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if matches!(event, tauri::RunEvent::Exit)
                && let (Some(player), Some(session)) = (
                    app_handle.try_state::<Arc<Player>>(),
                    app_handle.try_state::<Arc<SessionStore>>(),
                )
            {
                session.save_now(&player.export_session());
            }
        });
}
