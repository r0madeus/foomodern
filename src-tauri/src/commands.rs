//! Tauri commands. Thin wrappers around [`Player`]; Rust owns all state.

use crate::audio::ReplayGain;
use crate::player::{Player, PlayerSnapshot, TrackInfo};
use crate::queue::RepeatMode;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

type Res<T> = Result<T, String>;

fn to_paths(v: Vec<String>) -> Vec<PathBuf> {
    v.into_iter().map(PathBuf::from).collect()
}

fn s<T>(r: Result<T, crate::player::PlayerError>) -> Res<T> {
    r.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn queue_replace(
    player: State<'_, Arc<Player>>,
    paths: Vec<String>,
    start_index: usize,
) -> Res<PlayerSnapshot> {
    s(player.replace_queue(to_paths(paths), start_index))
}

#[tauri::command]
pub fn queue_append(player: State<'_, Arc<Player>>, paths: Vec<String>) -> Res<PlayerSnapshot> {
    s(player.append(to_paths(paths)))
}

#[tauri::command]
pub fn queue_play_next(
    lib: State<'_, Arc<Library>>,
    player: State<'_, Arc<Player>>,
    path: String,
) -> Res<PlayerSnapshot> {
    let track = match lib.track_meta(&path).ok().flatten() {
        Some(meta) => crate::queue::Track {
            path: PathBuf::from(&path),
            title: Some(meta.title),
            artist: Some(meta.artist),
        },
        None => crate::queue::Track::from(PathBuf::from(path)),
    };
    s(player.insert_next(track))
}

#[tauri::command]
pub fn queue_remove(player: State<'_, Arc<Player>>, index: usize) -> Res<PlayerSnapshot> {
    s(player.remove(index))
}

#[tauri::command]
pub fn queue_move(player: State<'_, Arc<Player>>, from: usize, to: usize) -> Res<PlayerSnapshot> {
    s(player.move_entry(from, to))
}

#[tauri::command]
pub fn queue_page(
    player: State<'_, Arc<Player>>,
    offset: usize,
    limit: usize,
) -> Res<Vec<TrackInfo>> {
    Ok(player.queue_page(offset, limit))
}

#[tauri::command]
pub fn player_play_index(player: State<'_, Arc<Player>>, index: usize) -> Res<PlayerSnapshot> {
    s(player.play_index(index))
}

#[tauri::command]
pub fn player_play(player: State<'_, Arc<Player>>) -> Res<PlayerSnapshot> {
    s(player.play())
}

#[tauri::command]
pub fn player_pause(player: State<'_, Arc<Player>>) -> Res<PlayerSnapshot> {
    s(player.pause())
}

#[tauri::command]
pub fn player_toggle_pause(player: State<'_, Arc<Player>>) -> Res<PlayerSnapshot> {
    s(player.toggle_pause())
}

#[tauri::command]
pub fn player_next(player: State<'_, Arc<Player>>) -> Res<PlayerSnapshot> {
    s(player.next())
}

#[tauri::command]
pub fn player_previous(player: State<'_, Arc<Player>>) -> Res<PlayerSnapshot> {
    s(player.previous())
}

#[tauri::command]
pub fn player_seek(player: State<'_, Arc<Player>>, seconds: f64) -> Res<PlayerSnapshot> {
    s(player.seek(seconds))
}

#[tauri::command]
pub fn player_set_volume(player: State<'_, Arc<Player>>, percent: f64) -> Res<PlayerSnapshot> {
    s(player.set_volume(percent))
}

#[tauri::command]
pub fn player_set_shuffle(player: State<'_, Arc<Player>>, enabled: bool) -> Res<PlayerSnapshot> {
    s(player.set_shuffle(enabled))
}

#[tauri::command]
pub fn player_set_repeat(player: State<'_, Arc<Player>>, mode: RepeatMode) -> Res<PlayerSnapshot> {
    s(player.set_repeat(mode))
}

#[tauri::command]
pub fn player_set_replaygain(
    player: State<'_, Arc<Player>>,
    mode: ReplayGain,
) -> Res<PlayerSnapshot> {
    s(player.set_replaygain(mode))
}

#[tauri::command]
pub fn player_set_gapless(player: State<'_, Arc<Player>>, enabled: bool) -> Res<PlayerSnapshot> {
    s(player.set_gapless(enabled))
}

#[tauri::command]
pub fn player_set_wasapi_exclusive(
    player: State<'_, Arc<Player>>,
    enabled: bool,
) -> Res<PlayerSnapshot> {
    s(player.set_wasapi_exclusive(enabled))
}

#[tauri::command]
pub fn player_snapshot(player: State<'_, Arc<Player>>) -> Res<PlayerSnapshot> {
    Ok(player.snapshot())
}

// ---------------------------------------------------------------------
// Library
// ---------------------------------------------------------------------

use crate::library::{
    AlbumDetail, AlbumRow, FolderInfo, Library, LibraryStats, Page, SortSpec, TrackRow,
};

fn le<T>(r: Result<T, crate::library::LibraryError>) -> Res<T> {
    r.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn library_add_folder(lib: State<'_, Arc<Library>>, path: String) -> Res<Vec<FolderInfo>> {
    let id = le(lib.add_folder(&path))?;
    let folders = le(lib.folders())?;
    lib.start_scan(folders.iter().filter(|f| f.id == id).cloned().collect());
    Ok(folders)
}

/// Cached cover of the album containing `path` (the playing track in the UI).
#[tauri::command]
pub fn library_track_cover(
    lib: State<'_, Arc<Library>>,
    online: State<'_, Arc<OnlineService>>,
    path: String,
) -> Res<Option<String>> {
    let local = le(lib.track_cover(&path))?;
    if local.is_some() {
        return Ok(local);
    }
    Ok(online.stream_cover(&path))
}

/// The latest GitHub release if it is newer than this build.
#[tauri::command]
pub async fn app_check_update(
    app: tauri::AppHandle,
    store: State<'_, Arc<SessionStore>>,
) -> Res<Option<crate::update::UpdateInfo>> {
    let current = app.package_info().version.to_string();
    crate::update::check(&current, &store.net_policy()).await
}

/// Opens a release page of this project in the browser.
#[tauri::command]
pub fn app_open_release(
    app: tauri::AppHandle,
    store: State<'_, Arc<SessionStore>>,
    url: String,
) -> Res<()> {
    use tauri_plugin_opener::OpenerExt;
    store
        .net_policy()
        .require(crate::net::Feature::Updates)
        .map_err(str::to_owned)?;
    if !url.starts_with(crate::update::RELEASES_PREFIX) {
        return Err("not a NeoModern release page".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

/// Folder holding the log files (shown in Settings for bug reports).
#[tauri::command]
pub fn app_log_dir(app: tauri::AppHandle) -> Res<String> {
    use tauri::Manager;
    app.path()
        .app_log_dir()
        .map(|p| p.display().to_string())
        .map_err(|e| e.to_string())
}

/// Highlights `path` in the OS file manager (desktop only).
#[tauri::command]
pub fn app_reveal_file(app: tauri::AppHandle, path: String) -> Res<()> {
    #[cfg(target_os = "android")]
    {
        let _ = (app, path);
        Err("not supported on Android".into())
    }
    #[cfg(not(target_os = "android"))]
    {
        use tauri_plugin_opener::OpenerExt;
        if path.starts_with("http://") || path.starts_with("https://") {
            return Err("cannot reveal an online stream".into());
        }
        app.opener()
            .reveal_item_in_dir(std::path::Path::new(&path))
            .map_err(|e| e.to_string())
    }
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct WindowInsets {
    top: f64,
    bottom: f64,
}

/// Space covered by the Android status and navigation bars (CSS pixels);
/// zero on desktop.
#[tauri::command]
pub async fn app_window_insets(app: tauri::AppHandle) -> Res<WindowInsets> {
    #[cfg(target_os = "android")]
    {
        use tauri::Manager;
        let handle = app
            .state::<tauri_plugin_android_audio::AndroidAudio<tauri::Wry>>()
            .0
            .clone();
        handle
            .run_mobile_plugin_async("windowInsets", ())
            .await
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(WindowInsets::default())
    }
}

/// Dominant colour of a cached cover (phone UI tint). Only files in the
/// cover cache are read.
#[tauri::command]
pub async fn library_cover_color(app: tauri::AppHandle, cover: String) -> Res<Option<[u8; 3]>> {
    use tauri::Manager;
    let covers = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("covers");
    tauri::async_runtime::spawn_blocking(move || {
        let path = std::path::Path::new(&cover);
        let inside = match (path.canonicalize(), covers.canonicalize()) {
            (Ok(p), Ok(dir)) => p.starts_with(dir),
            _ => false,
        };
        if !inside {
            return Ok(None);
        }
        Ok(crate::library::cover_color::cover_color(path).ok())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------
// Android-only helpers (system pickers, permissions). On desktop they
// report "not available"; the UI only calls them on Android.

#[cfg(target_os = "android")]
async fn android<T: serde::de::DeserializeOwned>(
    app: &tauri::AppHandle,
    command: &str,
    payload: serde_json::Value,
) -> Res<T> {
    use tauri::Manager;
    let handle = app
        .state::<tauri_plugin_android_audio::AndroidAudio<tauri::Wry>>()
        .0
        .clone();
    handle
        .run_mobile_plugin_async(command, payload)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(not(target_os = "android"))]
async fn android<T>(app: &tauri::AppHandle, command: &str, payload: serde_json::Value) -> Res<T> {
    let _ = (app, command, payload);
    Err("only available on Android".into())
}

#[derive(serde::Deserialize)]
struct PickedPath {
    path: Option<String>,
}

/// Asks for read access to music, then lets the user pick a folder;
/// `None` when cancelled.
#[tauri::command]
pub async fn library_android_pick_folder(app: tauri::AppHandle) -> Res<Option<String>> {
    #[derive(serde::Deserialize)]
    struct Access {
        granted: bool,
    }
    let access: Access = android(&app, "requestAudioAccess", serde_json::json!({})).await?;
    if !access.granted {
        return Err("Permission to read music was denied.".into());
    }
    let picked: PickedPath = android(&app, "pickFolder", serde_json::json!({})).await?;
    Ok(picked.path)
}

/// Lets the user pick one file on device storage; its path, or `None`.
#[tauri::command]
pub async fn android_pick_file(app: tauri::AppHandle) -> Res<Option<String>> {
    let picked: PickedPath = android(&app, "pickFile", serde_json::json!({})).await?;
    Ok(picked.path)
}

/// Whether the app may write files on shared storage (always on desktop).
#[tauri::command]
pub async fn app_file_access(app: tauri::AppHandle) -> Res<bool> {
    if !cfg!(target_os = "android") {
        return Ok(true);
    }
    #[derive(serde::Deserialize)]
    struct Granted {
        granted: bool,
    }
    let g: Granted = android(&app, "fileAccessStatus", serde_json::json!({})).await?;
    Ok(g.granted)
}

/// Opens Android's "all files access" screen for this app.
#[tauri::command]
pub async fn app_request_file_access(app: tauri::AppHandle) -> Res<()> {
    let _: serde_json::Value = android(&app, "requestFileAccess", serde_json::json!({})).await?;
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct TextFile {
    name: Option<String>,
    text: Option<String>,
}

/// Android: pick a text file anywhere (system picker) and read it.
#[tauri::command]
pub async fn android_open_text(app: tauri::AppHandle) -> Res<TextFile> {
    android(&app, "openTextFile", serde_json::json!({})).await
}

/// Android: ask where to save `text` and write it; false when cancelled.
#[tauri::command]
pub async fn android_save_text(app: tauri::AppHandle, name: String, text: String) -> Res<bool> {
    #[derive(serde::Deserialize)]
    struct Saved {
        saved: bool,
    }
    let s: Saved = android(
        &app,
        "saveTextFile",
        serde_json::json!({ "name": name, "text": text }),
    )
    .await?;
    Ok(s.saved)
}

#[tauri::command]
pub fn library_remove_folder(lib: State<'_, Arc<Library>>, id: i64) -> Res<Vec<FolderInfo>> {
    le(lib.remove_folder(id))?;
    le(lib.folders())
}

#[tauri::command]
pub fn library_folders(lib: State<'_, Arc<Library>>) -> Res<Vec<FolderInfo>> {
    le(lib.folders())
}

#[tauri::command]
pub fn library_rescan(lib: State<'_, Arc<Library>>) -> Res<()> {
    lib.start_scan(le(lib.folders())?);
    Ok(())
}

#[tauri::command]
pub fn library_stats(lib: State<'_, Arc<Library>>) -> Res<LibraryStats> {
    le(lib.stats())
}

#[tauri::command]
pub fn library_tracks_page(
    lib: State<'_, Arc<Library>>,
    offset: u64,
    limit: u64,
    sort: String,
    dir: Option<String>,
    query: Option<String>,
) -> Res<Page<TrackRow>> {
    le(lib.search_tracks_page(
        offset,
        limit,
        SortSpec::parse(&sort, dir.as_deref()),
        query.as_deref().unwrap_or(""),
    ))
}

#[tauri::command]
pub fn library_track_by_path(lib: State<'_, Arc<Library>>, path: String) -> Res<Option<TrackRow>> {
    le(lib.track_row_by_path(&path))
}

#[tauri::command]
pub fn library_albums_page(
    lib: State<'_, Arc<Library>>,
    offset: u64,
    limit: u64,
    query: Option<String>,
) -> Res<Page<AlbumRow>> {
    le(lib.search_albums_page(offset, limit, query.as_deref().unwrap_or("")))
}

#[tauri::command]
pub fn library_album(lib: State<'_, Arc<Library>>, album_id: i64) -> Res<AlbumDetail> {
    le(lib.album(album_id))
}

#[tauri::command]
pub fn library_play_album(
    lib: State<'_, Arc<Library>>,
    player: State<'_, Arc<Player>>,
    album_id: i64,
    start_index: usize,
) -> Res<PlayerSnapshot> {
    let paths = le(lib.album_tracks(album_id))?;
    s(player.replace_queue(paths, start_index))
}

#[tauri::command]
pub fn library_play_all(
    lib: State<'_, Arc<Library>>,
    player: State<'_, Arc<Player>>,
    sort: String,
    dir: Option<String>,
    start_index: usize,
    query: Option<String>,
) -> Res<PlayerSnapshot> {
    let paths = le(lib.matching_tracks(
        SortSpec::parse(&sort, dir.as_deref()),
        query.as_deref().unwrap_or(""),
    ))?;
    s(player.replace_queue(paths, start_index))
}

// ---------------------------------------------------------------------
// Playlists
// ---------------------------------------------------------------------

use crate::library::{PlaylistInfo, SavedPlaylist};
use tauri::{AppHandle, Emitter};

fn playlists_changed(app: &AppHandle) {
    if let Err(e) = app.emit("playlists://changed", ()) {
        log::warn!("failed to emit playlists change event: {e}");
    }
}

#[tauri::command]
pub fn playlists_list(lib: State<'_, Arc<Library>>) -> Res<Vec<PlaylistInfo>> {
    le(lib.playlists())
}

#[tauri::command]
pub fn playlist_create(
    app: AppHandle,
    lib: State<'_, Arc<Library>>,
    name: String,
    track_ids: Option<Vec<i64>>,
) -> Res<PlaylistInfo> {
    let info = le(lib.create_playlist(&name, &track_ids.unwrap_or_default()))?;
    playlists_changed(&app);
    Ok(info)
}

#[tauri::command]
pub fn playlist_rename(
    app: AppHandle,
    lib: State<'_, Arc<Library>>,
    id: i64,
    name: String,
) -> Res<()> {
    le(lib.rename_playlist(id, &name))?;
    playlists_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn playlist_delete(app: AppHandle, lib: State<'_, Arc<Library>>, id: i64) -> Res<()> {
    le(lib.delete_playlist(id))?;
    playlists_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn playlist_set_cover_file(
    app: AppHandle,
    lib: State<'_, Arc<Library>>,
    id: i64,
    path: String,
) -> Res<()> {
    le(lib.set_playlist_cover_file(id, std::path::Path::new(&path)))?;
    playlists_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn playlist_set_cover_preset(
    app: AppHandle,
    lib: State<'_, Arc<Library>>,
    id: i64,
    preset: String,
) -> Res<()> {
    le(lib.set_playlist_cover_preset(id, &preset))?;
    playlists_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn playlist_clear_cover(app: AppHandle, lib: State<'_, Arc<Library>>, id: i64) -> Res<()> {
    le(lib.clear_playlist_cover(id))?;
    playlists_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn playlist_tracks(
    lib: State<'_, Arc<Library>>,
    id: i64,
    offset: u64,
    limit: u64,
) -> Res<Vec<TrackRow>> {
    le(lib.playlist_tracks(id, offset, limit))
}

#[tauri::command]
pub fn playlist_add_tracks(
    app: AppHandle,
    lib: State<'_, Arc<Library>>,
    id: i64,
    track_ids: Vec<i64>,
) -> Res<()> {
    le(lib.playlist_add_tracks(id, &track_ids))?;
    playlists_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn playlist_add_album(
    app: AppHandle,
    lib: State<'_, Arc<Library>>,
    id: i64,
    album_id: i64,
) -> Res<()> {
    le(lib.playlist_add_album(id, album_id))?;
    playlists_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn playlist_remove(
    app: AppHandle,
    lib: State<'_, Arc<Library>>,
    id: i64,
    positions: Vec<usize>,
) -> Res<()> {
    le(lib.playlist_remove(id, &positions))?;
    playlists_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn playlist_move(
    app: AppHandle,
    lib: State<'_, Arc<Library>>,
    id: i64,
    from: usize,
    to: usize,
) -> Res<()> {
    le(lib.playlist_move(id, from, to))?;
    playlists_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn playlist_play(
    lib: State<'_, Arc<Library>>,
    player: State<'_, Arc<Player>>,
    id: i64,
    start_index: usize,
) -> Res<PlayerSnapshot> {
    let tracks = le(lib.playlist_queue_tracks(id))?;
    s(player.replace_queue(tracks, start_index))
}

/// Saves the current queue as a playlist. Queue entries that are not in the
/// library are skipped; the result says how many.
/// The playlist as M3U text (Android saves it through the system picker).
#[tauri::command]
pub fn playlist_export_m3u(lib: State<'_, Arc<Library>>, id: i64) -> Res<String> {
    le(lib.export_m3u(id))
}

/// Writes the playlist as an M3U file (desktop, path from a save dialog).
#[tauri::command]
pub fn playlist_export_m3u_file(lib: State<'_, Arc<Library>>, id: i64, path: String) -> Res<()> {
    let text = le(lib.export_m3u(id))?;
    std::fs::write(&path, text).map_err(|e| format!("could not write {path}: {e}"))
}

/// Creates a playlist from M3U text (Android, read through the picker).
#[tauri::command]
pub fn playlist_import_m3u(
    app: AppHandle,
    lib: State<'_, Arc<Library>>,
    name: String,
    text: String,
) -> Res<crate::library::ImportedPlaylist> {
    let imported = le(lib.import_m3u(&name, &text, None))?;
    playlists_changed(&app);
    Ok(imported)
}

/// Creates a playlist from an M3U file (desktop); named after the file.
#[tauri::command]
pub fn playlist_import_m3u_file(
    app: AppHandle,
    lib: State<'_, Arc<Library>>,
    path: String,
) -> Res<crate::library::ImportedPlaylist> {
    let p = std::path::Path::new(&path);
    let bytes = std::fs::read(p).map_err(|e| format!("could not read {path}: {e}"))?;
    let text = String::from_utf8_lossy(&bytes);
    let name = p
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Imported".into());
    let imported = le(lib.import_m3u(&name, &text, p.parent()))?;
    playlists_changed(&app);
    Ok(imported)
}

#[tauri::command]
pub fn playlist_from_queue(
    app: AppHandle,
    lib: State<'_, Arc<Library>>,
    player: State<'_, Arc<Player>>,
    name: String,
) -> Res<SavedPlaylist> {
    let paths: Vec<String> = player
        .queue_page(0, usize::MAX)
        .into_iter()
        .map(|t| t.path)
        .collect();
    let saved = le(lib.playlist_from_paths(&name, &paths))?;
    playlists_changed(&app);
    Ok(saved)
}

// ---------------------------------------------------------------------
// App settings
// ---------------------------------------------------------------------

use crate::lyrics::SaveMode;
use crate::net::Feature;
use crate::session::SessionStore;
use crate::tray::TrayStatus;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// The user's choice: closing the window keeps the app in the tray.
    close_to_tray: bool,
    /// False when no tray icon could be created in this run; closing the
    /// window then quits whatever the setting says.
    tray_available: bool,
    /// `"android"` or `"desktop"`; the UI adapts its layout and dialogs.
    platform: &'static str,
    /// True on Windows, where mpv supports WASAPI exclusive output.
    wasapi_available: bool,
    /// Where lyrics found online are saved: `sidecar`, `embed` or `off`.
    lyrics_save: SaveMode,
    /// Also search NetEase Cloud Music when LRCLIB has nothing.
    lyrics_netease: bool,
    /// Master switch: nothing contacts the internet.
    offline_mode: bool,
    /// The user's own per-feature switches (kept while offline mode is on).
    net_lyrics: bool,
    net_online: bool,
    net_artist_info: bool,
    net_metadata: bool,
    net_updates: bool,
    /// What is really allowed right now (offline mode applied). The UI
    /// uses these to hide or disable controls; Rust enforces them anyway.
    allow_lyrics: bool,
    allow_online: bool,
    allow_artist_info: bool,
    allow_metadata: bool,
    allow_updates: bool,
}

fn app_settings_now(store: &SessionStore, tray: &TrayStatus) -> AppSettings {
    AppSettings {
        close_to_tray: store.close_to_tray(),
        tray_available: tray.available(),
        platform: if cfg!(target_os = "android") {
            "android"
        } else {
            "desktop"
        },
        wasapi_available: cfg!(target_os = "windows"),
        lyrics_save: store.lyrics_save(),
        lyrics_netease: store.lyrics_netease(),
        offline_mode: store.offline_mode(),
        net_lyrics: store.net_feature(Feature::Lyrics),
        net_online: store.net_feature(Feature::Online),
        net_artist_info: store.net_feature(Feature::ArtistInfo),
        net_metadata: store.net_feature(Feature::Metadata),
        net_updates: store.net_feature(Feature::Updates),
        allow_lyrics: store.net_allowed(Feature::Lyrics),
        allow_online: store.net_allowed(Feature::Online),
        allow_artist_info: store.net_allowed(Feature::ArtistInfo),
        allow_metadata: store.net_allowed(Feature::Metadata),
        allow_updates: store.net_allowed(Feature::Updates),
    }
}

#[tauri::command]
pub fn app_settings(
    store: State<'_, Arc<SessionStore>>,
    tray: State<'_, TrayStatus>,
) -> AppSettings {
    app_settings_now(&store, &tray)
}

#[tauri::command]
pub fn app_set_close_to_tray(
    store: State<'_, Arc<SessionStore>>,
    tray: State<'_, TrayStatus>,
    player: State<'_, Arc<Player>>,
    enabled: bool,
) -> AppSettings {
    store.set_close_to_tray(enabled);
    store.save_now(&player.export_session());
    app_settings_now(&store, &tray)
}

#[tauri::command]
pub fn app_set_lyrics_save(
    store: State<'_, Arc<SessionStore>>,
    tray: State<'_, TrayStatus>,
    player: State<'_, Arc<Player>>,
    mode: SaveMode,
) -> AppSettings {
    store.set_lyrics_save(mode);
    store.save_now(&player.export_session());
    app_settings_now(&store, &tray)
}

#[tauri::command]
pub fn app_set_lyrics_netease(
    store: State<'_, Arc<SessionStore>>,
    tray: State<'_, TrayStatus>,
    player: State<'_, Arc<Player>>,
    enabled: bool,
) -> AppSettings {
    store.set_lyrics_netease(enabled);
    store.save_now(&player.export_session());
    app_settings_now(&store, &tray)
}

#[tauri::command]
pub fn app_set_offline_mode(
    store: State<'_, Arc<SessionStore>>,
    tray: State<'_, TrayStatus>,
    player: State<'_, Arc<Player>>,
    enabled: bool,
) -> AppSettings {
    store.set_offline_mode(enabled);
    store.save_now(&player.export_session());
    app_settings_now(&store, &tray)
}

/// Switches one internet feature (`lyrics`, `online`, `artistInfo`,
/// `metadata`, `updates`).
#[tauri::command]
pub fn app_set_net_feature(
    store: State<'_, Arc<SessionStore>>,
    tray: State<'_, TrayStatus>,
    player: State<'_, Arc<Player>>,
    feature: Feature,
    enabled: bool,
) -> AppSettings {
    store.set_net_feature(feature, enabled);
    store.save_now(&player.export_session());
    app_settings_now(&store, &tray)
}

// ---------------------------------------------------------------------
// Tag editing
// ---------------------------------------------------------------------

use crate::library::tags::{self, EditableTags, TagChanges, TagFailure};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackTags {
    id: i64,
    path: String,
    #[serde(flatten)]
    tags: EditableTags,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagsRead {
    tracks: Vec<TrackTags>,
    /// Value shared by all tracks per field; null when empty or mixed.
    common: EditableTags,
    /// Names of the fields whose values differ between the tracks.
    mixed: Vec<&'static str>,
    failed: Vec<TagFailure>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagsWritten {
    updated: usize,
    failed: Vec<TagFailure>,
}

#[tauri::command]
pub async fn tags_read(lib: State<'_, Arc<Library>>, track_ids: Vec<i64>) -> Res<TagsRead> {
    let lib = Arc::clone(&lib);
    tokio::task::spawn_blocking(move || {
        let (read, failed) = le(tags::read_for_tracks(&lib, &track_ids))?;
        let all: Vec<EditableTags> = read.iter().map(|t| t.tags.clone()).collect();
        let common = tags::common_tags(&all);
        Ok(TagsRead {
            tracks: read
                .into_iter()
                .map(|t| TrackTags {
                    id: t.id,
                    path: t.path,
                    tags: t.tags,
                })
                .collect(),
            common: common.values,
            mixed: common.mixed,
            failed,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn tags_write(
    lib: State<'_, Arc<Library>>,
    player: State<'_, Arc<Player>>,
    track_ids: Vec<i64>,
    changes: TagChanges,
) -> Res<TagsWritten> {
    let lib = Arc::clone(&lib);
    let player = Arc::clone(&player);
    tokio::task::spawn_blocking(move || {
        let current = player.current_path();
        let outcome = le(tags::write_for_tracks(
            &lib,
            &track_ids,
            &changes,
            current.as_deref(),
        ))?;
        // Show the edit in the player bar and queue as well.
        for path in &outcome.updated {
            match lib.track_meta(path) {
                Ok(Some(meta)) => {
                    player.update_track_meta(path, &meta.title, &meta.artist);
                }
                Ok(None) => {}
                Err(e) => log::warn!("could not read back {path}: {e}"),
            }
        }
        Ok(TagsWritten {
            updated: outcome.updated.len(),
            failed: outcome.failed,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------
// Metadata matching (MusicBrainz)
// ---------------------------------------------------------------------

use crate::metadata::{AlbumCandidate, ApplyOutcome, MetadataService, Proposal, TrackCandidate};

#[tauri::command]
pub async fn metadata_match_album(
    lib: State<'_, Arc<Library>>,
    metadata: State<'_, Arc<MetadataService>>,
    album_id: i64,
) -> Res<Vec<AlbumCandidate>> {
    let (lib, metadata) = (Arc::clone(&lib), Arc::clone(&metadata));
    metadata
        .match_album(&lib, album_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn metadata_match_track(
    lib: State<'_, Arc<Library>>,
    metadata: State<'_, Arc<MetadataService>>,
    track_id: i64,
) -> Res<Vec<TrackCandidate>> {
    let (lib, metadata) = (Arc::clone(&lib), Arc::clone(&metadata));
    metadata
        .match_track(&lib, track_id)
        .await
        .map_err(|e| e.to_string())
}

/// Shows written tags in the player bar and queue as well.
fn refresh_player_meta(lib: &Library, player: &Player, outcome: &ApplyOutcome) {
    for path in &outcome.updated_paths {
        match lib.track_meta(path) {
            Ok(Some(meta)) => {
                player.update_track_meta(path, &meta.title, &meta.artist);
            }
            Ok(None) => {}
            Err(e) => log::warn!("could not read back {path}: {e}"),
        }
    }
}

#[tauri::command]
pub async fn metadata_apply(
    lib: State<'_, Arc<Library>>,
    player: State<'_, Arc<Player>>,
    metadata: State<'_, Arc<MetadataService>>,
    proposal: Proposal,
) -> Res<ApplyOutcome> {
    let (lib, player, metadata) = (Arc::clone(&lib), Arc::clone(&player), Arc::clone(&metadata));
    // Never hold the player lock across the network or disk work.
    let current = player.current_path();
    let outcome = metadata
        .apply(&lib, proposal, current)
        .await
        .map_err(|e| e.to_string())?;
    refresh_player_meta(&lib, &player, &outcome);
    Ok(outcome)
}

#[tauri::command]
pub async fn metadata_undo(
    lib: State<'_, Arc<Library>>,
    player: State<'_, Arc<Player>>,
    metadata: State<'_, Arc<MetadataService>>,
    batch_id: i64,
) -> Res<ApplyOutcome> {
    let (lib, player, metadata) = (Arc::clone(&lib), Arc::clone(&player), Arc::clone(&metadata));
    let current = player.current_path();
    let outcome = metadata
        .undo(&lib, batch_id, current)
        .await
        .map_err(|e| e.to_string())?;
    refresh_player_meta(&lib, &player, &outcome);
    Ok(outcome)
}

// ---------------------------------------------------------------------
// Lyrics
// ---------------------------------------------------------------------

use crate::lyrics::{
    BulkSaveOutcome, Coverage, LyricsOptions, LyricsResult, LyricsService, ShareOutcome,
    ShareStatus, attach_local_lrc,
};

#[tauri::command]
pub async fn lyrics_for_track(
    lib: State<'_, Arc<Library>>,
    lyrics: State<'_, Arc<LyricsService>>,
    store: State<'_, Arc<SessionStore>>,
    player: State<'_, Arc<Player>>,
    path: String,
    duration_seconds: Option<f64>,
    force: bool,
) -> Res<LyricsResult> {
    let lib = Arc::clone(&lib);
    let lyrics = Arc::clone(&lyrics);
    let opts = LyricsOptions::new(
        store.lyrics_save(),
        store.lyrics_netease() && store.net_allowed(Feature::Lyrics),
        player.current_path(),
    );
    lyrics
        .for_track_with(lib, path, duration_seconds, force, &opts)
        .await
        .map_err(|e| e.to_string())
}

/// Writes every cached (found) lyrics of the library with the chosen save
/// mode. Returns when done; the UI shows the progress state meanwhile.
#[tauri::command]
pub async fn lyrics_save_cached(
    lib: State<'_, Arc<Library>>,
    lyrics: State<'_, Arc<LyricsService>>,
    store: State<'_, Arc<SessionStore>>,
    player: State<'_, Arc<Player>>,
) -> Res<BulkSaveOutcome> {
    let (lib, lyrics) = (Arc::clone(&lib), Arc::clone(&lyrics));
    let (mode, current) = (store.lyrics_save(), player.current_path());
    tokio::task::spawn_blocking(move || lyrics.save_cached_blocking(&lib, mode, current.as_deref()))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn lyrics_coverage(
    lib: State<'_, Arc<Library>>,
    lyrics: State<'_, Arc<LyricsService>>,
) -> Res<Coverage> {
    let (lib, lyrics) = (Arc::clone(&lib), Arc::clone(&lyrics));
    tokio::task::spawn_blocking(move || lyrics.coverage_blocking(&lib))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn lyrics_share_status(
    lib: State<'_, Arc<Library>>,
    lyrics: State<'_, Arc<LyricsService>>,
    path: String,
    duration_seconds: Option<f64>,
) -> Res<ShareStatus> {
    let (lib, lyrics) = (Arc::clone(&lib), Arc::clone(&lyrics));
    lyrics
        .share_status(lib, path, duration_seconds)
        .await
        .map_err(|e| e.to_string())
}

/// Publishes the track's own lyrics on LRCLIB (can take minutes).
#[tauri::command]
pub async fn lyrics_share(
    lib: State<'_, Arc<Library>>,
    lyrics: State<'_, Arc<LyricsService>>,
    path: String,
    duration_seconds: Option<f64>,
) -> Res<ShareOutcome> {
    let (lib, lyrics) = (Arc::clone(&lib), Arc::clone(&lyrics));
    lyrics
        .share(lib, path, duration_seconds)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn lyrics_share_cancel(lyrics: State<'_, Arc<LyricsService>>) {
    lyrics.cancel_share();
}

#[tauri::command]
pub async fn lyrics_attach_file(audio_path: String, lrc_path: String) -> Res<LyricsResult> {
    tokio::task::spawn_blocking(move || {
        attach_local_lrc(
            std::path::Path::new(&audio_path),
            std::path::Path::new(&lrc_path),
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------
// Online search & downloads
// ---------------------------------------------------------------------

use crate::online::{
    DownloadJob, DownloadKind, DownloadRequest, OnlineAlbum, OnlineAlbumDetail, OnlineService,
    OnlineStatus, OnlineTrack, SearchResults,
};
use crate::queue::Track;

#[tauri::command]
pub async fn online_status(
    store: State<'_, Arc<SessionStore>>,
    lib: State<'_, Arc<Library>>,
    online: State<'_, Arc<OnlineService>>,
) -> Res<OnlineStatus> {
    let store = Arc::clone(&store);
    let lib = Arc::clone(&lib);
    let online = Arc::clone(&online);
    tokio::task::spawn_blocking(move || online.status(&store, &lib))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn online_set_download_dir(
    store: State<'_, Arc<SessionStore>>,
    player: State<'_, Arc<Player>>,
    lib: State<'_, Arc<Library>>,
    online: State<'_, Arc<OnlineService>>,
    dir: Option<String>,
) -> Res<OnlineStatus> {
    store.set_download_dir(dir.map(PathBuf::from));
    store.save_now(&player.export_session());
    let store = Arc::clone(&store);
    let lib = Arc::clone(&lib);
    let online = Arc::clone(&online);
    tokio::task::spawn_blocking(move || online.status(&store, &lib))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn online_set_ytdlp_path(
    store: State<'_, Arc<SessionStore>>,
    player: State<'_, Arc<Player>>,
    lib: State<'_, Arc<Library>>,
    online: State<'_, Arc<OnlineService>>,
    path: Option<String>,
) -> Res<OnlineStatus> {
    store.set_ytdlp_path(path.map(PathBuf::from));
    store.save_now(&player.export_session());
    let store = Arc::clone(&store);
    let lib = Arc::clone(&lib);
    let online = Arc::clone(&online);
    tokio::task::spawn_blocking(move || online.status(&store, &lib))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn online_search(
    store: State<'_, Arc<SessionStore>>,
    online: State<'_, Arc<OnlineService>>,
    query: String,
) -> Res<SearchResults> {
    let bin = online.resolve_ytdlp_bin(&store);
    online.search(&query, &bin).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn online_album_detail(
    store: State<'_, Arc<SessionStore>>,
    online: State<'_, Arc<OnlineService>>,
    album: OnlineAlbum,
) -> Res<OnlineAlbumDetail> {
    let bin = online.resolve_ytdlp_bin(&store);
    online
        .album_detail(&album, &bin)
        .await
        .map_err(|e| e.to_string())
}

fn online_queue_entry(track: OnlineTrack, stream_url: String) -> Track {
    Track {
        path: PathBuf::from(stream_url),
        title: (!track.title.is_empty()).then_some(track.title),
        artist: (!track.artist.is_empty()).then_some(track.artist),
    }
}

/// Plays `tracks[start_index]` with the rest of the list (an album or search
/// results) after it, like playing from a local list. Only the clicked track
/// is resolved before playback starts; the following ones are resolved in
/// the background and inserted in order. That stops as soon as the queue no
/// longer holds the last inserted entry (the user replaced the queue).
#[tauri::command]
pub async fn online_play_tracks(
    store: State<'_, Arc<SessionStore>>,
    online: State<'_, Arc<OnlineService>>,
    player: State<'_, Arc<Player>>,
    tracks: Vec<OnlineTrack>,
    start_index: usize,
) -> Res<PlayerSnapshot> {
    let mut rest = tracks.into_iter().skip(start_index);
    let first = rest.next().ok_or("no track to play")?;
    let bin = online.resolve_ytdlp_bin(&store);
    let (stream_url, _) = online
        .resolve_stream(&first, &bin)
        .await
        .map_err(|e| e.to_string())?;
    let snap = s(player.replace_queue(vec![online_queue_entry(first, stream_url)], 0))?;
    let Some(mut anchor) = snap.current.as_ref().map(|c| c.id) else {
        return Ok(snap);
    };
    let rest: Vec<OnlineTrack> = rest.collect();
    if !rest.is_empty() {
        let online = Arc::clone(&online);
        let player = Arc::clone(&player);
        tauri::async_runtime::spawn(async move {
            for track in rest {
                let title = track.title.clone();
                let stream_url = match online.resolve_stream(&track, &bin).await {
                    Ok((url, _)) => url,
                    Err(e) => {
                        log::warn!("skipping '{title}': {e}");
                        continue;
                    }
                };
                match player.insert_after(anchor, online_queue_entry(track, stream_url)) {
                    Ok(Some(id)) => anchor = id,
                    Ok(None) => break,
                    Err(e) => {
                        log::warn!("could not queue '{title}': {e}");
                        break;
                    }
                }
            }
        });
    }
    Ok(snap)
}

#[tauri::command]
pub fn online_download_track(
    store: State<'_, Arc<SessionStore>>,
    lib: State<'_, Arc<Library>>,
    online: State<'_, Arc<OnlineService>>,
    track: OnlineTrack,
) -> Res<DownloadJob> {
    let title = track.title.clone();
    let artist = track.artist.clone();
    let cover = track.cover.clone().or_else(|| track.cover_url.clone());
    let req = DownloadRequest {
        kind: DownloadKind::Track,
        title,
        artist,
        cover,
        tracks: vec![track],
    };
    let res = online.enqueue_download(req, &store, &lib);
    res.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn online_download_album(
    store: State<'_, Arc<SessionStore>>,
    lib: State<'_, Arc<Library>>,
    online: State<'_, Arc<OnlineService>>,
    album: OnlineAlbum,
    tracks: Vec<OnlineTrack>,
) -> Res<DownloadJob> {
    let cover = album.cover.or(album.cover_url);
    let req = DownloadRequest {
        kind: DownloadKind::Album,
        title: album.title,
        artist: album.artist,
        cover,
        tracks,
    };
    let res = online.enqueue_download(req, &store, &lib);
    res.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn online_downloads(online: State<'_, Arc<OnlineService>>) -> Vec<DownloadJob> {
    online.downloads()
}

#[tauri::command]
pub fn online_cancel_download(online: State<'_, Arc<OnlineService>>, id: u64) -> Vec<DownloadJob> {
    online
        .cancel_download(id)
        .unwrap_or_else(|| online.downloads())
}

#[tauri::command]
pub fn online_clear_finished(online: State<'_, Arc<OnlineService>>) -> Vec<DownloadJob> {
    online.clear_finished()
}

// ---------------------------------------------------------------------
// Sidebar pinned items (albums & playlists)
// ---------------------------------------------------------------------

use crate::session::{PinnedKind, PinnedRef};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedEntry {
    pub kind: PinnedKind,
    pub id: i64,
    pub title: String,
    pub subtitle: String,
    pub cover: Option<String>,
    pub album_covers: Vec<String>,
    pub track_count: u32,
}

fn resolve_pinned(store: &SessionStore, lib: &Library) -> Res<Vec<PinnedEntry>> {
    let refs = store.pinned();
    let mut playlists_cache = None;
    let mut out = Vec::with_capacity(refs.len());
    for item in refs {
        match item.kind {
            PinnedKind::Album => {
                if let Ok(detail) = lib.album(item.id) {
                    out.push(PinnedEntry {
                        kind: PinnedKind::Album,
                        id: detail.album.id,
                        title: detail.album.title,
                        subtitle: detail.album.album_artist,
                        cover: detail.album.cover,
                        album_covers: Vec::new(),
                        track_count: detail.album.track_count,
                    });
                }
            }
            PinnedKind::Playlist => {
                let list = match &playlists_cache {
                    Some(v) => v,
                    None => playlists_cache.insert(le(lib.playlists())?),
                };
                if let Some(p) = list.iter().find(|p| p.id == item.id) {
                    out.push(PinnedEntry {
                        kind: PinnedKind::Playlist,
                        id: p.id,
                        title: p.name.clone(),
                        subtitle: String::new(),
                        cover: p.cover.clone(),
                        album_covers: p.album_covers.clone(),
                        track_count: p.track_count,
                    });
                }
            }
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn pinned_list(
    store: State<'_, Arc<SessionStore>>,
    lib: State<'_, Arc<Library>>,
) -> Res<Vec<PinnedEntry>> {
    resolve_pinned(&store, &lib)
}

#[tauri::command]
pub fn pinned_toggle(
    app: AppHandle,
    store: State<'_, Arc<SessionStore>>,
    player: State<'_, Arc<Player>>,
    lib: State<'_, Arc<Library>>,
    kind: PinnedKind,
    id: i64,
) -> Res<Vec<PinnedEntry>> {
    store.toggle_pin(PinnedRef { kind, id });
    store.save_now(&player.export_session());
    if let Err(e) = app.emit("pinned://changed", ()) {
        log::warn!("failed to emit pinned change event: {e}");
    }
    resolve_pinned(&store, &lib)
}

// ---------------------------------------------------------------------
// Favorites (liked tracks)
// ---------------------------------------------------------------------

use crate::library::FavoriteRef;

fn favorites_changed(app: &AppHandle) {
    if let Err(e) = app.emit("favorites://changed", ()) {
        log::warn!("failed to emit favorites change event: {e}");
    }
}

#[tauri::command]
pub fn favorites_list(lib: State<'_, Arc<Library>>) -> Res<Vec<FavoriteRef>> {
    le(lib.favorites())
}

#[tauri::command]
pub fn favorites_toggle(app: AppHandle, lib: State<'_, Arc<Library>>, track_id: i64) -> Res<bool> {
    let added = le(lib.toggle_favorite(track_id))?;
    favorites_changed(&app);
    Ok(added)
}

#[tauri::command]
pub fn favorites_toggle_by_path(
    app: AppHandle,
    lib: State<'_, Arc<Library>>,
    path: String,
) -> Res<bool> {
    let added = le(lib.toggle_favorite_by_path(&path))?;
    favorites_changed(&app);
    Ok(added)
}

#[tauri::command]
pub fn favorites_tracks_page(
    lib: State<'_, Arc<Library>>,
    offset: u64,
    limit: u64,
) -> Res<Page<TrackRow>> {
    le(lib.favorites_page(offset, limit))
}

#[tauri::command]
pub fn favorites_play(
    lib: State<'_, Arc<Library>>,
    player: State<'_, Arc<Player>>,
    start_index: usize,
) -> Res<PlayerSnapshot> {
    let tracks = le(lib.favorites_queue_tracks())?;
    s(player.replace_queue(tracks, start_index))
}

// ---------------------------------------------------------------------
// Listening history & stats
// ---------------------------------------------------------------------

#[tauri::command]
pub fn history_stats(
    lib: State<'_, Arc<Library>>,
    from: Option<i64>,
    to: Option<i64>,
) -> Res<crate::library::ListeningStats> {
    le(lib.listening_stats(from, to))
}

#[tauri::command]
pub fn history_clear(app: AppHandle, lib: State<'_, Arc<Library>>) -> Res<u64> {
    let removed = le(lib.clear_listen_history())?;
    let _ = app.emit("library://history", ());
    Ok(removed)
}

// ---------------------------------------------------------------------
// Artists & profiles
// ---------------------------------------------------------------------

#[tauri::command]
pub fn library_artists_page(
    lib: State<'_, Arc<Library>>,
    offset: u64,
    limit: u64,
    query: Option<String>,
) -> Res<Page<crate::library::ArtistRow>> {
    le(lib.search_artists_page(offset, limit, query.as_deref().unwrap_or("")))
}

#[tauri::command]
pub fn library_artist_profile(
    lib: State<'_, Arc<Library>>,
    name: String,
) -> Res<crate::library::ArtistProfile> {
    le(lib.artist_profile(&name))
}

#[tauri::command]
pub fn library_play_artist(
    lib: State<'_, Arc<Library>>,
    player: State<'_, Arc<Player>>,
    name: String,
    start_index: usize,
) -> Res<PlayerSnapshot> {
    let tracks = le(lib.artist_tracks(&name))?;
    s(player.replace_queue(tracks, start_index))
}

#[tauri::command]
pub async fn artist_enrich(
    lib: State<'_, Arc<Library>>,
    online: State<'_, Arc<OnlineService>>,
    name: String,
    lang: Option<String>,
) -> Res<crate::online::ArtistOnlineInfo> {
    online
        .artist_info(&lib, &name, lang.as_deref() == Some("tr"))
        .await
}

#[tauri::command]
pub async fn artist_enrich_all(
    app: AppHandle,
    lib: State<'_, Arc<Library>>,
    online: State<'_, Arc<OnlineService>>,
    force: bool,
    lang: Option<String>,
) -> Res<crate::online::ArtistScanProgress> {
    let lib = Arc::clone(&lib);
    let online = Arc::clone(&online);
    tokio::task::spawn_blocking(move || {
        online.start_artist_scan(app, lib, force, lang.as_deref() == Some("tr"))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn artist_enrich_status(
    online: State<'_, Arc<OnlineService>>,
) -> crate::online::ArtistScanProgress {
    online.artist_scan_status()
}

#[tauri::command]
pub fn artist_enrich_cancel(online: State<'_, Arc<OnlineService>>) {
    online.cancel_artist_scan();
}

// ---------------------------------------------------------------------
// Home feed & dynamic mixes
// ---------------------------------------------------------------------

#[tauri::command]
pub async fn library_home(lib: State<'_, Arc<Library>>) -> Res<crate::library::HomeFeed> {
    let lib = Arc::clone(&lib);
    tokio::task::spawn_blocking(move || le(lib.home_feed()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn library_home_mix(
    lib: State<'_, Arc<Library>>,
    mix_id: String,
) -> Res<Option<crate::library::HomeMixDetail>> {
    let lib = Arc::clone(&lib);
    tokio::task::spawn_blocking(move || le(lib.home_mix_detail(&mix_id)))
        .await
        .map_err(|e| e.to_string())?
}

/// Plays a home mix. The mix is recomputed here, so the UI names the track
/// it showed (`start_path`) instead of an index; the queue is only replaced
/// when the mix is still available and contains that track.
#[tauri::command]
pub async fn library_play_mix(
    lib: State<'_, Arc<Library>>,
    player: State<'_, Arc<Player>>,
    mix_id: String,
    start_path: Option<String>,
) -> Res<PlayerSnapshot> {
    let lib = Arc::clone(&lib);
    let player = Arc::clone(&player);
    tokio::task::spawn_blocking(move || {
        let tracks = le(lib.home_mix_queue_tracks(&mix_id))?;
        let start = crate::library::db::resolve_mix_start(&tracks, start_path.as_deref())?;
        s(player.replace_queue(tracks, start))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn library_save_mix_playlist(
    app: AppHandle,
    lib: State<'_, Arc<Library>>,
    mix_id: String,
    name: String,
) -> Res<PlaylistInfo> {
    let info = le(lib.save_mix_as_playlist(&mix_id, &name))?;
    playlists_changed(&app);
    Ok(info)
}
