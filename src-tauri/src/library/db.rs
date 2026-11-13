//! SQLite schema and queries. All functions take a plain `&Connection` so
//! they can be tested without the [`super::Library`] facade.

use super::{
    AlbumDetail, AlbumRow, BucketStat, FavoriteRef, FolderInfo, ListeningStats, NewListenEvent,
    Page, PlaylistInfo, RecentListen, Result, StatsSummary, TopAlbumStat, TopArtistStat,
    TopGenreStat, TopTrackStat, TrackMeta, TrackRow,
};
use crate::queue::Track;
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, Row, params_from_iter};
use std::path::PathBuf;

const SCHEMA_VERSION: i64 = 6;

pub(super) const SCHEMA_V1: &str = "
CREATE TABLE folders (
    id   INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE
);
CREATE TABLE albums (
    id           INTEGER PRIMARY KEY,
    title        TEXT NOT NULL COLLATE NOCASE,
    album_artist TEXT NOT NULL COLLATE NOCASE,
    year         INTEGER,
    cover        TEXT,
    UNIQUE (title, album_artist)
);
CREATE TABLE tracks (
    id          INTEGER PRIMARY KEY,
    folder_id   INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    path        TEXT NOT NULL UNIQUE,
    mtime       INTEGER NOT NULL,
    size        INTEGER NOT NULL,
    title       TEXT NOT NULL COLLATE NOCASE,
    artist      TEXT NOT NULL COLLATE NOCASE,
    album_id    INTEGER REFERENCES albums(id),
    track_no    INTEGER,
    disc_no     INTEGER,
    year        INTEGER,
    duration_ms INTEGER,
    genre       TEXT,
    added_at    INTEGER NOT NULL
);
CREATE INDEX tracks_folder   ON tracks(folder_id);
CREATE INDEX tracks_album    ON tracks(album_id, disc_no, track_no, title);
CREATE INDEX tracks_artist   ON tracks(artist, title);
CREATE INDEX tracks_title    ON tracks(title);
CREATE INDEX tracks_added    ON tracks(added_at DESC);
";

pub(super) const SCHEMA_V2: &str = "
CREATE TABLE playlists (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE TABLE playlist_tracks (
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    track_id    INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    PRIMARY KEY (playlist_id, position)
);
CREATE INDEX playlist_tracks_track ON playlist_tracks(track_id);
";

const SCHEMA_V3: &str = "
CREATE TABLE albums_v3 (
    id           INTEGER PRIMARY KEY,
    title        TEXT NOT NULL COLLATE NOCASE,
    album_artist TEXT NOT NULL COLLATE NOCASE,
    year         INTEGER,
    cover        TEXT
);
INSERT INTO albums_v3(id, title, album_artist, year, cover)
SELECT id, title, album_artist, year, cover FROM albums;
DROP TABLE albums;
ALTER TABLE albums_v3 RENAME TO albums;
CREATE UNIQUE INDEX albums_key ON albums(title, album_artist, COALESCE(year, -1));
INSERT OR IGNORE INTO albums(title, album_artist, year, cover)
SELECT DISTINCT a.title, a.album_artist, t.year, a.cover
FROM tracks t
JOIN albums a ON a.id = t.album_id
WHERE COALESCE(t.year, -1) != COALESCE(a.year, -1);
UPDATE tracks
SET album_id = (
    SELECT target.id
    FROM albums cur
    JOIN albums target
      ON target.title = cur.title
     AND target.album_artist = cur.album_artist
     AND COALESCE(target.year, -1) = COALESCE(tracks.year, -1)
    WHERE cur.id = tracks.album_id
)
WHERE album_id IS NOT NULL
  AND COALESCE(year, -1) != (
      SELECT COALESCE(a.year, -1) FROM albums a WHERE a.id = tracks.album_id
  );
";

const SCHEMA_V4: &str = "
CREATE TABLE favorites (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    added_at INTEGER NOT NULL
);
CREATE INDEX favorites_added ON favorites(added_at DESC);
";

const SCHEMA_V5: &str = "
ALTER TABLE playlists ADD COLUMN cover TEXT;
";

const SCHEMA_V6: &str = "
CREATE TABLE listen_events (
    id          INTEGER PRIMARY KEY,
    track_id    INTEGER REFERENCES tracks(id) ON DELETE SET NULL,
    path        TEXT NOT NULL,
    title       TEXT NOT NULL COLLATE NOCASE,
    artist      TEXT NOT NULL COLLATE NOCASE,
    album       TEXT NOT NULL COLLATE NOCASE,
    genre       TEXT NOT NULL COLLATE NOCASE,
    duration_ms INTEGER,
    started_at  INTEGER NOT NULL,
    ended_at    INTEGER NOT NULL,
    listened_ms INTEGER NOT NULL,
    completed   INTEGER NOT NULL DEFAULT 0,
    skipped     INTEGER NOT NULL DEFAULT 0,
    source      TEXT NOT NULL DEFAULT 'local'
);
CREATE INDEX listen_events_started ON listen_events(started_at DESC);
CREATE INDEX listen_events_artist ON listen_events(artist, started_at);
CREATE INDEX listen_events_track ON listen_events(track_id);
";

/// Opens-time setup: pragmas and migrations.
pub fn init(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(super::LibraryError::Other(format!(
            "library database version {version} is newer than supported {SCHEMA_VERSION}"
        )));
    }
    if version < 1 {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(SCHEMA_V1)?;
        tx.pragma_update(None, "user_version", 1)?;
        tx.commit()?;
    }
    if version < 2 {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(SCHEMA_V2)?;
        tx.pragma_update(None, "user_version", 2)?;
        tx.commit()?;
    }
    if version < 3 {
        // Dropping `albums` to replace its UNIQUE constraint requires
        // `foreign_keys = OFF` outside the transaction; FKs are re-enabled
        // before returning.
        conn.pragma_update(None, "foreign_keys", false)?;
        let res = (|| -> Result<()> {
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(SCHEMA_V3)?;
            delete_empty_albums(&tx)?;
            tx.pragma_update(None, "user_version", 3)?;
            tx.commit()?;
            Ok(())
        })();
        conn.pragma_update(None, "foreign_keys", true)?;
        res?;
    }
    if version < 4 {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(SCHEMA_V4)?;
        tx.pragma_update(None, "user_version", 4)?;
        tx.commit()?;
    }
    if version < 5 {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(SCHEMA_V5)?;
        tx.pragma_update(None, "user_version", 5)?;
        tx.commit()?;
    }
    if version < 6 {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(SCHEMA_V6)?;
        tx.pragma_update(None, "user_version", 6)?;
        tx.commit()?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Artist,
    Title,
    Album,
    Added,
}

impl SortOrder {
    /// Unknown names fall back to the default (`Artist`).
    pub fn parse(s: &str) -> Self {
        match s {
            "title" => Self::Title,
            "album" => Self::Album,
            "added" => Self::Added,
            _ => Self::Artist,
        }
    }

    fn order_by(self) -> &'static str {
        match self {
            Self::Artist => "t.artist, a.title, a.year, a.id, t.disc_no, t.track_no, t.title, t.id",
            Self::Title => "t.title, t.artist, t.id",
            Self::Album => {
                "a.title, a.album_artist, a.year, a.id, t.disc_no, t.track_no, t.title, t.id"
            }
            Self::Added => "t.added_at DESC, t.id",
        }
    }
}

pub fn folders(conn: &Connection) -> Result<Vec<FolderInfo>> {
    let mut st = conn.prepare("SELECT id, path FROM folders ORDER BY id")?;
    let rows = st.query_map([], |r| {
        Ok(FolderInfo {
            id: r.get(0)?,
            path: r.get(1)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub fn add_folder(conn: &Connection, path: &str) -> Result<i64> {
    conn.execute(
        "INSERT INTO folders(path) VALUES (?1) ON CONFLICT(path) DO NOTHING",
        [path],
    )?;
    Ok(
        conn.query_row("SELECT id FROM folders WHERE path = ?1", [path], |r| {
            r.get(0)
        })?,
    )
}

/// Deletes a folder (tracks cascade) and returns cover files of albums that
/// became empty; the caller deletes the files.
pub fn remove_folder(conn: &mut Connection, id: i64) -> Result<Vec<String>> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM folders WHERE id = ?1", [id])?;
    let covers = delete_empty_albums(&tx)?;
    tx.commit()?;
    Ok(covers)
}

/// Deletes albums without tracks; returns their cover paths (excluding any
/// still referenced by another non-empty album).
pub fn delete_empty_albums(conn: &Connection) -> Result<Vec<String>> {
    let mut st = conn.prepare(
        "SELECT DISTINCT cover FROM albums WHERE cover IS NOT NULL
         AND NOT EXISTS (SELECT 1 FROM tracks WHERE album_id = albums.id)
         AND NOT EXISTS (
             SELECT 1 FROM albums other
             WHERE other.cover = albums.cover
               AND EXISTS (SELECT 1 FROM tracks WHERE album_id = other.id)
         )",
    )?;
    let covers = st
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    conn.execute(
        "DELETE FROM albums WHERE NOT EXISTS (SELECT 1 FROM tracks WHERE album_id = albums.id)",
        [],
    )?;
    Ok(covers)
}

pub fn counts(conn: &Connection) -> Result<(u64, u64)> {
    let tracks: i64 = conn.query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))?;
    let albums: i64 = conn.query_row("SELECT COUNT(*) FROM albums", [], |r| r.get(0))?;
    Ok((tracks as u64, albums as u64))
}

const TRACK_SELECT: &str = "SELECT t.id, t.title, t.artist, COALESCE(a.title, ''), t.album_id,
        t.track_no, t.disc_no, t.year, t.duration_ms, t.path, a.cover
     FROM tracks t LEFT JOIN albums a ON a.id = t.album_id";

fn opt_u32(v: Option<i64>) -> Option<u32> {
    v.and_then(|n| u32::try_from(n).ok())
}

fn track_row(r: &Row<'_>) -> rusqlite::Result<TrackRow> {
    Ok(TrackRow {
        id: r.get(0)?,
        title: r.get(1)?,
        artist: r.get(2)?,
        album: r.get(3)?,
        album_id: r.get(4)?,
        track_no: opt_u32(r.get(5)?),
        disc_no: opt_u32(r.get(6)?),
        year: r
            .get::<_, Option<i64>>(7)?
            .and_then(|n| i32::try_from(n).ok()),
        duration_ms: r
            .get::<_, Option<i64>>(8)?
            .and_then(|n| u64::try_from(n).ok()),
        path: r.get(9)?,
        cover: r.get(10)?,
    })
}

struct Filter {
    where_sql: String,
    params: Vec<String>,
}

fn build_filter(query: &str, clause_for_param: impl Fn(usize) -> String) -> Filter {
    let params: Vec<String> = query
        .split_whitespace()
        .take(8)
        .map(|tok| {
            let escaped = tok
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            format!("%{escaped}%")
        })
        .collect();
    if params.is_empty() {
        return Filter {
            where_sql: String::new(),
            params,
        };
    }
    let parts: Vec<String> = (1..=params.len()).map(clause_for_param).collect();
    Filter {
        where_sql: format!("WHERE {}", parts.join(" AND ")),
        params,
    }
}

fn track_filter(query: &str) -> Filter {
    build_filter(query, |i| {
        format!(
            "(t.title LIKE ?{i} ESCAPE '\\' OR t.artist LIKE ?{i} ESCAPE '\\' \
             OR COALESCE(a.title, '') LIKE ?{i} ESCAPE '\\')"
        )
    })
}

fn album_filter(query: &str) -> Filter {
    build_filter(query, |i| {
        format!(
            "(a.title LIKE ?{i} ESCAPE '\\' OR a.album_artist LIKE ?{i} ESCAPE '\\' \
             OR EXISTS (SELECT 1 FROM tracks t WHERE t.album_id = a.id \
             AND (t.title LIKE ?{i} ESCAPE '\\' OR t.artist LIKE ?{i} ESCAPE '\\')))"
        )
    })
}

pub fn tracks_page(
    conn: &Connection,
    offset: u64,
    limit: u64,
    sort: SortOrder,
) -> Result<Vec<TrackRow>> {
    Ok(search_tracks_page(conn, offset, limit, sort, "")?.items)
}

fn query_page<T>(
    conn: &Connection,
    count_sql: &str,
    page_sql: &str,
    filter_params: Vec<String>,
    offset: u64,
    limit: u64,
    mapper: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
) -> Result<Page<T>> {
    let mut values: Vec<Value> = filter_params.into_iter().map(Value::Text).collect();
    let total: i64 = conn.query_row(count_sql, params_from_iter(values.iter()), |r| r.get(0))?;
    values.push(Value::Integer(limit as i64));
    values.push(Value::Integer(offset as i64));
    let mut st = conn.prepare(page_sql)?;
    let rows = st.query_map(params_from_iter(values.iter()), mapper)?;
    Ok(Page {
        total: total.max(0) as u64,
        items: rows.collect::<std::result::Result<_, _>>()?,
    })
}

pub fn search_tracks_page(
    conn: &Connection,
    offset: u64,
    limit: u64,
    sort: SortOrder,
    query: &str,
) -> Result<Page<TrackRow>> {
    let f = track_filter(query);
    let n = f.params.len();
    let count_sql = format!(
        "SELECT COUNT(*) FROM tracks t LEFT JOIN albums a ON a.id = t.album_id {}",
        f.where_sql
    );
    let page_sql = format!(
        "{TRACK_SELECT} {} ORDER BY {} LIMIT ?{} OFFSET ?{}",
        f.where_sql,
        sort.order_by(),
        n + 1,
        n + 2
    );
    query_page(
        conn, &count_sql, &page_sql, f.params, offset, limit, track_row,
    )
}

/// Row → queue track (path, title, artist) for the three-column selects below.
fn queue_track(r: &rusqlite::Row<'_>) -> rusqlite::Result<Track> {
    Ok(Track {
        path: PathBuf::from(r.get::<_, String>(0)?),
        title: Some(r.get(1)?),
        artist: Some(r.get(2)?),
    })
}

pub fn all_tracks(conn: &Connection, sort: SortOrder) -> Result<Vec<Track>> {
    matching_tracks(conn, sort, "")
}

pub fn matching_tracks(conn: &Connection, sort: SortOrder, query: &str) -> Result<Vec<Track>> {
    let f = track_filter(query);
    let sql = format!(
        "SELECT t.path, t.title, t.artist FROM tracks t \
         LEFT JOIN albums a ON a.id = t.album_id {} ORDER BY {}",
        f.where_sql,
        sort.order_by()
    );
    let values: Vec<Value> = f.params.into_iter().map(Value::Text).collect();
    let mut st = conn.prepare(&sql)?;
    let rows = st.query_map(params_from_iter(values.iter()), queue_track)?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

const ALBUM_SELECT: &str = "SELECT a.id, a.title, a.album_artist, a.year,
        (SELECT COUNT(*) FROM tracks WHERE album_id = a.id), a.cover
     FROM albums a";

fn album_row(r: &Row<'_>) -> rusqlite::Result<AlbumRow> {
    Ok(AlbumRow {
        id: r.get(0)?,
        title: r.get(1)?,
        album_artist: r.get(2)?,
        year: r
            .get::<_, Option<i64>>(3)?
            .and_then(|n| i32::try_from(n).ok()),
        track_count: r.get::<_, i64>(4)?.try_into().unwrap_or(u32::MAX),
        cover: r.get(5)?,
    })
}

pub fn albums_page(conn: &Connection, offset: u64, limit: u64) -> Result<Vec<AlbumRow>> {
    Ok(search_albums_page(conn, offset, limit, "")?.items)
}

pub fn search_albums_page(
    conn: &Connection,
    offset: u64,
    limit: u64,
    query: &str,
) -> Result<Page<AlbumRow>> {
    let f = album_filter(query);
    let n = f.params.len();
    let count_sql = format!("SELECT COUNT(*) FROM albums a {}", f.where_sql);
    let page_sql = format!(
        "{ALBUM_SELECT} {} ORDER BY a.album_artist, a.title, a.year, a.id LIMIT ?{} OFFSET ?{}",
        f.where_sql,
        n + 1,
        n + 2
    );
    query_page(
        conn, &count_sql, &page_sql, f.params, offset, limit, album_row,
    )
}

const ALBUM_TRACK_ORDER: &str = "t.disc_no, t.track_no, t.title, t.id";

pub fn album_detail(conn: &Connection, album_id: i64) -> Result<AlbumDetail> {
    let album = conn
        .query_row(
            &format!("{ALBUM_SELECT} WHERE a.id = ?1"),
            [album_id],
            album_row,
        )
        .optional()?
        .ok_or_else(|| super::LibraryError::Other(format!("album {album_id} not found")))?;
    let mut st = conn.prepare(&format!(
        "{TRACK_SELECT} WHERE t.album_id = ?1 ORDER BY {ALBUM_TRACK_ORDER}"
    ))?;
    let tracks = st
        .query_map([album_id], track_row)?
        .collect::<std::result::Result<_, _>>()?;
    Ok(AlbumDetail { album, tracks })
}

pub fn album_tracks(conn: &Connection, album_id: i64) -> Result<Vec<Track>> {
    let mut st = conn.prepare(&format!(
        "SELECT t.path, t.title, t.artist FROM tracks t WHERE t.album_id = ?1 ORDER BY {ALBUM_TRACK_ORDER}"
    ))?;
    let rows = st.query_map([album_id], queue_track)?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// Cover file of the album a track belongs to, if any.
pub fn track_cover_by_path(conn: &Connection, path: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT a.cover FROM tracks t JOIN albums a ON a.id = t.album_id
         WHERE t.path = ?1",
        [path],
        |r| r.get::<_, Option<String>>(0),
    )
    .optional()
    .map(Option::flatten)
    .map_err(Into::into)
}

pub fn track_meta_by_path(conn: &Connection, path: &str) -> Result<Option<TrackMeta>> {
    conn.query_row(
        "SELECT t.title, t.artist, COALESCE(a.title, ''), t.duration_ms
         FROM tracks t LEFT JOIN albums a ON a.id = t.album_id
         WHERE t.path = ?1",
        [path],
        |r| {
            Ok(TrackMeta {
                title: r.get(0)?,
                artist: r.get(1)?,
                album: r.get(2)?,
                duration_ms: r
                    .get::<_, Option<i64>>(3)?
                    .and_then(|n| u64::try_from(n).ok()),
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

// ---------------------------------------------------------------------
// Playlists
//
// Entries are ordered by `position`. Positions can have gaps (a track that
// disappears in a rescan cascades its entries away), so the public API
// addresses entries by their *index* in that order, and every edit rewrites
// the playlist compactly (0..n).
// ---------------------------------------------------------------------

fn playlist_not_found(id: i64) -> super::LibraryError {
    super::LibraryError::Other(format!("playlist {id} not found"))
}

fn clean_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() {
        return Err(super::LibraryError::Other(
            "playlist name must not be empty".into(),
        ));
    }
    Ok(name)
}

fn ensure_playlist(conn: &Connection, id: i64) -> Result<()> {
    let found: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM playlists WHERE id = ?1)",
        [id],
        |r| r.get(0),
    )?;
    if found {
        Ok(())
    } else {
        Err(playlist_not_found(id))
    }
}

fn playlist_album_covers(conn: &Connection, id: i64) -> Result<Vec<String>> {
    let mut st = conn.prepare_cached(
        "SELECT a.cover
         FROM playlist_tracks pt
         JOIN tracks t ON t.id = pt.track_id
         JOIN albums a ON a.id = t.album_id
         WHERE pt.playlist_id = ?1 AND a.cover IS NOT NULL AND a.cover != ''
         GROUP BY a.id
         ORDER BY MIN(pt.position)
         LIMIT 4",
    )?;
    let rows = st.query_map([id], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub fn playlist_info(conn: &Connection, id: i64) -> Result<PlaylistInfo> {
    let mut info = conn
        .query_row(
            "SELECT p.id, p.name,
                    (SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = p.id),
                    p.cover
             FROM playlists p WHERE p.id = ?1",
            [id],
            |r| {
                Ok(PlaylistInfo {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    track_count: r.get::<_, i64>(2)?.try_into().unwrap_or(u32::MAX),
                    cover: r.get(3)?,
                    album_covers: Vec::new(),
                })
            },
        )
        .optional()?
        .ok_or_else(|| playlist_not_found(id))?;
    info.album_covers = playlist_album_covers(conn, info.id)?;
    Ok(info)
}

pub fn playlists(conn: &Connection) -> Result<Vec<PlaylistInfo>> {
    let mut st = conn.prepare(
        "SELECT p.id, p.name, COUNT(pt.track_id), p.cover
         FROM playlists p LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id
         GROUP BY p.id ORDER BY p.name COLLATE NOCASE, p.id",
    )?;
    let rows = st.query_map([], |r| {
        Ok(PlaylistInfo {
            id: r.get(0)?,
            name: r.get(1)?,
            track_count: r.get::<_, i64>(2)?.try_into().unwrap_or(u32::MAX),
            cover: r.get(3)?,
            album_covers: Vec::new(),
        })
    })?;
    let mut out: Vec<PlaylistInfo> = rows.collect::<std::result::Result<_, _>>()?;
    for item in &mut out {
        item.album_covers = playlist_album_covers(conn, item.id)?;
    }
    Ok(out)
}

/// Appends the tracks that exist in the library; unknown ids are skipped.
/// Returns how many entries were added.
fn append_tracks(conn: &Connection, id: i64, track_ids: &[i64]) -> Result<usize> {
    let mut next: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM playlist_tracks WHERE playlist_id = ?1",
        [id],
        |r| r.get(0),
    )?;
    let mut insert = conn.prepare_cached(
        "INSERT INTO playlist_tracks(playlist_id, position, track_id)
         SELECT ?1, ?2, id FROM tracks WHERE id = ?3",
    )?;
    let mut added = 0;
    for track_id in track_ids {
        if insert.execute(rusqlite::params![id, next, track_id])? > 0 {
            next += 1;
            added += 1;
        }
    }
    Ok(added)
}

pub fn create_playlist(
    conn: &mut Connection,
    name: &str,
    track_ids: &[i64],
    now: i64,
) -> Result<PlaylistInfo> {
    let name = clean_name(name)?;
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO playlists(name, created_at) VALUES (?1, ?2)",
        rusqlite::params![name, now],
    )?;
    let id = tx.last_insert_rowid();
    append_tracks(&tx, id, track_ids)?;
    let info = playlist_info(&tx, id)?;
    tx.commit()?;
    Ok(info)
}

pub fn rename_playlist(conn: &Connection, id: i64, name: &str) -> Result<()> {
    let name = clean_name(name)?;
    let n = conn.execute(
        "UPDATE playlists SET name = ?2 WHERE id = ?1",
        rusqlite::params![id, name],
    )?;
    if n == 0 {
        return Err(playlist_not_found(id));
    }
    Ok(())
}

pub fn set_playlist_cover(
    conn: &Connection,
    id: i64,
    cover: Option<&str>,
) -> Result<Option<String>> {
    let old: Option<String> = conn
        .query_row("SELECT cover FROM playlists WHERE id = ?1", [id], |r| r.get(0))
        .optional()?
        .flatten();
    let n = conn.execute(
        "UPDATE playlists SET cover = ?2 WHERE id = ?1",
        rusqlite::params![id, cover],
    )?;
    if n == 0 {
        return Err(playlist_not_found(id));
    }
    Ok(old)
}

pub fn delete_playlist(conn: &Connection, id: i64) -> Result<Option<String>> {
    let old: Option<String> = conn
        .query_row("SELECT cover FROM playlists WHERE id = ?1", [id], |r| r.get(0))
        .optional()?
        .flatten();
    let n = conn.execute("DELETE FROM playlists WHERE id = ?1", [id])?;
    if n == 0 {
        return Err(playlist_not_found(id));
    }
    Ok(old)
}

pub fn playlist_tracks(
    conn: &Connection,
    id: i64,
    offset: u64,
    limit: u64,
) -> Result<Vec<TrackRow>> {
    ensure_playlist(conn, id)?;
    let mut st = conn.prepare(&format!(
        "{TRACK_SELECT} JOIN playlist_tracks pt ON pt.track_id = t.id
         WHERE pt.playlist_id = ?1 ORDER BY pt.position LIMIT ?2 OFFSET ?3"
    ))?;
    let rows = st.query_map(
        rusqlite::params![id, limit as i64, offset as i64],
        track_row,
    )?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// Playlist contents in order, ready for the queue.
pub fn playlist_queue_tracks(conn: &Connection, id: i64) -> Result<Vec<Track>> {
    ensure_playlist(conn, id)?;
    let mut st = conn.prepare(
        "SELECT t.path, t.title, t.artist FROM playlist_tracks pt
         JOIN tracks t ON t.id = pt.track_id
         WHERE pt.playlist_id = ?1 ORDER BY pt.position",
    )?;
    let rows = st.query_map([id], queue_track)?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub fn playlist_add_tracks(conn: &mut Connection, id: i64, track_ids: &[i64]) -> Result<usize> {
    let tx = conn.transaction()?;
    ensure_playlist(&tx, id)?;
    let added = append_tracks(&tx, id, track_ids)?;
    tx.commit()?;
    Ok(added)
}

pub fn playlist_add_album(conn: &mut Connection, id: i64, album_id: i64) -> Result<usize> {
    let tx = conn.transaction()?;
    ensure_playlist(&tx, id)?;
    let ids = {
        let mut st = tx.prepare(&format!(
            "SELECT t.id FROM tracks t WHERE t.album_id = ?1 ORDER BY {ALBUM_TRACK_ORDER}"
        ))?;
        st.query_map([album_id], |r| r.get::<_, i64>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    let added = append_tracks(&tx, id, &ids)?;
    tx.commit()?;
    Ok(added)
}

/// Track ids of a playlist in order.
fn order(conn: &Connection, id: i64) -> Result<Vec<i64>> {
    let mut st = conn.prepare_cached(
        "SELECT track_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position",
    )?;
    Ok(st
        .query_map([id], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?)
}

/// Replaces the playlist's entries with `ids` at positions 0..n.
fn rewrite(conn: &Connection, id: i64, ids: &[i64]) -> Result<()> {
    conn.execute("DELETE FROM playlist_tracks WHERE playlist_id = ?1", [id])?;
    let mut insert = conn.prepare_cached(
        "INSERT INTO playlist_tracks(playlist_id, position, track_id) VALUES (?1, ?2, ?3)",
    )?;
    for (pos, track_id) in ids.iter().enumerate() {
        insert.execute(rusqlite::params![id, pos as i64, track_id])?;
    }
    Ok(())
}

/// Removes the entries at the given indexes (duplicates and out-of-range
/// indexes are ignored) and closes the gaps.
pub fn playlist_remove(conn: &mut Connection, id: i64, indexes: &[usize]) -> Result<()> {
    let tx = conn.transaction()?;
    ensure_playlist(&tx, id)?;
    let drop: std::collections::HashSet<usize> = indexes.iter().copied().collect();
    let kept: Vec<i64> = order(&tx, id)?
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !drop.contains(i))
        .map(|(_, t)| t)
        .collect();
    rewrite(&tx, id, &kept)?;
    tx.commit()?;
    Ok(())
}

/// Moves the entry at index `from` so that it ends up at index `to`.
pub fn playlist_move(conn: &mut Connection, id: i64, from: usize, to: usize) -> Result<()> {
    let tx = conn.transaction()?;
    ensure_playlist(&tx, id)?;
    let mut ids = order(&tx, id)?;
    if from >= ids.len() || to >= ids.len() {
        return Err(super::LibraryError::Other(format!(
            "index out of range (from {from}, to {to}, length {})",
            ids.len()
        )));
    }
    let moved = ids.remove(from);
    ids.insert(to, moved);
    rewrite(&tx, id, &ids)?;
    tx.commit()?;
    Ok(())
}

/// Library track id for each path (`None` when the file is not in the library).
pub fn track_ids_by_paths(conn: &Connection, paths: &[String]) -> Result<Vec<Option<i64>>> {
    let mut st = conn.prepare_cached("SELECT id FROM tracks WHERE path = ?1")?;
    paths
        .iter()
        .map(|p| Ok(st.query_row([p], |r| r.get(0)).optional()?))
        .collect()
}

/// Every track, for matching imported playlists.
pub fn match_candidates(conn: &Connection) -> Result<Vec<super::m3u::Candidate>> {
    let mut st = conn.prepare("SELECT id, path, artist, title, duration_ms FROM tracks")?;
    let rows = st.query_map([], |r| {
        Ok(super::m3u::Candidate {
            id: r.get(0)?,
            path: r.get(1)?,
            artist: r.get(2)?,
            title: r.get(3)?,
            duration_ms: r
                .get::<_, Option<i64>>(4)?
                .and_then(|n| u64::try_from(n).ok()),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Id, path and folder of each existing track, in the order of `ids`.
/// Unknown ids are skipped.
pub fn track_files_by_ids(conn: &Connection, ids: &[i64]) -> Result<Vec<(i64, String, i64)>> {
    let mut st = conn.prepare_cached("SELECT path, folder_id FROM tracks WHERE id = ?1")?;
    let mut out = Vec::with_capacity(ids.len());
    for &id in ids {
        if let Some((path, folder)) = st
            .query_row([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .optional()?
        {
            out.push((id, path, folder));
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------
// Favorites
// ---------------------------------------------------------------------

pub fn favorites(conn: &Connection) -> Result<Vec<FavoriteRef>> {
    let mut st = conn.prepare(
        "SELECT f.track_id, t.path
         FROM favorites f
         JOIN tracks t ON t.id = f.track_id
         ORDER BY f.added_at DESC, t.title COLLATE NOCASE, f.track_id",
    )?;
    let rows = st.query_map([], |r| {
        Ok(FavoriteRef {
            track_id: r.get(0)?,
            path: r.get(1)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub fn toggle_favorite(conn: &Connection, track_id: i64, now: i64) -> Result<bool> {
    let removed = conn.execute("DELETE FROM favorites WHERE track_id = ?1", [track_id])?;
    if removed > 0 {
        return Ok(false);
    }
    let inserted = conn.execute(
        "INSERT INTO favorites(track_id, added_at)
         SELECT id, ?2 FROM tracks WHERE id = ?1",
        rusqlite::params![track_id, now],
    )?;
    if inserted == 0 {
        return Err(super::LibraryError::Other(format!(
            "track {track_id} not found"
        )));
    }
    Ok(true)
}

pub fn favorites_page(conn: &Connection, offset: u64, limit: u64) -> Result<Page<TrackRow>> {
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM favorites", [], |r| r.get(0))?;
    let mut st = conn.prepare(&format!(
        "{TRACK_SELECT} JOIN favorites f ON f.track_id = t.id
         ORDER BY f.added_at DESC, t.title COLLATE NOCASE, f.track_id
         LIMIT ?1 OFFSET ?2"
    ))?;
    let rows = st.query_map(rusqlite::params![limit as i64, offset as i64], track_row)?;
    let items = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(Page {
        total: u64::try_from(total).unwrap_or(0),
        items,
    })
}

pub fn favorites_queue_tracks(conn: &Connection) -> Result<Vec<Track>> {
    let mut st = conn.prepare(
        "SELECT t.path, t.title, t.artist
         FROM favorites f
         JOIN tracks t ON t.id = f.track_id
         ORDER BY f.added_at DESC, t.title COLLATE NOCASE, f.track_id",
    )?;
    let rows = st.query_map([], queue_track)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

// ---------------------------------------------------------------------
// Listening history & stats
// ---------------------------------------------------------------------

const LISTEN_RANGE: &str =
    "(?1 IS NULL OR e.started_at >= ?1) AND (?2 IS NULL OR e.started_at < ?2)";
const PLAY_EXPR: &str = "CASE WHEN e.completed = 1 OR e.listened_ms >= 15000 THEN 1 ELSE 0 END";

pub fn record_listen_event(conn: &Connection, ev: &NewListenEvent) -> Result<()> {
    let matched: Option<(
        i64,
        String,
        String,
        String,
        String,
        Option<i64>,
    )> = conn
        .query_row(
            "SELECT t.id, t.title, t.artist, COALESCE(a.title, ''),
                    COALESCE(t.genre, ''), t.duration_ms
             FROM tracks t
             LEFT JOIN albums a ON a.id = t.album_id
             WHERE t.path = ?1",
            [&ev.path],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .optional()?;

    let (track_id, title, artist, album, genre, lib_duration) = match matched {
        Some((id, t, ar, al, ge, dur)) => (
            Some(id),
            if t.trim().is_empty() {
                ev.title.clone()
            } else {
                t
            },
            if ar.trim().is_empty() {
                ev.artist.clone()
            } else {
                ar
            },
            al,
            ge,
            dur,
        ),
        None => (
            None,
            ev.title.clone(),
            ev.artist.clone(),
            String::new(),
            String::new(),
            None,
        ),
    };

    let duration_ms = lib_duration
        .or_else(|| ev.duration_ms.map(|d| i64::try_from(d).unwrap_or(i64::MAX)));
    let listened_ms = i64::try_from(ev.listened_ms).unwrap_or(i64::MAX);
    let source = if ev.path.starts_with("http://") || ev.path.starts_with("https://") {
        "online"
    } else {
        "local"
    };

    conn.execute(
        "INSERT INTO listen_events(
             track_id, path, title, artist, album, genre, duration_ms,
             started_at, ended_at, listened_ms, completed, skipped, source
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        rusqlite::params![
            track_id,
            ev.path,
            title,
            artist,
            album,
            genre,
            duration_ms,
            ev.started_at,
            ev.ended_at,
            listened_ms,
            i64::from(ev.completed),
            i64::from(ev.skipped),
            source,
        ],
    )?;
    Ok(())
}

pub fn clear_listen_history(conn: &Connection) -> Result<u64> {
    let n = conn.execute("DELETE FROM listen_events", [])?;
    Ok(n as u64)
}

fn u64_col(r: &Row<'_>, idx: usize) -> rusqlite::Result<u64> {
    let v: i64 = r.get(idx)?;
    Ok(u64::try_from(v).unwrap_or(0))
}

fn query_top_tracks(
    conn: &Connection,
    from: Option<i64>,
    to: Option<i64>,
    order_by_time: bool,
    limit: usize,
) -> Result<Vec<TopTrackStat>> {
    let order = if order_by_time {
        "total_ms DESC, plays DESC, MIN(e.title) COLLATE NOCASE"
    } else {
        "plays DESC, total_ms DESC, MIN(e.title) COLLATE NOCASE"
    };
    let having = if order_by_time { "" } else { "HAVING plays > 0" };
    let sql = format!(
        "SELECT MAX(e.track_id), MAX(e.path), MAX(e.title), MAX(e.artist),
                MAX(e.album), MAX(a.cover),
                SUM({PLAY_EXPR}) AS plays,
                SUM(e.listened_ms) AS total_ms,
                SUM(e.skipped) AS skips
         FROM listen_events e
         LEFT JOIN tracks t ON t.id = e.track_id
         LEFT JOIN albums a ON a.id = t.album_id
         WHERE {LISTEN_RANGE}
         GROUP BY LOWER(e.title), LOWER(e.artist)
         {having}
         ORDER BY {order}
         LIMIT ?3"
    );
    let mut st = conn.prepare(&sql)?;
    let rows = st.query_map(rusqlite::params![from, to, limit as i64], |r| {
        Ok(TopTrackStat {
            track_id: r.get(0)?,
            path: r.get(1)?,
            title: r.get(2)?,
            artist: r.get(3)?,
            album: r.get(4)?,
            cover: r.get(5)?,
            play_count: u64_col(r, 6)?,
            listened_ms: u64_col(r, 7)?,
            skip_count: u64_col(r, 8)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn streak_stats(conn: &Connection, from: Option<i64>, to: Option<i64>) -> Result<(u64, u64, u64)> {
    let sql = format!(
        "SELECT DISTINCT
             CAST(
                 julianday(e.started_at, 'unixepoch', 'localtime', 'start of day') + 0.5
                 AS INTEGER
             ) AS d
         FROM listen_events e
         WHERE {LISTEN_RANGE}
         ORDER BY d ASC"
    );
    let mut st = conn.prepare(&sql)?;
    let days: Vec<i64> = st
        .query_map(rusqlite::params![from, to], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    if days.is_empty() {
        return Ok((0, 0, 0));
    }
    let today: i64 = conn.query_row(
        "SELECT CAST(julianday('now', 'localtime', 'start of day') + 0.5 AS INTEGER)",
        [],
        |r| r.get(0),
    )?;
    let mut longest = 1u64;
    let mut run = 1u64;
    for pair in days.windows(2) {
        if pair[1] == pair[0] + 1 {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 1;
        }
    }
    let last_day = *days.last().unwrap_or(&0);
    let current = if last_day == today || last_day == today - 1 {
        run
    } else {
        0
    };
    Ok((days.len() as u64, longest, current))
}

pub fn listening_stats(
    conn: &Connection,
    from: Option<i64>,
    to: Option<i64>,
    limit: usize,
) -> Result<ListeningStats> {
    let summary_sql = format!(
        "SELECT COALESCE(SUM(e.listened_ms), 0),
                COALESCE(SUM({PLAY_EXPR}), 0),
                COUNT(*),
                COALESCE(SUM(e.skipped), 0),
                COALESCE(SUM(e.completed), 0),
                COUNT(DISTINCT LOWER(e.title) || char(0) || LOWER(e.artist)),
                COUNT(DISTINCT CASE WHEN TRIM(e.artist) != '' THEN LOWER(e.artist) END),
                COUNT(DISTINCT CASE WHEN TRIM(e.album) != '' THEN LOWER(e.album) END),
                MIN(e.started_at)
         FROM listen_events e
         WHERE {LISTEN_RANGE}"
    );
    let (
        total_listened_ms,
        play_count,
        raw_event_count,
        skip_count,
        completed_count,
        unique_tracks,
        unique_artists,
        unique_albums,
        first_listen_at,
    ) = conn.query_row(&summary_sql, rusqlite::params![from, to], |r| {
        Ok((
            u64_col(r, 0)?,
            u64_col(r, 1)?,
            u64_col(r, 2)?,
            u64_col(r, 3)?,
            u64_col(r, 4)?,
            u64_col(r, 5)?,
            u64_col(r, 6)?,
            u64_col(r, 7)?,
            r.get::<_, Option<i64>>(8)?,
        ))
    })?;

    let (active_days, longest_streak_days, current_streak_days) = streak_stats(conn, from, to)?;

    let top_tracks_by_plays = query_top_tracks(conn, from, to, false, limit)?;
    let top_tracks_by_time = query_top_tracks(conn, from, to, true, limit)?;

    let artist_sql = format!(
        "SELECT MAX(e.artist), MAX(a.cover),
                SUM({PLAY_EXPR}) AS plays,
                SUM(e.listened_ms) AS total_ms,
                COUNT(DISTINCT LOWER(e.title)) AS uniq,
                SUM(e.skipped) AS skips
         FROM listen_events e
         LEFT JOIN tracks t ON t.id = e.track_id
         LEFT JOIN albums a ON a.id = t.album_id
         WHERE {LISTEN_RANGE} AND TRIM(e.artist) != ''
         GROUP BY LOWER(e.artist)
         ORDER BY total_ms DESC, plays DESC, MAX(e.artist) COLLATE NOCASE
         LIMIT ?3"
    );
    let mut artist_st = conn.prepare(&artist_sql)?;
    let top_artists_by_time = artist_st
        .query_map(rusqlite::params![from, to, limit as i64], |r| {
            Ok(TopArtistStat {
                artist: r.get(0)?,
                cover: r.get(1)?,
                play_count: u64_col(r, 2)?,
                listened_ms: u64_col(r, 3)?,
                unique_tracks: u64_col(r, 4)?,
                skip_count: u64_col(r, 5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let album_sql = format!(
        "SELECT MAX(e.album), MAX(e.artist), MAX(t.album_id), MAX(a.cover),
                SUM({PLAY_EXPR}) AS plays,
                SUM(e.listened_ms) AS total_ms
         FROM listen_events e
         LEFT JOIN tracks t ON t.id = e.track_id
         LEFT JOIN albums a ON a.id = t.album_id
         WHERE {LISTEN_RANGE} AND TRIM(e.album) != ''
         GROUP BY LOWER(e.album)
         ORDER BY total_ms DESC, plays DESC, MAX(e.album) COLLATE NOCASE
         LIMIT ?3"
    );
    let mut album_st = conn.prepare(&album_sql)?;
    let top_albums_by_time = album_st
        .query_map(rusqlite::params![from, to, limit as i64], |r| {
            Ok(TopAlbumStat {
                album: r.get(0)?,
                artist: r.get(1)?,
                album_id: r.get(2)?,
                cover: r.get(3)?,
                play_count: u64_col(r, 4)?,
                listened_ms: u64_col(r, 5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let genre_sql = format!(
        "SELECT MAX(e.genre), SUM({PLAY_EXPR}) AS plays, SUM(e.listened_ms) AS total_ms
         FROM listen_events e
         WHERE {LISTEN_RANGE} AND TRIM(e.genre) != ''
         GROUP BY LOWER(e.genre)
         ORDER BY total_ms DESC, plays DESC, MAX(e.genre) COLLATE NOCASE
         LIMIT ?3"
    );
    let mut genre_st = conn.prepare(&genre_sql)?;
    let top_genres = genre_st
        .query_map(rusqlite::params![from, to, limit as i64], |r| {
            Ok(TopGenreStat {
                genre: r.get(0)?,
                play_count: u64_col(r, 1)?,
                listened_ms: u64_col(r, 2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut by_hour: Vec<BucketStat> = (0..24)
        .map(|h| BucketStat {
            label: format!("{h:02}"),
            listened_ms: 0,
            play_count: 0,
        })
        .collect();
    let hour_sql = format!(
        "SELECT CAST(strftime('%H', e.started_at, 'unixepoch', 'localtime') AS INTEGER) AS h,
                SUM(e.listened_ms), SUM({PLAY_EXPR})
         FROM listen_events e
         WHERE {LISTEN_RANGE}
         GROUP BY h"
    );
    let mut hour_st = conn.prepare(&hour_sql)?;
    let hour_rows = hour_st.query_map(rusqlite::params![from, to], |r| {
        Ok((r.get::<_, i64>(0)?, u64_col(r, 1)?, u64_col(r, 2)?))
    })?;
    for row in hour_rows {
        let (h, ms, plays) = row?;
        if let Ok(idx) = usize::try_from(h)
            && idx < 24
        {
            by_hour[idx].listened_ms = ms;
            by_hour[idx].play_count = plays;
        }
    }
    let top_hour = by_hour
        .iter()
        .enumerate()
        .filter(|(_, b)| b.listened_ms > 0)
        .max_by_key(|(_, b)| b.listened_ms)
        .map(|(idx, _)| idx as u32);

    let mut by_weekday: Vec<BucketStat> = (0..7)
        .map(|w| BucketStat {
            label: w.to_string(),
            listened_ms: 0,
            play_count: 0,
        })
        .collect();
    let wday_sql = format!(
        "SELECT CAST(strftime('%w', e.started_at, 'unixepoch', 'localtime') AS INTEGER) AS w,
                SUM(e.listened_ms), SUM({PLAY_EXPR})
         FROM listen_events e
         WHERE {LISTEN_RANGE}
         GROUP BY w"
    );
    let mut wday_st = conn.prepare(&wday_sql)?;
    let wday_rows = wday_st.query_map(rusqlite::params![from, to], |r| {
        Ok((r.get::<_, i64>(0)?, u64_col(r, 1)?, u64_col(r, 2)?))
    })?;
    for row in wday_rows {
        let (w, ms, plays) = row?;
        // Convert SQLite %w (0 = Sun .. 6 = Sat) to Monday-first (0 = Mon .. 6 = Sun).
        let idx = ((w.rem_euclid(7) + 6) % 7) as usize;
        by_weekday[idx].listened_ms = ms;
        by_weekday[idx].play_count = plays;
    }
    let top_weekday = by_weekday
        .iter()
        .enumerate()
        .filter(|(_, b)| b.listened_ms > 0)
        .max_by_key(|(_, b)| b.listened_ms)
        .map(|(idx, _)| idx as u32);

    let month_sql = format!(
        "SELECT m, total_ms, plays FROM (
             SELECT strftime('%Y-%m', e.started_at, 'unixepoch', 'localtime') AS m,
                    SUM(e.listened_ms) AS total_ms,
                    SUM({PLAY_EXPR}) AS plays
             FROM listen_events e
             WHERE {LISTEN_RANGE}
             GROUP BY m
             ORDER BY m DESC
             LIMIT 24
         ) ORDER BY m ASC"
    );
    let mut month_st = conn.prepare(&month_sql)?;
    let by_month = month_st
        .query_map(rusqlite::params![from, to], |r| {
            Ok(BucketStat {
                label: r.get(0)?,
                listened_ms: u64_col(r, 1)?,
                play_count: u64_col(r, 2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let recent_sql = format!(
        "SELECT e.id, e.track_id, e.path, e.title, e.artist, e.album, a.cover,
                e.started_at, e.listened_ms, e.completed, e.skipped, e.source
         FROM listen_events e
         LEFT JOIN tracks t ON t.id = e.track_id
         LEFT JOIN albums a ON a.id = t.album_id
         WHERE {LISTEN_RANGE}
         ORDER BY e.started_at DESC, e.id DESC
         LIMIT 50"
    );
    let mut recent_st = conn.prepare(&recent_sql)?;
    let recent = recent_st
        .query_map(rusqlite::params![from, to], |r| {
            Ok(RecentListen {
                id: r.get(0)?,
                track_id: r.get(1)?,
                path: r.get(2)?,
                title: r.get(3)?,
                artist: r.get(4)?,
                album: r.get(5)?,
                cover: r.get(6)?,
                started_at: r.get(7)?,
                listened_ms: u64_col(r, 8)?,
                completed: r.get::<_, i64>(9)? != 0,
                skipped: r.get::<_, i64>(10)? != 0,
                source: r.get(11)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut year_st = conn.prepare(
        "SELECT DISTINCT CAST(strftime('%Y', started_at, 'unixepoch', 'localtime') AS INTEGER) AS y
         FROM listen_events
         ORDER BY y DESC",
    )?;
    let available_years = year_st
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(ListeningStats {
        summary: StatsSummary {
            total_listened_ms,
            play_count,
            raw_event_count,
            skip_count,
            completed_count,
            unique_tracks,
            unique_artists,
            unique_albums,
            active_days,
            longest_streak_days,
            current_streak_days,
            top_hour,
            top_weekday,
            first_listen_at,
        },
        top_tracks_by_plays,
        top_tracks_by_time,
        top_artists_by_time,
        top_albums_by_time,
        top_genres,
        by_hour,
        by_weekday,
        by_month,
        recent,
        available_years,
    })
}
