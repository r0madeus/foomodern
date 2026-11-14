pub mod audio;
mod commands;
pub mod library;
pub mod lyrics;
#[cfg(not(target_os = "android"))]
pub mod media_keys;
pub mod metadata;
pub mod net;
pub mod online;
pub mod player;
pub mod queue;
pub mod session;
pub mod tray;
pub mod update;

#[cfg(not(target_os = "android"))]
use audio::mpv::{MpvEngine, MpvOptions};
use library::{Hooks, Library};
use lyrics::LyricsService;
use online::OnlineService;
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
    let builder = tauri::Builder::default();
    // Register single-instance first so a second desktop launch focuses the
    // existing window (also recovers hidden windows when close-to-tray is on
    // and no tray host is visible) before other plugins initialize.
    #[cfg(not(target_os = "android"))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        tray::show_main_window(app);
    }));
    let builder = builder
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
                Box::new(audio::NetGuardedEngine::new(
                    Box::new(engine),
                    session_store.net_policy(),
                )),
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
                        PlayerEvent::Listen(rec) => {
                            if let Some(lib) = handle.try_state::<Arc<Library>>() {
                                if let Err(e) = lib.record_listen_event(library::NewListenEvent {
                                    path: rec.path,
                                    title: rec.title,
                                    artist: rec.artist,
                                    duration_ms: rec.duration_ms,
                                    started_at: rec.started_at,
                                    ended_at: rec.ended_at,
                                    listened_ms: rec.listened_ms,
                                    completed: rec.completed,
                                    skipped: rec.skipped,
                                }) {
                                    log::warn!("failed to record listen event: {e}");
                                    Ok(())
                                } else {
                                    handle.emit("library://history", ())
                                }
                            } else {
                                Ok(())
                            }
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

            let lyrics = Arc::new(
                LyricsService::new(&cache_dir.join("lyrics"))
                    .map_err(|e| {
                        log::error!("failed to start lyrics service: {e}");
                        e
                    })?
                    .with_net_policy(app.state::<Arc<SessionStore>>().net_policy()),
            );
            app.manage(lyrics);

            let metadata = Arc::new(
                metadata::MetadataService::new()
                    .map_err(|e| {
                        log::error!("failed to start metadata service: {e}");
                        e
                    })?
                    .with_net_policy(app.state::<Arc<SessionStore>>().net_policy()),
            );
            app.manage(metadata);

            let default_downloads = app
                .path()
                .audio_dir()
                .unwrap_or_else(|_| data_dir.join("Downloads"))
                .join("NeoModern");
            let online_handle = app.handle().clone();
            let online = Arc::new(OnlineService::new(
                cache_dir.join("covers/online"),
                default_downloads,
                Arc::new(move |jobs| {
                    if let Err(e) = online_handle.emit("online://downloads", jobs) {
                        log::warn!("failed to emit download jobs: {e}");
                    }
                }),
                app.state::<Arc<SessionStore>>().net_policy(),
            ));
            app.manage(online);

            app.manage(tray::TrayStatus::default());
            tray::install(app.handle());
            Ok(())
        })
        .on_window_event(tray::on_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::queue_replace,
            commands::queue_append,
            commands::queue_play_next,
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
            commands::player_set_gapless,
            commands::player_set_wasapi_exclusive,
            commands::player_snapshot,
            commands::library_add_folder,
            commands::library_android_pick_folder,
            commands::android_pick_file,
            commands::app_file_access,
            commands::app_request_file_access,
            commands::android_open_text,
            commands::android_save_text,
            commands::library_track_cover,
            commands::library_cover_color,
            commands::library_remove_folder,
            commands::library_folders,
            commands::library_rescan,
            commands::library_stats,
            commands::library_tracks_page,
            commands::library_track_by_path,
            commands::library_albums_page,
            commands::library_album,
            commands::library_play_album,
            commands::library_play_all,
            commands::playlists_list,
            commands::playlist_create,
            commands::playlist_rename,
            commands::playlist_delete,
            commands::playlist_set_cover_file,
            commands::playlist_set_cover_preset,
            commands::playlist_clear_cover,
            commands::playlist_tracks,
            commands::playlist_add_tracks,
            commands::playlist_add_album,
            commands::playlist_remove,
            commands::playlist_move,
            commands::playlist_play,
            commands::playlist_from_queue,
            commands::playlist_export_m3u,
            commands::playlist_export_m3u_file,
            commands::playlist_import_m3u,
            commands::playlist_import_m3u_file,
            commands::app_settings,
            commands::app_window_insets,
            commands::app_log_dir,
            commands::app_reveal_file,
            commands::app_check_update,
            commands::app_open_release,
            commands::app_set_close_to_tray,
            commands::app_set_lyrics_save,
            commands::app_set_lyrics_netease,
            commands::app_set_offline_mode,
            commands::app_set_net_feature,
            commands::tags_read,
            commands::tags_write,
            commands::metadata_match_album,
            commands::metadata_match_track,
            commands::metadata_apply,
            commands::metadata_undo,
            commands::lyrics_for_track,
            commands::lyrics_attach_file,
            commands::lyrics_save_cached,
            commands::lyrics_coverage,
            commands::lyrics_share_status,
            commands::lyrics_share,
            commands::lyrics_share_cancel,
            commands::online_status,
            commands::online_set_download_dir,
            commands::online_set_ytdlp_path,
            commands::online_search,
            commands::online_album_detail,
            commands::online_play_tracks,
            commands::online_download_track,
            commands::online_download_album,
            commands::online_downloads,
            commands::online_cancel_download,
            commands::online_clear_finished,
            commands::pinned_list,
            commands::pinned_toggle,
            commands::favorites_list,
            commands::favorites_toggle,
            commands::favorites_toggle_by_path,
            commands::favorites_tracks_page,
            commands::favorites_play,
            commands::history_stats,
            commands::history_clear,
            commands::library_artists_page,
            commands::library_artist_profile,
            commands::library_play_artist,
            commands::artist_enrich,
            commands::artist_enrich_all,
            commands::artist_enrich_status,
            commands::artist_enrich_cancel,
            commands::library_home,
            commands::library_home_mix,
            commands::library_play_mix,
            commands::library_save_mix_playlist,
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
                player.flush_listen();
                session.save_now(&player.export_session());
            }
        });
}
