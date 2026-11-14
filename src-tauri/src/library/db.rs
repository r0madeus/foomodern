//! SQLite schema and queries. All functions take a plain `&Connection` so
//! they can be tested without the [`super::Library`] facade.

use super::{
    AlbumDetail, AlbumRow, ArtistMeta, ArtistProfile, ArtistRow, ArtistTopTrack, BucketStat,
    DynamicMix, FavoriteRef, FolderInfo, HomeFeed, HomeMixDetail, ListeningStats, NewListenEvent,
    Page, PlaylistInfo, RecentListen, RecentTrack, Result, StatsSummary, TopAlbumStat,
    TopArtistStat, TopGenreStat, TopTrackStat, TrackMeta, TrackRow,
};
use crate::queue::Track;
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, Row, params_from_iter};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: i64 = 9;

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

const SCHEMA_V7: &str = "
CREATE TABLE track_artists (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    artist   TEXT NOT NULL COLLATE NOCASE,
    position INTEGER NOT NULL,
    PRIMARY KEY (track_id, artist)
);
CREATE INDEX track_artists_artist ON track_artists(artist, track_id);

CREATE TABLE artists_meta (
    name_key   TEXT PRIMARY KEY COLLATE NOCASE,
    display    TEXT NOT NULL,
    image      TEXT,
    bio        TEXT,
    fetched_at INTEGER NOT NULL
);
";

pub(super) const SCHEMA_V9: &str = "
CREATE TABLE IF NOT EXISTS tag_history (
    id       INTEGER PRIMARY KEY,
    batch_id INTEGER NOT NULL,
    track_id INTEGER,
    path     TEXT NOT NULL,
    saved_at INTEGER NOT NULL,
    old_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS tag_history_batch ON tag_history(batch_id);
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
    if version < 7 {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(SCHEMA_V7)?;
        backfill_track_artists(&tx)?;
        tx.pragma_update(None, "user_version", 7)?;
        tx.commit()?;
    }
    if version < 8 {
        // v3 keyed albums by each track's own year, which split albums whose
        // tracks carry different or missing years. Re-derive albums per
        // folder group, and re-split artists with the corrected rules.
        let tx = conn.unchecked_transaction()?;
        regroup_all_albums(&tx)?;
        let orphan_covers = delete_empty_albums(&tx)?;
        backfill_track_artists(&tx)?;
        tx.pragma_update(None, "user_version", 8)?;
        tx.commit()?;
        for cover in orphan_covers {
            if let Err(e) = std::fs::remove_file(&cover) {
                log::debug!("could not remove cover {cover}: {e}");
            }
        }
    }
    if version < 9 {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(SCHEMA_V9)?;
        tx.pragma_update(None, "user_version", 9)?;
        tx.commit()?;
    }
    Ok(())
}

/// The year shown for an album: the most common track year, the latest one
/// on a tie (a compilation's tracks all differ; its release is the latest).
/// Tracks without a year are ignored.
pub fn pick_album_year(years: impl IntoIterator<Item = Option<i64>>) -> Option<i64> {
    let mut counts: HashMap<i64, usize> = HashMap::new();
    for y in years.into_iter().flatten() {
        *counts.entry(y).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|&(year, n)| (n, year))
        .map(|(year, _)| year)
}

/// A track with its current album, as needed to re-derive album groups.
struct GroupedTrack {
    track_id: i64,
    path: String,
    year: Option<i64>,
    album_id: i64,
    title: String,
    album_artist: String,
    album_year: Option<i64>,
    cover: Option<String>,
}

const GROUPED_TRACK_SELECT: &str = "SELECT t.id, t.path, t.year, a.id, a.title,
        a.album_artist, a.year, a.cover
     FROM tracks t JOIN albums a ON a.id = t.album_id";

fn grouped_track(r: &Row<'_>) -> rusqlite::Result<GroupedTrack> {
    Ok(GroupedTrack {
        track_id: r.get(0)?,
        path: r.get(1)?,
        year: r.get(2)?,
        album_id: r.get(3)?,
        title: r.get(4)?,
        album_artist: r.get(5)?,
        album_year: r.get(6)?,
        cover: r.get(7)?,
    })
}

/// Re-derives the albums of the tracks directly inside `dirs`. An album is
/// one (folder, album title, album artist) group; its year comes from
/// [`pick_album_year`], so a track's own year never splits it off. Groups in
/// different folders that end up with the same title, artist and year share
/// one album (e.g. `CD1`/`CD2` folders). Albums left empty are not deleted
/// here; call [`delete_empty_albums`] afterwards.
pub fn regroup_albums_in_dirs(conn: &Connection, dirs: &HashSet<PathBuf>) -> Result<()> {
    let mut st = conn.prepare_cached(&format!(
        "{GROUPED_TRACK_SELECT} WHERE t.path >= ?1 AND t.path < ?2"
    ))?;
    let mut rows = Vec::new();
    for dir in dirs {
        let lower = dir.to_string_lossy().into_owned();
        // Every path that starts with `lower`; the filter below keeps only
        // the files directly inside `dir`.
        let upper = format!("{lower}\u{10FFFF}");
        for row in st.query_map([&lower, &upper], grouped_track)? {
            let row = row?;
            if Path::new(&row.path).parent() == Some(dir.as_path()) {
                rows.push(row);
            }
        }
    }
    apply_regroup(conn, rows)
}

fn regroup_all_albums(conn: &Connection) -> Result<()> {
    let mut st = conn.prepare(GROUPED_TRACK_SELECT)?;
    let rows = st
        .query_map([], grouped_track)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    apply_regroup(conn, rows)
}

fn apply_regroup(conn: &Connection, rows: Vec<GroupedTrack>) -> Result<()> {
    let mut groups: HashMap<(PathBuf, String, String), Vec<GroupedTrack>> = HashMap::new();
    for row in rows {
        let dir = Path::new(&row.path)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let key = (
            dir,
            row.title.to_lowercase(),
            row.album_artist.to_lowercase(),
        );
        groups.entry(key).or_default().push(row);
    }
    let mut upsert = conn.prepare_cached(
        "INSERT INTO albums(title, album_artist, year, cover) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(title, album_artist, COALESCE(year, -1)) DO UPDATE SET
            cover = COALESCE(cover, excluded.cover)
         RETURNING id",
    )?;
    let mut move_track = conn.prepare_cached("UPDATE tracks SET album_id = ?1 WHERE id = ?2")?;
    for members in groups.values() {
        let year = pick_album_year(members.iter().map(|m| m.year));
        let first = &members[0];
        let settled = members
            .iter()
            .all(|m| m.album_id == first.album_id && m.album_year == year);
        if settled {
            continue;
        }
        let cover = members
            .iter()
            .filter(|m| m.album_year == year)
            .chain(members.iter())
            .find_map(|m| m.cover.clone());
        let target: i64 = upsert.query_row(
            rusqlite::params![first.title, first.album_artist, year, cover],
            |r| r.get(0),
        )?;
        for m in members.iter().filter(|m| m.album_id != target) {
            move_track.execute([target, m.track_id])?;
        }
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
        .query_row("SELECT cover FROM playlists WHERE id = ?1", [id], |r| {
            r.get(0)
        })
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
        .query_row("SELECT cover FROM playlists WHERE id = ?1", [id], |r| {
            r.get(0)
        })
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
    let matched: Option<(i64, String, String, String, String, Option<i64>)> = conn
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

    let duration_ms =
        lib_duration.or_else(|| ev.duration_ms.map(|d| i64::try_from(d).unwrap_or(i64::MAX)));
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
    let having = if order_by_time {
        ""
    } else {
        "HAVING plays > 0"
    };
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

// ---------------------------------------------------------------------
// Artists & profiles
// ---------------------------------------------------------------------

/// Feature markers, matched ASCII-case-insensitively. Longer variants come
/// first so `" feat. "` wins over `" feat "`.
const FEATURE_TOKENS: &[&str] = &[
    "(featuring ",
    "[featuring ",
    "(feat.",
    "[feat.",
    "(ft.",
    "[ft.",
    " featuring ",
    " feat. ",
    " feat ",
    " ft. ",
    " ft ",
];

/// Splits compound artist strings (`"A feat. B"`, `"A, B"`, `"A; B"`) into
/// distinct artists. Group names stay whole: `&` and `+` never split, a part
/// starting with "The" belongs to the name before it (`"Tyler, The
/// Creator"`), and a list of three or more single words joined by a final
/// `&`/`and` is a band (`"Earth, Wind & Fire"`).
///
/// The UI mirrors these rules in `src/lib/artistNames.ts`; both are checked
/// against `fixtures/artist-split-cases.json`.
pub fn split_artists(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for segment in feature_segments(raw.trim()) {
        for piece in segment.split(';') {
            for name in split_name_list(piece) {
                if seen.insert(name.to_lowercase()) {
                    out.push(name);
                }
            }
        }
    }
    out
}

/// Cuts `raw` at feature markers. Matching runs on the original bytes: the
/// markers are ASCII, so cut points are always char boundaries (lowercasing
/// first would shift byte offsets for letters such as `İ`).
fn feature_segments(raw: &str) -> Vec<&str> {
    let bytes = raw.as_bytes();
    let mut segments = Vec::new();
    let (mut start, mut i) = (0, 0);
    while i < bytes.len() {
        let hit = FEATURE_TOKENS.iter().find(|tok| {
            bytes
                .get(i..i + tok.len())
                .is_some_and(|b| b.eq_ignore_ascii_case(tok.as_bytes()))
        });
        match hit {
            Some(tok) => {
                segments.push(&raw[start..i]);
                i += tok.len();
                start = i;
            }
            None => i += 1,
        }
    }
    segments.push(&raw[start..]);
    segments
}

/// Drops a closing bracket left over from `"(feat. X)"`.
fn strip_unmatched_close(part: &str) -> &str {
    let mut p = part.trim();
    if let Some(s) = p.strip_suffix(')').filter(|s| !s.contains('(')) {
        p = s.trim();
    }
    if let Some(s) = p.strip_suffix(']').filter(|s| !s.contains('[')) {
        p = s.trim();
    }
    p
}

fn starts_with_ignore_ascii_case(s: &str, prefix: &str) -> bool {
    s.as_bytes()
        .get(..prefix.len())
        .is_some_and(|b| b.eq_ignore_ascii_case(prefix.as_bytes()))
}

/// Splits one comma-separated name list, keeping group names whole.
fn split_name_list(list: &str) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    for part in list
        .split(',')
        .map(strip_unmatched_close)
        .filter(|p| !p.is_empty())
    {
        match parts.last_mut() {
            Some(prev) if starts_with_ignore_ascii_case(part, "the ") => {
                prev.push_str(", ");
                prev.push_str(part);
            }
            _ => parts.push(part.to_owned()),
        }
    }
    if parts.len() > 1 && is_single_word_band(&parts) {
        return vec![parts.join(", ")];
    }
    parts
}

/// `"Earth, Wind & Fire"`, `"Crosby, Stills, Nash & Young"`,
/// `"Peter, Paul and Mary"`: three or more single words, the last two joined
/// by `&` or `and`.
fn is_single_word_band(parts: &[String]) -> bool {
    let Some((last, rest)) = parts.split_last() else {
        return false;
    };
    let lower = last.to_lowercase();
    let Some((a, b)) = lower
        .split_once(" & ")
        .or_else(|| lower.split_once(" and "))
    else {
        return false;
    };
    let single = |w: &str| !w.trim().is_empty() && !w.trim().contains(char::is_whitespace);
    rest.len() + 2 >= 3 && rest.iter().all(|w| single(w)) && single(a) && single(b)
}

pub fn sync_track_artists(conn: &Connection, track_id: i64, raw_artist: &str) -> Result<()> {
    conn.execute("DELETE FROM track_artists WHERE track_id = ?1", [track_id])?;
    let mut st = conn.prepare_cached(
        "INSERT OR IGNORE INTO track_artists(track_id, artist, position)
         VALUES (?1, ?2, ?3)",
    )?;
    for (idx, name) in split_artists(raw_artist).into_iter().enumerate() {
        st.execute(rusqlite::params![track_id, name, idx as i64])?;
    }
    Ok(())
}

fn backfill_track_artists(conn: &Connection) -> Result<()> {
    let mut st = conn.prepare("SELECT id, artist FROM tracks")?;
    let rows: Vec<(i64, String)> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (id, artist) in rows {
        sync_track_artists(conn, id, &artist)?;
    }
    Ok(())
}

pub fn search_artists_page(
    conn: &Connection,
    offset: u64,
    limit: u64,
    query: &str,
) -> Result<Page<ArtistRow>> {
    let f = build_filter(query, |i| format!("ta.artist LIKE ?{i} ESCAPE '\\'"));
    let n = f.params.len();
    let count_sql = format!(
        "SELECT COUNT(DISTINCT ta.artist) FROM track_artists ta {}",
        f.where_sql
    );
    let page_sql = format!(
        "SELECT MIN(ta.artist),
                COUNT(DISTINCT ta.track_id) AS tracks_n,
                COUNT(DISTINCT t.album_id) AS albums_n,
                COALESCE(MAX(am.image), MAX(a.cover)),
                COALESCE((
                    SELECT SUM({PLAY_EXPR}) FROM listen_events e
                    WHERE e.artist = ta.artist COLLATE NOCASE
                ), 0),
                COALESCE((
                    SELECT SUM(e.listened_ms) FROM listen_events e
                    WHERE e.artist = ta.artist COLLATE NOCASE
                ), 0)
         FROM track_artists ta
         JOIN tracks t ON t.id = ta.track_id
         LEFT JOIN albums a ON a.id = t.album_id
         LEFT JOIN artists_meta am ON am.name_key = ta.artist
         {}
         GROUP BY ta.artist
         ORDER BY MIN(ta.artist) COLLATE NOCASE
         LIMIT ?{} OFFSET ?{}",
        f.where_sql,
        n + 1,
        n + 2
    );
    query_page(conn, &count_sql, &page_sql, f.params, offset, limit, |r| {
        Ok(ArtistRow {
            name: r.get(0)?,
            track_count: r.get::<_, i64>(1)?.max(0) as u32,
            album_count: r.get::<_, i64>(2)?.max(0) as u32,
            cover: r.get(3)?,
            play_count: u64_col(r, 4)?,
            listened_ms: u64_col(r, 5)?,
        })
    })
}

pub fn artist_meta(conn: &Connection, name: &str) -> Result<Option<ArtistMeta>> {
    conn.query_row(
        "SELECT display, image, bio, fetched_at
         FROM artists_meta
         WHERE name_key = ?1 COLLATE NOCASE",
        [name],
        |r| {
            Ok(ArtistMeta {
                name: r.get(0)?,
                image: r.get(1)?,
                bio: r.get(2)?,
                fetched_at: r.get(3)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

pub fn save_artist_meta(
    conn: &Connection,
    name: &str,
    image: Option<&str>,
    bio: Option<&str>,
    fetched_at: i64,
) -> Result<ArtistMeta> {
    conn.execute(
        "INSERT INTO artists_meta(name_key, display, image, bio, fetched_at)
         VALUES (?1, ?1, ?2, ?3, ?4)
         ON CONFLICT(name_key) DO UPDATE SET
            display = excluded.display,
            image = COALESCE(excluded.image, image),
            bio = COALESCE(excluded.bio, bio),
            fetched_at = excluded.fetched_at",
        rusqlite::params![name, image, bio, fetched_at],
    )?;
    artist_meta(conn, name)?.ok_or_else(|| super::LibraryError::Other("meta save failed".into()))
}

pub fn artist_queue_tracks(conn: &Connection, name: &str) -> Result<Vec<Track>> {
    let mut st = conn.prepare(
        "SELECT t.path, t.title, t.artist
         FROM track_artists ta
         JOIN tracks t ON t.id = ta.track_id
         LEFT JOIN albums a ON a.id = t.album_id
         WHERE ta.artist = ?1 COLLATE NOCASE
         ORDER BY COALESCE(a.year, 0) DESC, a.title COLLATE NOCASE,
                  t.disc_no, t.track_no, t.title COLLATE NOCASE, t.id",
    )?;
    let rows = st.query_map([name], queue_track)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn artist_profile(conn: &Connection, name: &str) -> Result<ArtistProfile> {
    let mut tr_st = conn.prepare(&format!(
        "{TRACK_SELECT}
         JOIN track_artists ta ON ta.track_id = t.id
         WHERE ta.artist = ?1 COLLATE NOCASE
         ORDER BY COALESCE(a.year, 0) DESC, a.title COLLATE NOCASE,
                  t.disc_no, t.track_no, t.title COLLATE NOCASE, t.id"
    ))?;
    let tracks = tr_st
        .query_map([name], track_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut al_st = conn.prepare(&format!(
        "{ALBUM_SELECT}
         WHERE a.album_artist = ?1 COLLATE NOCASE
            OR EXISTS (
                SELECT 1 FROM tracks t
                JOIN track_artists ta ON ta.track_id = t.id
                WHERE t.album_id = a.id AND ta.artist = ?1 COLLATE NOCASE
            )
         ORDER BY COALESCE(a.year, 0) DESC, a.title COLLATE NOCASE, a.id"
    ))?;
    let albums = al_st
        .query_map([name], album_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut pop_st = conn.prepare(&format!(
        "SELECT t.id, t.title, t.artist, COALESCE(a.title, ''), t.album_id,
                t.track_no, t.disc_no, t.year, t.duration_ms, t.path, a.cover,
                COALESCE(SUM({PLAY_EXPR}), 0) AS plays,
                COALESCE(SUM(e.listened_ms), 0) AS total_ms
         FROM track_artists ta
         JOIN tracks t ON t.id = ta.track_id
         LEFT JOIN albums a ON a.id = t.album_id
         LEFT JOIN listen_events e ON e.track_id = t.id
         WHERE ta.artist = ?1 COLLATE NOCASE
         GROUP BY t.id
         ORDER BY plays DESC, total_ms DESC, t.disc_no, t.track_no, t.title COLLATE NOCASE
         LIMIT 10"
    ))?;
    let popular_tracks = pop_st
        .query_map([name], |r| {
            let track = track_row(r)?;
            let play_count = u64_col(r, 11)?;
            let listened_ms = u64_col(r, 12)?;
            Ok(ArtistTopTrack {
                track,
                play_count,
                listened_ms,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let (play_count, listened_ms): (u64, u64) = conn.query_row(
        &format!(
            "SELECT COALESCE(SUM({PLAY_EXPR}), 0), COALESCE(SUM(e.listened_ms), 0)
             FROM listen_events e
             WHERE e.artist = ?1 COLLATE NOCASE
                OR EXISTS (
                    SELECT 1 FROM track_artists ta
                    WHERE ta.track_id = e.track_id AND ta.artist = ?1 COLLATE NOCASE
                )"
        ),
        [name],
        |r| Ok((u64_col(r, 0)?, u64_col(r, 1)?)),
    )?;

    let mut gen_st = conn.prepare(
        "SELECT TRIM(t.genre) AS g, COUNT(*) AS n
         FROM track_artists ta
         JOIN tracks t ON t.id = ta.track_id
         WHERE ta.artist = ?1 COLLATE NOCASE AND TRIM(COALESCE(t.genre, '')) != ''
         GROUP BY LOWER(TRIM(t.genre))
         ORDER BY n DESC, g COLLATE NOCASE
         LIMIT 5",
    )?;
    let genres = gen_st
        .query_map([name], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let meta = artist_meta(conn, name)?;
    let canonical = tracks
        .first()
        .and_then(|t| {
            split_artists(&t.artist)
                .into_iter()
                .find(|a| a.eq_ignore_ascii_case(name))
        })
        .or_else(|| meta.as_ref().map(|m| m.name.clone()))
        .unwrap_or_else(|| name.to_owned());

    let total_duration_ms: u64 = tracks.iter().filter_map(|t| t.duration_ms).sum();
    let fallback_cover = albums
        .iter()
        .find_map(|a| a.cover.clone())
        .or_else(|| tracks.iter().find_map(|t| t.cover.clone()));

    Ok(ArtistProfile {
        name: canonical,
        track_count: tracks.len() as u32,
        album_count: albums.len() as u32,
        total_duration_ms,
        genres,
        play_count,
        listened_ms,
        image: meta.as_ref().and_then(|m| m.image.clone()),
        bio: meta.as_ref().and_then(|m| m.bio.clone()),
        fetched_at: meta.as_ref().map(|m| m.fetched_at),
        fallback_cover,
        popular_tracks,
        albums,
        tracks,
    })
}

// ---------------------------------------------------------------------
// Tag history (undo for automatic metadata matching)
// ---------------------------------------------------------------------

/// One saved "before" state of a track's tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagHistoryRow {
    pub id: i64,
    pub batch_id: i64,
    pub track_id: Option<i64>,
    pub path: String,
    pub old_json: String,
}

/// A fresh batch id (one per apply call).
pub fn next_tag_batch_id(conn: &Connection) -> Result<i64> {
    let max: Option<i64> =
        conn.query_row("SELECT MAX(batch_id) FROM tag_history", [], |r| r.get(0))?;
    Ok(max.unwrap_or(0) + 1)
}

pub fn insert_tag_history(
    conn: &Connection,
    batch_id: i64,
    track_id: Option<i64>,
    path: &str,
    saved_at: i64,
    old_json: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO tag_history(batch_id, track_id, path, saved_at, old_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![batch_id, track_id, path, saved_at, old_json],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_tag_history_row(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM tag_history WHERE id = ?1", [id])?;
    Ok(())
}

pub fn tag_history_batch(conn: &Connection, batch_id: i64) -> Result<Vec<TagHistoryRow>> {
    let mut st = conn.prepare(
        "SELECT id, batch_id, track_id, path, old_json FROM tag_history
         WHERE batch_id = ?1 ORDER BY id",
    )?;
    let rows = st.query_map([batch_id], |r| {
        Ok(TagHistoryRow {
            id: r.get(0)?,
            batch_id: r.get(1)?,
            track_id: r.get(2)?,
            path: r.get(3)?,
            old_json: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// Id of the library track stored for `path`.
pub fn track_id_by_path(conn: &Connection, path: &str) -> Result<Option<i64>> {
    Ok(conn
        .query_row("SELECT id FROM tracks WHERE path = ?1", [path], |r| {
            r.get(0)
        })
        .optional()?)
}

/// Album row of the album a track currently belongs to.
pub fn track_album(conn: &Connection, track_id: i64) -> Result<Option<AlbumRow>> {
    Ok(conn
        .query_row(
            &format!("{ALBUM_SELECT} WHERE a.id = (SELECT album_id FROM tracks WHERE id = ?1)"),
            [track_id],
            album_row,
        )
        .optional()?)
}

/// One track by id, in the same shape as album listings.
pub fn track_by_id(conn: &Connection, track_id: i64) -> Result<Option<TrackRow>> {
    Ok(conn
        .query_row(
            &format!("{TRACK_SELECT} WHERE t.id = ?1"),
            [track_id],
            track_row,
        )
        .optional()?)
}

pub fn set_album_cover(conn: &Connection, album_id: i64, cover: &str) -> Result<()> {
    conn.execute(
        "UPDATE albums SET cover = ?1 WHERE id = ?2",
        rusqlite::params![cover, album_id],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------
// Home feed & local rule-based dynamic mixes
// ---------------------------------------------------------------------

const SECS_PER_DAY: i64 = 86_400;
const MIN_MIX_TRACKS: usize = 3;
const MIX_LIMIT: i64 = 25;

const MIX_WEEKLY_FAVORITES: &str = "weekly-favorites";
const MIX_ON_REPEAT: &str = "on-repeat";
const MIX_REDISCOVER: &str = "rediscover";
const MIX_FOCUS: &str = "focus-mix";

fn mix_album_covers(tracks: &[TrackRow]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut covers = Vec::with_capacity(4);
    for t in tracks {
        if let Some(c) = t.cover.as_deref()
            && !c.is_empty()
            && seen.insert(c.to_string())
        {
            covers.push(c.to_string());
            if covers.len() == 4 {
                break;
            }
        }
    }
    covers
}

fn build_mix_meta(
    id: &str,
    kind: &str,
    title_key: &str,
    subject: Option<String>,
    subtitle_key: &str,
    tracks: &[TrackRow],
) -> DynamicMix {
    DynamicMix {
        id: id.to_string(),
        kind: kind.to_string(),
        title_key: title_key.to_string(),
        subject,
        subtitle_key: subtitle_key.to_string(),
        track_count: u32::try_from(tracks.len()).unwrap_or(u32::MAX),
        album_covers: mix_album_covers(tracks),
    }
}

fn daily_rank(track_id: i64, day_seed: i64) -> u64 {
    let mut x = (track_id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (day_seed as u64).wrapping_add(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn has_listen_history(conn: &Connection) -> Result<bool> {
    let exists: bool = conn.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM listen_events e WHERE {PLAY_EXPR} = 1)"),
        [],
        |r| r.get(0),
    )?;
    Ok(exists)
}

fn recently_played_tracks(conn: &Connection, limit: i64) -> Result<Vec<RecentTrack>> {
    let sql = format!(
        "SELECT t.id, t.title, t.artist, COALESCE(a.title, ''), t.album_id,
                t.track_no, t.disc_no, t.year, t.duration_ms, t.path, a.cover,
                MAX(e.started_at) AS last_started
         FROM listen_events e
         LEFT JOIN tracks tp ON e.track_id IS NULL AND tp.path = e.path
         JOIN tracks t ON t.id = COALESCE(e.track_id, tp.id)
         LEFT JOIN albums a ON a.id = t.album_id
         WHERE {PLAY_EXPR} = 1
         GROUP BY t.id
         ORDER BY last_started DESC, t.id DESC
         LIMIT ?1"
    );
    let mut st = conn.prepare(&sql)?;
    let rows = st.query_map([limit], |r| {
        Ok(RecentTrack {
            track: track_row(r)?,
            started_at: r.get(11)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

fn recently_played_albums(conn: &Connection, limit: i64) -> Result<Vec<AlbumRow>> {
    let mut st = conn.prepare(&format!(
        "SELECT a.id, a.title, a.album_artist, a.year,
                (SELECT COUNT(*) FROM tracks WHERE album_id = a.id), a.cover
         FROM listen_events e
         LEFT JOIN tracks tp ON e.track_id IS NULL AND tp.path = e.path
         JOIN tracks t ON t.id = COALESCE(e.track_id, tp.id)
         JOIN albums a ON a.id = t.album_id
         WHERE {PLAY_EXPR} = 1
         GROUP BY a.id
         ORDER BY MAX(e.started_at) DESC, a.id DESC
         LIMIT ?1"
    ))?;
    let rows = st.query_map([limit], album_row)?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

fn recently_added_albums(conn: &Connection, limit: i64) -> Result<Vec<AlbumRow>> {
    let mut st = conn.prepare(&format!(
        "{ALBUM_SELECT}
         ORDER BY (SELECT COALESCE(MAX(t.added_at), 0) FROM tracks t WHERE t.album_id = a.id) DESC,
                  a.id DESC
         LIMIT ?1"
    ))?;
    let rows = st.query_map([limit], album_row)?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

fn weekly_favorites_tracks(conn: &Connection, now: i64) -> Result<Vec<TrackRow>> {
    let since = now.saturating_sub(7 * SECS_PER_DAY);
    let sql = format!(
        "SELECT t.id, t.title, t.artist, COALESCE(a.title, ''), t.album_id,
                t.track_no, t.disc_no, t.year, t.duration_ms, t.path, a.cover
         FROM listen_events e
         LEFT JOIN tracks tp ON e.track_id IS NULL AND tp.path = e.path
         JOIN tracks t ON t.id = COALESCE(e.track_id, tp.id)
         LEFT JOIN albums a ON a.id = t.album_id
         WHERE e.started_at >= ?1
         GROUP BY t.id
         HAVING SUM(
             e.completed * 3.0
             + MIN(e.listened_ms, 600000) / 30000.0
             - e.skipped * 2.0
         ) > 0
           AND SUM({PLAY_EXPR}) >= 1
         ORDER BY SUM(
             e.completed * 3.0
             + MIN(e.listened_ms, 600000) / 30000.0
             - e.skipped * 2.0
         ) DESC,
         MAX(e.started_at) DESC,
         t.id ASC
         LIMIT ?2"
    );
    let mut st = conn.prepare(&sql)?;
    let rows = st.query_map(rusqlite::params![since, MIX_LIMIT], track_row)?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

fn on_repeat_tracks(conn: &Connection, now: i64) -> Result<Vec<TrackRow>> {
    let since = now.saturating_sub(30 * SECS_PER_DAY);
    let sql = format!(
        "SELECT t.id, t.title, t.artist, COALESCE(a.title, ''), t.album_id,
                t.track_no, t.disc_no, t.year, t.duration_ms, t.path, a.cover
         FROM listen_events e
         LEFT JOIN tracks tp ON e.track_id IS NULL AND tp.path = e.path
         JOIN tracks t ON t.id = COALESCE(e.track_id, tp.id)
         LEFT JOIN albums a ON a.id = t.album_id
         WHERE e.started_at >= ?1
         GROUP BY t.id
         HAVING SUM({PLAY_EXPR}) >= 2
         ORDER BY SUM({PLAY_EXPR}) DESC, SUM(e.listened_ms) DESC, MAX(e.started_at) DESC, t.id ASC
         LIMIT ?2"
    );
    let mut st = conn.prepare(&sql)?;
    let rows = st.query_map(rusqlite::params![since, MIX_LIMIT], track_row)?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

fn rediscover_tracks(conn: &Connection, now: i64) -> Result<Vec<TrackRow>> {
    let recent_cutoff = now.saturating_sub(30 * SECS_PER_DAY);
    let sql = format!(
        "{TRACK_SELECT}
         WHERE (
             EXISTS (SELECT 1 FROM favorites f WHERE f.track_id = t.id)
             OR EXISTS (
                 SELECT 1 FROM listen_events e
                 WHERE (e.track_id = t.id OR (e.track_id IS NULL AND e.path = t.path))
                   AND (e.completed = 1 OR e.listened_ms >= 30000)
             )
         )
         AND NOT EXISTS (
             SELECT 1 FROM listen_events e
             WHERE (e.track_id = t.id OR (e.track_id IS NULL AND e.path = t.path))
               AND e.started_at >= ?1
               AND {PLAY_EXPR} = 1
         )"
    );
    let mut st = conn.prepare(&sql)?;
    let mut items = st
        .query_map([recent_cutoff], track_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let day_seed = now.div_euclid(SECS_PER_DAY);
    items.sort_by_key(|tr| daily_rank(tr.id, day_seed));
    items.truncate(MIX_LIMIT as usize);
    Ok(items)
}

fn focus_mix_data(conn: &Connection, now: i64) -> Result<Option<(String, Vec<TrackRow>)>> {
    let since = now.saturating_sub(14 * SECS_PER_DAY);
    let top_artist: Option<String> = conn
        .query_row(
            "SELECT ta.artist
             FROM listen_events e
             LEFT JOIN tracks tp ON e.track_id IS NULL AND tp.path = e.path
             JOIN tracks t ON t.id = COALESCE(e.track_id, tp.id)
             JOIN track_artists ta ON ta.track_id = t.id
             WHERE e.started_at >= ?1
               AND e.skipped = 0
               AND TRIM(ta.artist) != ''
               AND (
                   SELECT COUNT(DISTINCT ta2.track_id)
                   FROM track_artists ta2
                   WHERE ta2.artist = ta.artist
               ) >= ?2
             GROUP BY ta.artist COLLATE NOCASE
             ORDER BY SUM(e.listened_ms) DESC, COUNT(*) DESC
             LIMIT 1",
            rusqlite::params![since, MIN_MIX_TRACKS as i64],
            |r| r.get(0),
        )
        .optional()?;

    if let Some(artist) = top_artist {
        let mut st = conn.prepare(&format!(
            "{TRACK_SELECT}
             WHERE EXISTS (
                 SELECT 1 FROM track_artists ta
                 WHERE ta.track_id = t.id AND ta.artist = ?1 COLLATE NOCASE
             )
             ORDER BY COALESCE((
                 SELECT SUM(e.completed * 2 + e.listened_ms / 30000.0 - e.skipped)
                 FROM listen_events e
                 WHERE (e.track_id = t.id OR (e.track_id IS NULL AND e.path = t.path))
                   AND e.started_at >= ?2
             ), 0) DESC, t.added_at DESC, t.id ASC
             LIMIT ?3"
        ))?;
        let rows = st.query_map(rusqlite::params![&artist, since, MIX_LIMIT], track_row)?;
        let tracks = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        if tracks.len() >= MIN_MIX_TRACKS {
            return Ok(Some((artist, tracks)));
        }
    }

    let top_genre: Option<String> = conn
        .query_row(
            "SELECT TRIM(t.genre)
             FROM listen_events e
             LEFT JOIN tracks tp ON e.track_id IS NULL AND tp.path = e.path
             JOIN tracks t ON t.id = COALESCE(e.track_id, tp.id)
             WHERE e.started_at >= ?1
               AND e.skipped = 0
               AND t.genre IS NOT NULL AND TRIM(t.genre) != ''
               AND (
                   SELECT COUNT(*)
                   FROM tracks t2
                   WHERE TRIM(COALESCE(t2.genre, '')) = TRIM(t.genre) COLLATE NOCASE
               ) >= ?2
             GROUP BY TRIM(t.genre) COLLATE NOCASE
             ORDER BY SUM(e.listened_ms) DESC, COUNT(*) DESC
             LIMIT 1",
            rusqlite::params![since, MIN_MIX_TRACKS as i64],
            |r| r.get(0),
        )
        .optional()?;

    if let Some(genre) = top_genre {
        let mut st = conn.prepare(&format!(
            "{TRACK_SELECT}
             WHERE TRIM(COALESCE(t.genre, '')) = ?1 COLLATE NOCASE
             ORDER BY COALESCE((
                 SELECT SUM(e.completed * 2 + e.listened_ms / 30000.0 - e.skipped)
                 FROM listen_events e
                 WHERE (e.track_id = t.id OR (e.track_id IS NULL AND e.path = t.path))
                   AND e.started_at >= ?2
             ), 0) DESC, t.added_at DESC, t.id ASC
             LIMIT ?3"
        ))?;
        let rows = st.query_map(rusqlite::params![&genre, since, MIX_LIMIT], track_row)?;
        let tracks = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        if tracks.len() >= MIN_MIX_TRACKS {
            return Ok(Some((genre, tracks)));
        }
    }

    Ok(None)
}

fn mix_detail_for_id(conn: &Connection, mix_id: &str, now: i64) -> Result<Option<HomeMixDetail>> {
    match mix_id {
        MIX_WEEKLY_FAVORITES => {
            let tracks = weekly_favorites_tracks(conn, now)?;
            if tracks.len() < MIN_MIX_TRACKS {
                return Ok(None);
            }
            let mix = build_mix_meta(
                MIX_WEEKLY_FAVORITES,
                MIX_WEEKLY_FAVORITES,
                "Weekly Favorites",
                None,
                "Most played and finished over the last 7 days",
                &tracks,
            );
            Ok(Some(HomeMixDetail { mix, tracks }))
        }
        MIX_ON_REPEAT => {
            let tracks = on_repeat_tracks(conn, now)?;
            if tracks.len() < MIN_MIX_TRACKS {
                return Ok(None);
            }
            let mix = build_mix_meta(
                MIX_ON_REPEAT,
                MIX_ON_REPEAT,
                "On Repeat",
                None,
                "Songs you kept coming back to this month",
                &tracks,
            );
            Ok(Some(HomeMixDetail { mix, tracks }))
        }
        MIX_REDISCOVER => {
            let tracks = rediscover_tracks(conn, now)?;
            if tracks.len() < MIN_MIX_TRACKS {
                return Ok(None);
            }
            let mix = build_mix_meta(
                MIX_REDISCOVER,
                MIX_REDISCOVER,
                "Rediscover",
                None,
                "Favorites and past listens you haven't played lately",
                &tracks,
            );
            Ok(Some(HomeMixDetail { mix, tracks }))
        }
        MIX_FOCUS => {
            let Some((subject, tracks)) = focus_mix_data(conn, now)? else {
                return Ok(None);
            };
            let mix = build_mix_meta(
                MIX_FOCUS,
                MIX_FOCUS,
                "{name} Mix",
                Some(subject),
                "Inspired by what you played most lately",
                &tracks,
            );
            Ok(Some(HomeMixDetail { mix, tracks }))
        }
        _ => Ok(None),
    }
}

pub fn home_mix_detail(conn: &Connection, mix_id: &str, now: i64) -> Result<Option<HomeMixDetail>> {
    mix_detail_for_id(conn, mix_id, now)
}

pub fn home_mix_queue_tracks(conn: &Connection, mix_id: &str, now: i64) -> Result<Vec<Track>> {
    let Some(detail) = mix_detail_for_id(conn, mix_id, now)? else {
        return Ok(Vec::new());
    };
    Ok(detail
        .tracks
        .into_iter()
        .map(|t| Track {
            path: PathBuf::from(t.path),
            title: Some(t.title),
            artist: Some(t.artist),
        })
        .collect())
}

pub fn save_mix_as_playlist(
    conn: &mut Connection,
    mix_id: &str,
    name: &str,
    now: i64,
) -> Result<PlaylistInfo> {
    let detail = mix_detail_for_id(conn, mix_id, now)?
        .ok_or_else(|| super::LibraryError::Other(format!("mix '{mix_id}' is unavailable")))?;
    let ids: Vec<i64> = detail.tracks.iter().map(|t| t.id).collect();
    create_playlist(conn, name, &ids, now)
}

pub fn home_feed(conn: &Connection, now: i64) -> Result<HomeFeed> {
    let has_history = has_listen_history(conn)?;
    let recently_played_albums = if has_history {
        recently_played_albums(conn, 12)?
    } else {
        Vec::new()
    };
    let recently_played_tracks = if has_history {
        recently_played_tracks(conn, 8)?
    } else {
        Vec::new()
    };
    let recently_added_albums = recently_added_albums(conn, 12)?;
    let favs = favorites_page(conn, 0, 8)?;

    let mut mixes = Vec::new();
    for id in [
        MIX_WEEKLY_FAVORITES,
        MIX_ON_REPEAT,
        MIX_REDISCOVER,
        MIX_FOCUS,
    ] {
        if let Some(detail) = mix_detail_for_id(conn, id, now)? {
            mixes.push(detail.mix);
        }
    }

    Ok(HomeFeed {
        recently_played_albums,
        recently_played_tracks,
        recently_added_albums,
        favorite_tracks: favs.items,
        favorite_count: favs.total,
        mixes,
        has_history,
    })
}
