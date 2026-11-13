//! Music library: folder scanning (`walkdir`), tags and embedded covers
//! (`lofty`), SQLite storage (`rusqlite`). Covers are written to a cache
//! folder and shown through the asset protocol, never sent over IPC as
//! base64 (architecture rule 2). Song lists are paged over IPC (rule 1).
//!
//! Not implemented yet: roadmap step 4.
