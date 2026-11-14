//! The one and only play queue (architecture rule 2). Pure data structure:
//! no engine, no Tauri. The player owns an instance and keeps the audio
//! engine's "next" slot in sync with it.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

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
    shuffle: bool,
    repeat: RepeatMode,
    order: Vec<EntryId>,
    rng_state: u64,
}

impl Queue {
    pub fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0x4D59_5DF4_D0F3_3173, |d| {
                (d.as_nanos() ^ (d.as_nanos() >> 64)) as u64
            });
        Self {
            rng_state: nanos | 1,
            ..Self::default()
        }
    }

    #[cfg(test)]
    pub(crate) fn with_seed(seed: u64) -> Self {
        Self {
            rng_state: seed,
            ..Self::default()
        }
    }

    fn next_random(&mut self) -> u64 {
        self.rng_state = self.rng_state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng_state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn shuffle_slice<T>(&mut self, slice: &mut [T]) {
        for i in (1..slice.len()).rev() {
            let j = (self.next_random() % (i as u64 + 1)) as usize;
            slice.swap(i, j);
        }
    }

    fn rebuild_order(&mut self) {
        self.order.clear();
        if !self.shuffle {
            return;
        }
        let mut order: Vec<EntryId> = self.entries.iter().map(|e| e.id).collect();
        self.shuffle_slice(&mut order);
        // Keep the active entry at the front so shuffled playback continues from it.
        if let Some(cur_id) = self.current().map(|e| e.id)
            && let Some(pos) = order.iter().position(|&id| id == cur_id)
        {
            order.swap(0, pos);
        }
        self.order = order;
    }

    fn current_order_pos(&self) -> Option<usize> {
        let cur_id = self.current().map(|e| e.id)?;
        self.order.iter().position(|&id| id == cur_id)
    }

    fn step(&self, forward: bool, honor_repeat_one: bool) -> Option<usize> {
        let cur = self.current?;
        if self.entries.is_empty() {
            return None;
        }
        if honor_repeat_one && self.repeat == RepeatMode::One {
            return Some(cur);
        }
        if self.shuffle {
            let pos = self.current_order_pos()?;
            let target_id = if forward {
                if pos + 1 < self.order.len() {
                    Some(self.order[pos + 1])
                } else if self.repeat == RepeatMode::All {
                    self.order.first().copied()
                } else {
                    None
                }
            } else if pos > 0 {
                Some(self.order[pos - 1])
            } else if self.repeat == RepeatMode::All {
                self.order.last().copied()
            } else {
                None
            }?;
            return self.index_of(target_id);
        }
        if forward {
            if cur + 1 < self.entries.len() {
                Some(cur + 1)
            } else if self.repeat == RepeatMode::All {
                Some(0)
            } else {
                None
            }
        } else if cur > 0 {
            Some(cur - 1)
        } else if self.repeat == RepeatMode::All {
            Some(self.entries.len() - 1)
        } else {
            None
        }
    }

    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    pub fn set_shuffle(&mut self, enabled: bool) {
        if self.shuffle == enabled {
            return;
        }
        self.shuffle = enabled;
        self.rebuild_order();
    }

    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    pub fn set_repeat(&mut self, mode: RepeatMode) {
        self.repeat = mode;
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
        self.rebuild_order();
        self.version += 1;
    }

    pub fn append<T: Into<Track>>(&mut self, tracks: Vec<T>) {
        if tracks.is_empty() {
            return;
        }
        let mut added_ids = Vec::with_capacity(tracks.len());
        for t in tracks {
            let e = self.make_entry(t.into());
            added_ids.push(e.id);
            self.entries.push(e);
        }
        if self.shuffle {
            let tail_start = self.current_order_pos().map_or(0, |pos| pos + 1);
            self.order.extend(added_ids);
            if tail_start < self.order.len() {
                let mut tail = std::mem::take(&mut self.order);
                self.shuffle_slice(&mut tail[tail_start..]);
                self.order = tail;
            }
        }
        self.version += 1;
    }

    /// Insert `track` immediately after the current entry (or at the front
    /// when nothing is selected). Even with shuffle enabled, it plays next.
    pub fn insert_next(&mut self, track: impl Into<Track>) -> EntryId {
        let at = self.current.map_or(0, |c| c + 1);
        let entry = self.make_entry(track.into());
        let id = entry.id;
        self.entries.insert(at, entry);
        if self.shuffle {
            let order_at = self.current_order_pos().map_or(0, |pos| pos + 1);
            self.order.insert(order_at, id);
        }
        self.version += 1;
        id
    }

    /// Insert `track` right after the entry `anchor`. Returns the new entry's
    /// id, or `None` (and changes nothing) when `anchor` is no longer queued.
    /// With shuffle on, the new entry gets a random place among the entries
    /// still to play.
    pub fn insert_after(&mut self, anchor: EntryId, track: impl Into<Track>) -> Option<EntryId> {
        let at = self.index_of(anchor)? + 1;
        let entry = self.make_entry(track.into());
        let id = entry.id;
        self.entries.insert(at, entry);
        if let Some(c) = self.current
            && c >= at
        {
            self.current = Some(c + 1);
        }
        if self.shuffle {
            let tail_start = self.current_order_pos().map_or(0, |pos| pos + 1);
            let span = (self.order.len() - tail_start) as u64 + 1;
            let pos = tail_start + (self.next_random() % span) as usize;
            self.order.insert(pos, id);
        }
        self.version += 1;
        Some(id)
    }

    /// Remove the entry at `index`. Removing the current entry leaves the
    /// queue without a current entry.
    pub fn remove(&mut self, index: usize) -> Option<QueueEntry> {
        if index >= self.entries.len() {
            return None;
        }
        let removed = self.entries.remove(index);
        self.order.retain(|&id| id != removed.id);
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

    /// First entry index to start playback from when no current entry is set.
    /// Updates the cached title/artist of every entry whose path matches
    /// `is_path`. Returns how many entries changed.
    pub fn update_meta(
        &mut self,
        is_path: impl Fn(&Path) -> bool,
        title: &str,
        artist: &str,
    ) -> usize {
        let mut changed = 0;
        for e in self.entries.iter_mut().filter(|e| is_path(&e.path)) {
            if e.title.as_deref() != Some(title) || e.artist.as_deref() != Some(artist) {
                e.title = Some(title.to_owned());
                e.artist = Some(artist.to_owned());
                changed += 1;
            }
        }
        if changed > 0 {
            self.version += 1;
        }
        changed
    }

    pub fn first_index(&self) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }
        if self.shuffle {
            self.order.first().and_then(|&id| self.index_of(id))
        } else {
            Some(0)
        }
    }

    /// The entry that should auto-advance next (honors shuffle and repeat,
    /// including `RepeatMode::One`).
    pub fn next(&self) -> Option<&QueueEntry> {
        self.next_index().and_then(|i| self.entries.get(i))
    }

    pub fn next_index(&self) -> Option<usize> {
        self.step(true, true)
    }

    /// Index for an explicit user "Next" command (skips `RepeatMode::One` so
    /// the user can leave a repeated track).
    pub fn next_manual_index(&self) -> Option<usize> {
        if self.current.is_none() {
            return self.first_index();
        }
        self.step(true, false)
    }

    pub fn previous_index(&self) -> Option<usize> {
        self.step(false, false)
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

    /// Restore saved entries alongside shuffle and repeat modes.
    pub fn restore<T: Into<Track>>(
        &mut self,
        tracks: Vec<T>,
        current: Option<usize>,
        shuffle: bool,
        repeat: RepeatMode,
    ) {
        self.shuffle = shuffle;
        self.repeat = repeat;
        self.replace(tracks, current);
    }

    pub fn entries(&self) -> &[QueueEntry] {
        &self.entries
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

    #[test]
    fn repeat_all_wraps_forward_and_backward() {
        let mut q = queue(&["a", "b", "c"], Some(2));
        q.set_repeat(RepeatMode::All);
        assert_eq!(q.next().unwrap().path, PathBuf::from("a"));
        assert_eq!(q.next_manual_index(), Some(0));

        q.set_current(Some(0));
        assert_eq!(q.previous_index(), Some(2));
    }

    #[test]
    fn repeat_one_loops_on_auto_next_but_skips_on_manual_next() {
        let mut q = queue(&["a", "b", "c"], Some(1));
        q.set_repeat(RepeatMode::One);
        assert_eq!(q.next().unwrap().path, PathBuf::from("b"));
        assert_eq!(q.next_index(), Some(1));
        assert_eq!(q.next_manual_index(), Some(2));
        assert_eq!(q.previous_index(), Some(0));

        // At the end without Repeat All, manual next is a noop.
        q.set_current(Some(2));
        assert_eq!(q.next_index(), Some(2));
        assert_eq!(q.next_manual_index(), None);
    }

    #[test]
    fn shuffle_starts_at_current_and_visits_all_entries_once() {
        let mut q = Queue::with_seed(42);
        q.replace(paths(&["a", "b", "c", "d", "e"]), Some(2));
        q.set_shuffle(true);

        let mut visited = vec![q.current().unwrap().path.clone()];
        while let Some(next_idx) = q.next_index() {
            q.set_current(Some(next_idx));
            visited.push(q.current().unwrap().path.clone());
        }
        assert_eq!(visited.len(), 5);
        assert_eq!(visited[0], PathBuf::from("c"));

        let mut sorted = visited.clone();
        sorted.sort();
        assert_eq!(sorted, paths(&["a", "b", "c", "d", "e"]));

        // Walking back follows the same shuffled order in reverse.
        let mut back = vec![q.current().unwrap().path.clone()];
        while let Some(prev_idx) = q.previous_index() {
            q.set_current(Some(prev_idx));
            back.push(q.current().unwrap().path.clone());
        }
        back.reverse();
        assert_eq!(back, visited);
    }

    #[test]
    fn shuffle_append_remove_move_and_toggle_off() {
        let mut q = Queue::with_seed(7);
        q.replace(paths(&["a", "b", "c"]), Some(0));
        q.set_shuffle(true);
        let first_next = q.next_index().unwrap();
        let next_path = q.get(first_next).unwrap().path.clone();

        // Removing a different entry keeps the rest of the shuffled order intact.
        let other = (1..3).find(|&i| i != first_next).unwrap();
        q.remove(other);
        assert_eq!(q.len(), 2);
        assert_eq!(q.next().unwrap().path, next_path);

        // Appending adds new items after the current entry in shuffled mode.
        q.append(paths(&["d", "e"]));
        let mut remaining = Vec::new();
        while let Some(idx) = q.next_index() {
            q.set_current(Some(idx));
            remaining.push(q.current().unwrap().path.clone());
        }
        assert_eq!(remaining.len(), 3);
        assert!(remaining.contains(&PathBuf::from("d")));
        assert!(remaining.contains(&PathBuf::from("e")));

        // Turning shuffle off resumes linear queue order from current.
        q.set_shuffle(false);
        q.set_current(Some(1));
        assert_eq!(q.next_index(), Some(2));
        assert_eq!(q.previous_index(), Some(0));
    }

    #[test]
    fn shuffle_with_repeat_all_wraps_cycle() {
        let mut q = Queue::with_seed(99);
        q.replace(paths(&["a", "b", "c"]), Some(0));
        q.set_shuffle(true);
        q.set_repeat(RepeatMode::All);

        let first = q.current_index().unwrap();
        let second = q.next_index().unwrap();
        q.set_current(Some(second));
        let third = q.next_index().unwrap();
        q.set_current(Some(third));

        assert_ne!(first, second);
        assert_ne!(second, third);
        assert_ne!(first, third);
        assert_eq!(q.next_index(), Some(first));

        q.set_current(Some(first));
        assert_eq!(q.previous_index(), Some(third));
    }

    #[test]
    fn insert_after_keeps_order_and_current_and_stops_without_anchor() {
        let mut q = queue(&["a", "b", "c"], Some(2));
        let a = q.get(0).unwrap().id;
        let x = q.insert_after(a, PathBuf::from("x")).unwrap();
        assert_eq!(names(&q), ["a", "x", "b", "c"]);
        assert_eq!(q.current_index(), Some(3), "current still points at c");
        let y = q.insert_after(x, PathBuf::from("y")).unwrap();
        assert_eq!(names(&q), ["a", "x", "y", "b", "c"]);
        assert_ne!(x, y);

        q.replace(paths(&["z"]), Some(0));
        assert_eq!(q.insert_after(y, PathBuf::from("late")), None);
        assert_eq!(names(&q), ["z"]);
    }

    #[test]
    fn insert_after_with_shuffle_queues_the_entry_to_play_later() {
        let mut q = Queue::with_seed(5);
        q.set_shuffle(true);
        q.replace(paths(&["a", "b", "c", "d"]), Some(0));
        let cur = q.current().unwrap().id;
        let x = q.insert_after(cur, PathBuf::from("x")).unwrap();
        let cur_pos = q.order.iter().position(|&id| id == cur).unwrap();
        let x_pos = q.order.iter().position(|&id| id == x).unwrap();
        assert!(x_pos > cur_pos);
        assert_eq!(q.order.len(), 5);
    }
}
