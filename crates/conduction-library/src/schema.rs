use rusqlite::Connection;

use crate::error::{LibraryError, LibraryResult};

/// 現在のスキーマバージョン。マイグレーションを追加する際にインクリメント。
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// スキーマメタテーブル + 全テーブルを作成する（バージョン判定付き）。
///
/// 既存DBがあれば、バージョン比較のみ行い、差分があればエラーにする。
/// マイグレーションは将来対応。
pub fn initialize(conn: &Connection) -> LibraryResult<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS schema_meta (
           id INTEGER PRIMARY KEY CHECK (id = 1),
           version INTEGER NOT NULL
         );",
    )?;

    let current: Option<u32> = conn
        .query_row(
            "SELECT version FROM schema_meta WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .ok();

    match current {
        Some(v) if v == CURRENT_SCHEMA_VERSION => Ok(()),
        Some(other) => Err(LibraryError::Schema(format!(
            "unexpected schema version {other} (expected {CURRENT_SCHEMA_VERSION})"
        ))),
        None => {
            create_v1_tables(conn)?;
            conn.execute(
                "INSERT INTO schema_meta (id, version) VALUES (1, ?1)",
                [CURRENT_SCHEMA_VERSION],
            )?;
            Ok(())
        }
    }
}

fn create_v1_tables(conn: &Connection) -> LibraryResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE tracks (
          id TEXT PRIMARY KEY,
          path TEXT NOT NULL UNIQUE,
          title TEXT NOT NULL DEFAULT '',
          artist TEXT NOT NULL DEFAULT '',
          album TEXT NOT NULL DEFAULT '',
          genre TEXT NOT NULL DEFAULT '',
          duration_sec REAL NOT NULL DEFAULT 0,
          bpm REAL NOT NULL DEFAULT 0,
          key_camelot_number INTEGER NOT NULL DEFAULT 1,
          key_mode INTEGER NOT NULL DEFAULT 0,
          energy REAL NOT NULL DEFAULT 0,
          beatgrid_verified INTEGER NOT NULL DEFAULT 0,
          analyzed_at TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE INDEX idx_tracks_bpm ON tracks(bpm);
        CREATE INDEX idx_tracks_key ON tracks(key_camelot_number, key_mode);
        CREATE INDEX idx_tracks_title ON tracks(title);
        CREATE INDEX idx_tracks_artist ON tracks(artist);

        CREATE TABLE cues (
          id TEXT PRIMARY KEY,
          track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
          position_beats REAL NOT NULL,
          cue_type TEXT NOT NULL,
          section_start REAL,
          section_end REAL,
          bpm_at_cue REAL NOT NULL,
          key_camelot_number INTEGER NOT NULL,
          key_mode INTEGER NOT NULL,
          energy_level REAL NOT NULL,
          phrase_length INTEGER NOT NULL,
          mixable_as TEXT NOT NULL DEFAULT '',
          compatible_energy_start REAL NOT NULL,
          compatible_energy_end REAL NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE INDEX idx_cues_track ON cues(track_id);
        CREATE INDEX idx_cues_type ON cues(cue_type);

        CREATE TABLE beats (
          track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
          position_sec REAL NOT NULL,
          instantaneous_bpm REAL,
          is_downbeat INTEGER NOT NULL DEFAULT 0,
          PRIMARY KEY (track_id, position_sec)
        );
        "#,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_fresh_db() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();

        let v: u32 = conn
            .query_row(
                "SELECT version FROM schema_meta WHERE id = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(v, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn initialize_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        initialize(&conn).unwrap();
    }

    #[test]
    fn rejects_unexpected_version() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_meta (id INTEGER PRIMARY KEY, version INTEGER);
             INSERT INTO schema_meta (id, version) VALUES (1, 99);",
        )
        .unwrap();
        let err = initialize(&conn).unwrap_err();
        matches!(err, LibraryError::Schema(_));
    }
}
