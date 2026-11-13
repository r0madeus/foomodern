//! The one and only play queue (architecture rule 2). Pure data structure:
//! no engine, no Tauri. The player owns an instance and keeps the audio
//! engine's "next" slot in sync with it.

use serde::Serialize;
use std::path::PathBuf;

/// Stable identity of a queue entry. Survives moves and removals of other
/// entries; never reused within one queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct EntryId(u64);

impl EntryId {
    pub fn value(self) -> u64 {
        self.0
    }
}

/// What gets queued: a file plus the tags we already know (from the
/// library). Plain paths (e.g. "Open files") convert without tags.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Track {
    pub path: PathBuf,
    pub title: Option<String>,
    pub artist: Option<String>,
}

impl From<PathBuf> for Track {
    fn from(path: PathBuf) -> Self {
        Track {
            path,
            ..Track::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueEntry {
    pub id: EntryId,
    pub path: PathBuf,
    pub title: Option<String>,
    pub artist: Option<String>,
}

#[derive(Debug, Default)]
pub struct Queue {
    entries: Vec<QueueEntry>,
    current: Option<usize>,
    next_id: u64,
    version: u64,
}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    fn make_entry(&mut self, track: Track) -> QueueEntry {
        let id = EntryId(self.next_id);
        self.next_id += 1;
        QueueEntry {
            id,
            path: track.path,
            title: track.title,
            artist: track.artist,
        }
    }

    /// Replace all entries. `start` becomes the current index when in range,
    /// otherwise there is no current entry.
    pub fn replace<T: Into<Track>>(&mut self, tracks: Vec<T>, start: Option<usize>) {
        let entries: Vec<QueueEntry> = tracks
            .into_iter()
            .map(|t| self.make_entry(t.into()))
            .collect();
        self.entries = entries;
        self.current = start.filter(|&i| i < self.entries.len());
        self.version += 1;
    }

    pub fn append<T: Into<Track>>(&mut self, tracks: Vec<T>) {
        if tracks.is_empty() {
            return;
        }
        for t in tracks {
            let e = self.make_entry(t.into());
            self.entries.push(e);
        }
        self.version += 1;
    }

    /// Remove the entry at `index`. Removing the current entry leaves the
    /// queue without a current entry.
    pub fn remove(&mut self, index: usize) -> Option<QueueEntry> {
        if index >= self.entries.len() {
            return None;
        }
        let removed = self.entries.remove(index);
        self.current = match self.current {
            Some(c) if c == index => None,
            Some(c) if c > index => Some(c - 1),
            other => other,
        };
        self.version += 1;
        Some(removed)
    }

    /// Move the entry at `from` so that it ends up at index `to`.
    /// Returns false (and changes nothing) when either index is out of range.
    pub fn move_entry(&mut self, from: usize, to: usize) -> bool {
        let len = self.entries.len();
        if from >= len || to >= len {
            return false;
        }
        if from == to {
            return true;
        }
        let entry = self.entries.remove(from);
        self.entries.insert(to, entry);
        self.current = self.current.map(|c| {
            if c == from {
                to
            } else if from < to && c > from && c <= to {
                c - 1
            } else if from > to && c >= to && c < from {
                c + 1
            } else {
                c
            }
        });
        self.version += 1;
        true
    }

    /// Does not change `version`. Out-of-range values clear the current entry.
    pub fn set_current(&mut self, index: Option<usize>) {
        self.current = index.filter(|&i| i < self.entries.len());
    }

    pub fn current(&self) -> Option<&QueueEntry> {
        self.current.and_then(|i| self.entries.get(i))
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    /// The entry after the current one (no repeat / shuffle yet).
    pub fn next(&self) -> Option<&QueueEntry> {
        self.current.and_then(|i| self.entries.get(i + 1))
    }

    pub fn previous_index(&self) -> Option<usize> {
        self.current.and_then(|i| i.checked_sub(1))
    }

    pub fn index_of(&self, id: EntryId) -> Option<usize> {
        self.entries.iter().position(|e| e.id == id)
    }

    pub fn get(&self, index: usize) -> Option<&QueueEntry> {
        self.entries.get(index)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn page(&self, offset: usize, limit: usize) -> &[QueueEntry] {
        let start = offset.min(self.entries.len());
        let end = start.saturating_add(limit).min(self.entries.len());
        &self.entries[start..end]
    }

    /// Incremented on every mutation of the entries (not on `set_current`).
    pub fn version(&self) -> u64 {
        self.version
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    fn names(q: &Queue) -> Vec<String> {
        q.page(0, usize::MAX)
            .iter()
            .map(|e| e.path.to_string_lossy().into_owned())
            .collect()
    }

    fn queue(n: &[&str], cur: Option<usize>) -> Queue {
        let mut q = Queue::new();
        q.replace(paths(n), cur);
        q
    }

    #[test]
    fn replace_sets_entries_and_current() {
        let q = queue(&["a", "b", "c"], Some(1));
        assert_eq!(names(&q), ["a", "b", "c"]);
        assert_eq!(q.current_index(), Some(1));
        assert_eq!(q.current().unwrap().path, PathBuf::from("b"));
        assert_eq!(q.next().unwrap().path, PathBuf::from("c"));
        assert_eq!(q.len(), 3);
        assert!(!q.is_empty());
    }

    #[test]
    fn replace_out_of_range_start_clears_current() {
        let q = queue(&["a"], Some(5));
        assert_eq!(q.current_index(), None);
        assert!(q.next().is_none());
    }

    #[test]
    fn ids_are_unique_across_replace() {
        let mut q = queue(&["a", "b"], None);
        let old = q.get(0).unwrap().id;
        q.replace(paths(&["a"]), None);
        assert_ne!(q.get(0).unwrap().id, old);
        assert_eq!(q.index_of(old), None);
    }

    #[test]
    fn append_adds_and_bumps_version() {
        let mut q = queue(&["a"], Some(0));
        let v = q.version();
        q.append(paths(&["b", "c"]));
        assert_eq!(names(&q), ["a", "b", "c"]);
        assert_eq!(q.current_index(), Some(0));
        assert_eq!(q.version(), v + 1);
        q.append(Vec::<PathBuf>::new());
        assert_eq!(q.version(), v + 1);
    }

    #[test]
    fn remove_before_current_shifts_current() {
        let mut q = queue(&["a", "b", "c"], Some(2));
        assert_eq!(q.remove(0).unwrap().path, PathBuf::from("a"));
        assert_eq!(q.current_index(), Some(1));
        assert_eq!(q.current().unwrap().path, PathBuf::from("c"));
    }

    #[test]
    fn remove_after_current_keeps_current() {
        let mut q = queue(&["a", "b", "c"], Some(0));
        q.remove(2);
        assert_eq!(q.current_index(), Some(0));
        assert_eq!(names(&q), ["a", "b"]);
    }

    #[test]
    fn remove_current_clears_current() {
        let mut q = queue(&["a", "b", "c"], Some(1));
        q.remove(1);
        assert_eq!(q.current_index(), None);
        assert_eq!(names(&q), ["a", "c"]);
    }

    #[test]
    fn remove_out_of_range_is_noop() {
        let mut q = queue(&["a"], Some(0));
        let v = q.version();
        assert!(q.remove(1).is_none());
        assert_eq!(q.version(), v);
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn move_forward_over_current() {
        // [a b c d], current = c (2); move a -> 3 => [b c d a], c at 1
        let mut q = queue(&["a", "b", "c", "d"], Some(2));
        assert!(q.move_entry(0, 3));
        assert_eq!(names(&q), ["b", "c", "d", "a"]);
        assert_eq!(q.current().unwrap().path, PathBuf::from("c"));
    }

    #[test]
    fn move_backward_over_current() {
        let mut q = queue(&["a", "b", "c", "d"], Some(1));
        assert!(q.move_entry(3, 0));
        assert_eq!(names(&q), ["d", "a", "b", "c"]);
        assert_eq!(q.current().unwrap().path, PathBuf::from("b"));
    }

    #[test]
    fn move_current_itself() {
        let mut q = queue(&["a", "b", "c"], Some(0));
        assert!(q.move_entry(0, 2));
        assert_eq!(names(&q), ["b", "c", "a"]);
        assert_eq!(q.current_index(), Some(2));
    }

    #[test]
    fn move_not_touching_current() {
        let mut q = queue(&["a", "b", "c", "d"], Some(0));
        assert!(q.move_entry(2, 3));
        assert_eq!(q.current_index(), Some(0));
        let mut q = queue(&["a", "b", "c", "d"], Some(3));
        assert!(q.move_entry(0, 1));
        assert_eq!(q.current_index(), Some(3));
    }

    #[test]
    fn move_out_of_range_or_same() {
        let mut q = queue(&["a", "b"], Some(0));
        let v = q.version();
        assert!(!q.move_entry(0, 2));
        assert!(!q.move_entry(2, 0));
        assert!(q.move_entry(1, 1));
        assert_eq!(q.version(), v);
        assert_eq!(names(&q), ["a", "b"]);
    }

    #[test]
    fn set_current_does_not_bump_version() {
        let mut q = queue(&["a", "b"], None);
        let v = q.version();
        q.set_current(Some(1));
        assert_eq!(q.current_index(), Some(1));
        q.set_current(Some(9));
        assert_eq!(q.current_index(), None);
        q.set_current(None);
        assert_eq!(q.version(), v);
    }

    #[test]
    fn next_and_previous() {
        let mut q = queue(&["a", "b"], Some(0));
        assert_eq!(q.previous_index(), None);
        assert_eq!(q.next().unwrap().path, PathBuf::from("b"));
        q.set_current(Some(1));
        assert_eq!(q.previous_index(), Some(0));
        assert!(q.next().is_none());
        q.set_current(None);
        assert!(q.next().is_none());
        assert_eq!(q.previous_index(), None);
    }

    #[test]
    fn index_of_follows_moves() {
        let mut q = queue(&["a", "b", "c"], None);
        let id = q.get(2).unwrap().id;
        q.move_entry(2, 0);
        assert_eq!(q.index_of(id), Some(0));
        q.remove(0);
        assert_eq!(q.index_of(id), None);
    }

    #[test]
    fn page_bounds() {
        let q = queue(&["a", "b", "c"], None);
        assert_eq!(q.page(1, 1).len(), 1);
        assert_eq!(q.page(1, 10).len(), 2);
        assert_eq!(q.page(3, 10).len(), 0);
        assert_eq!(q.page(100, 10).len(), 0);
        assert_eq!(q.page(0, 0).len(), 0);
        assert_eq!(q.page(1, usize::MAX).len(), 2);
    }

    #[test]
    fn version_increments_on_mutations() {
        let mut q = Queue::new();
        let v0 = q.version();
        q.replace(paths(&["a", "b", "c"]), None);
        q.append(paths(&["d"]));
        q.remove(0);
        q.move_entry(0, 1);
        assert_eq!(q.version(), v0 + 4);
    }
}
