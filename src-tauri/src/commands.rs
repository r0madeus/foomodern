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
pub fn player_snapshot(player: State<'_, Arc<Player>>) -> Res<PlayerSnapshot> {
    Ok(player.snapshot())
}

// ---------------------------------------------------------------------
// Library
// ---------------------------------------------------------------------

use crate::library::{
    AlbumDetail, AlbumRow, FolderInfo, Library, LibraryStats, Page, SortOrder, TrackRow,
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
    query: Option<String>,
) -> Res<Page<TrackRow>> {
    le(lib.search_tracks_page(
        offset,
        limit,
        SortOrder::parse(&sort),
        query.as_deref().unwrap_or(""),
    ))
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
    start_index: usize,
    query: Option<String>,
) -> Res<PlayerSnapshot> {
    let paths = le(lib.matching_tracks(SortOrder::parse(&sort), query.as_deref().unwrap_or("")))?;
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
// Lyrics
// ---------------------------------------------------------------------

use crate::lyrics::{LyricsResult, LyricsService, attach_local_lrc};

#[tauri::command]
pub async fn lyrics_for_track(
    lib: State<'_, Arc<Library>>,
    lyrics: State<'_, Arc<LyricsService>>,
    path: String,
    duration_seconds: Option<f64>,
    force: bool,
) -> Res<LyricsResult> {
    let lib = Arc::clone(&lib);
    let lyrics = Arc::clone(&lyrics);
    lyrics
        .for_track(lib, path, duration_seconds, force)
        .await
        .map_err(|e| e.to_string())
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
