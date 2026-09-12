//! Stable numeric identifiers used when publishing the library over Pro DJ Link.

use conduction_core::{SetlistId, TrackId};
use rusqlite::{params, OptionalExtension};

use crate::{Library, LibraryResult};

impl Library {
    /// Return the persistent Link ID of a registered track, or `None` if absent.
    /// IDs are allocated by the schema's insertion trigger, never by a read.
    pub fn link_track_id(&self, track_id: TrackId) -> LibraryResult<Option<u32>> {
        Ok(self
            .raw_conn()
            .query_row(
                "SELECT link_id FROM link_track_ids WHERE track_id = ?1",
                params![track_id.as_uuid().to_string()],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// Return the persistent Link ID of a registered setlist, or `None` if absent.
    /// Removing a setlist also removes its mapping; that Link ID is never reused.
    pub fn link_setlist_id(&self, setlist_id: SetlistId) -> LibraryResult<Option<u32>> {
        Ok(self
            .raw_conn()
            .query_row(
                "SELECT link_id FROM link_setlist_ids WHERE setlist_id = ?1",
                params![setlist_id.as_uuid().to_string()],
                |row| row.get(0),
            )
            .optional()?)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use conduction_core::{Key, KeyMode, Track};
    use uuid::Uuid;

    use super::*;

    fn sample_track(path: &str) -> Track {
        Track::placeholder(PathBuf::from(path), Key::new(8, KeyMode::Minor).unwrap())
    }

    #[test]
    fn link_track_id_survives_reimport_of_the_same_path() {
        let lib = Library::in_memory().unwrap();
        let original = sample_track("/music/夜明け.wav");
        lib.insert_track(&original).unwrap();
        let link_id = lib.link_track_id(original.id).unwrap().unwrap();
        assert_ne!(link_id, 0);

        let mut reimported = sample_track("/music/夜明け.wav");
        reimported.title = "夜明けのダンス".into();
        reimported.artist = "東京の音楽家".into();
        let retained_id = lib.upsert_track_by_path(&reimported).unwrap();

        assert_eq!(retained_id, original.id);
        assert_eq!(lib.link_track_id(retained_id).unwrap(), Some(link_id));
        assert_eq!(lib.link_track_id(reimported.id).unwrap(), None);
        let stored = lib.get_track(retained_id).unwrap().unwrap();
        assert_eq!(stored.title, reimported.title);
        assert_eq!(stored.artist, reimported.artist);
        assert_eq!(stored.path, reimported.path);
    }

    #[test]
    fn deleted_track_link_id_is_not_reused() {
        let lib = Library::in_memory().unwrap();
        let original = sample_track("/music/deleted.wav");
        lib.insert_track(&original).unwrap();
        let old_id = lib.link_track_id(original.id).unwrap().unwrap();
        lib.delete_track(original.id).unwrap();
        assert_eq!(lib.link_track_id(original.id).unwrap(), None);

        let replacement = sample_track("/music/deleted.wav");
        lib.insert_track(&replacement).unwrap();
        assert!(lib.link_track_id(replacement.id).unwrap().unwrap() > old_id);
    }

    #[test]
    fn setlist_link_id_survives_rename_and_is_not_reused_after_delete() {
        let lib = Library::in_memory().unwrap();
        let original = lib.create_setlist("夜のセット".into()).unwrap();
        let old_id = lib.link_setlist_id(original.id).unwrap().unwrap();
        assert_ne!(old_id, 0);

        let renamed = lib
            .rename_setlist(original.id, "朝のセット".into())
            .unwrap();
        assert_eq!(renamed.name, "朝のセット");
        assert_eq!(lib.link_setlist_id(original.id).unwrap(), Some(old_id));

        lib.delete_setlist(original.id).unwrap();
        assert_eq!(lib.link_setlist_id(original.id).unwrap(), None);
        let replacement = lib.create_setlist("朝のセット".into()).unwrap();
        assert!(lib.link_setlist_id(replacement.id).unwrap().unwrap() > old_id);
    }

    #[test]
    fn missing_records_do_not_allocate_link_ids() {
        let lib = Library::in_memory().unwrap();
        assert_eq!(lib.link_track_id(TrackId::new()).unwrap(), None);
        assert_eq!(lib.link_setlist_id(SetlistId::new()).unwrap(), None);
        assert_eq!(
            lib.raw_conn()
                .query_row("SELECT COUNT(*) FROM link_track_ids", [], |row| row
                    .get::<_, u32>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            lib.raw_conn()
                .query_row("SELECT COUNT(*) FROM link_setlist_ids", [], |row| row
                    .get::<_, u32>(0))
                .unwrap(),
            0
        );
    }

    /// Use a private temporary directory so SQLite's WAL/SHM files are cleaned up too.
    struct TestDatabase(PathBuf);

    impl TestDatabase {
        fn new() -> Self {
            let directory =
                std::env::temp_dir().join(format!("conduction-link-ids-{}", Uuid::new_v4()));
            std::fs::create_dir(&directory).unwrap();
            Self(directory)
        }

        fn path(&self) -> PathBuf {
            self.0.join("library.db")
        }
    }

    impl Drop for TestDatabase {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn link_ids_survive_database_reopen() {
        let database = TestDatabase::new();
        let mut track = sample_track("/music/再生.wav");
        track.title = "再生の曲".into();
        track.artist = "演奏者".into();
        let (track_link_id, setlist_id, setlist_link_id) = {
            let lib = Library::open(database.path()).unwrap();
            lib.insert_track(&track).unwrap();
            let setlist = lib.create_setlist("夕方の選曲".into()).unwrap();
            (
                lib.link_track_id(track.id).unwrap().unwrap(),
                setlist.id,
                lib.link_setlist_id(setlist.id).unwrap().unwrap(),
            )
        };

        let reopened = Library::open(database.path()).unwrap();
        assert_eq!(
            reopened.link_track_id(track.id).unwrap(),
            Some(track_link_id)
        );
        assert_eq!(
            reopened.link_setlist_id(setlist_id).unwrap(),
            Some(setlist_link_id)
        );
        assert_eq!(
            reopened.get_track(track.id).unwrap().unwrap().title,
            "再生の曲"
        );
        assert_eq!(
            reopened.get_setlist(setlist_id).unwrap().unwrap().name,
            "夕方の選曲"
        );
    }

    #[test]
    fn deleted_ids_remain_reserved_after_database_reopen() {
        let database = TestDatabase::new();
        let track = sample_track("/music/replacement.wav");
        let (old_track_id, old_setlist_id) = {
            let lib = Library::open(database.path()).unwrap();
            lib.insert_track(&track).unwrap();
            let setlist = lib.create_setlist("Deleted".into()).unwrap();
            let old_track_id = lib.link_track_id(track.id).unwrap().unwrap();
            let old_setlist_id = lib.link_setlist_id(setlist.id).unwrap().unwrap();
            lib.delete_track(track.id).unwrap();
            lib.delete_setlist(setlist.id).unwrap();
            (old_track_id, old_setlist_id)
        };
        let reopened = Library::open(database.path()).unwrap();
        reopened.insert_track(&track).unwrap();
        let setlist = reopened.create_setlist("Replacement".into()).unwrap();
        assert!(reopened.link_track_id(track.id).unwrap().unwrap() > old_track_id);
        assert!(reopened.link_setlist_id(setlist.id).unwrap().unwrap() > old_setlist_id);
    }
}
