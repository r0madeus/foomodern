//! SQLite schema and queries. All functions take a plain `&Connection` so
//! they can be tested without the [`super::Library`] facade.

use super::{AlbumDetail, AlbumRow, FolderInfo, Page, Result, TrackMeta, TrackRow};
use crate::queue::Track;
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, Row, params_from_iter};
use std::path::PathBuf;

const SCHEMA_VERSION: i64 = 1;

const SCHEMA_V1: &str = "
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
            Self::Artist => "t.artist, a.title, t.disc_no, t.track_no, t.title, t.id",
            Self::Title => "t.title, t.artist, t.id",
            Self::Album => "a.title, a.album_artist, t.disc_no, t.track_no, t.title, t.id",
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

/// Deletes albums without tracks; returns their cover paths.
pub fn delete_empty_albums(conn: &Connection) -> Result<Vec<String>> {
    let mut st = conn.prepare(
        "SELECT cover FROM albums WHERE cover IS NOT NULL
         AND NOT EXISTS (SELECT 1 FROM tracks WHERE album_id = albums.id)",
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
        t.track_no, t.disc_no, t.year, t.duration_ms, t.path
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
    query_page(conn, &count_sql, &page_sql, f.params, offset, limit, track_row)
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
        "{ALBUM_SELECT} {} ORDER BY a.album_artist, a.title, a.id LIMIT ?{} OFFSET ?{}",
        f.where_sql,
        n + 1,
        n + 2
    );
    query_page(conn, &count_sql, &page_sql, f.params, offset, limit, album_row)
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
