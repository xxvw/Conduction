use rusqlite::Connection;

use crate::error::{LibraryError, LibraryResult};

/// 現在のスキーマバージョン。マイグレーションを追加する際にインクリメント。
pub const CURRENT_SCHEMA_VERSION: u32 = 5;

/// スキーマメタテーブル + 全テーブルを作成する（バージョン判定 + マイグレーション）。
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
        Some(1) => {
            migrate_v1_to_v2(conn)?;
            migrate_v2_to_v3(conn)?;
            migrate_v3_to_v4(conn)?;
            migrate_v4_to_v5(conn)?;
            set_version(conn, CURRENT_SCHEMA_VERSION)?;
            Ok(())
        }
        Some(2) => {
            migrate_v2_to_v3(conn)?;
            migrate_v3_to_v4(conn)?;
            migrate_v4_to_v5(conn)?;
            set_version(conn, CURRENT_SCHEMA_VERSION)?;
            Ok(())
        }
        Some(3) => {
            migrate_v3_to_v4(conn)?;
            migrate_v4_to_v5(conn)?;
            set_version(conn, CURRENT_SCHEMA_VERSION)?;
            Ok(())
        }
        Some(4) => {
            migrate_v4_to_v5(conn)?;
            set_version(conn, CURRENT_SCHEMA_VERSION)?;
            Ok(())
        }
        Some(other) => Err(LibraryError::Schema(format!(
            "unexpected schema version {other} (expected {CURRENT_SCHEMA_VERSION})"
        ))),
        None => {
            create_v1_tables(conn)?;
            migrate_v1_to_v2(conn)?;
            migrate_v2_to_v3(conn)?;
            migrate_v3_to_v4(conn)?;
            migrate_v4_to_v5(conn)?;
            set_version(conn, CURRENT_SCHEMA_VERSION)?;
            Ok(())
        }
    }
}

fn set_version(conn: &Connection, version: u32) -> LibraryResult<()> {
    conn.execute(
        "INSERT INTO schema_meta (id, version) VALUES (1, ?1)
         ON CONFLICT(id) DO UPDATE SET version = excluded.version",
        [version],
    )?;
    Ok(())
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

/// v2: 波形プレビュー (3 バンド RMS) を保持する `waveforms` テーブルを追加。
fn migrate_v1_to_v2(conn: &Connection) -> LibraryResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS waveforms (
          track_id TEXT PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
          sample_count INTEGER NOT NULL,
          low_blob BLOB NOT NULL,
          mid_blob BLOB NOT NULL,
          high_blob BLOB NOT NULL,
          generated_at TEXT NOT NULL
        );
        "#,
    )?;
    Ok(())
}

/// v3: Hot Cue（8 スロット、track_id × slot で一意）を保持する `hot_cues` テーブルを追加。
fn migrate_v2_to_v3(conn: &Connection) -> LibraryResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS hot_cues (
          track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
          slot INTEGER NOT NULL CHECK (slot BETWEEN 1 AND 8),
          position_sec REAL NOT NULL,
          created_at TEXT NOT NULL,
          PRIMARY KEY (track_id, slot)
        );
        CREATE INDEX IF NOT EXISTS idx_hot_cues_track ON hot_cues(track_id);
        "#,
    )?;
    Ok(())
}

/// v5: ユーザー作成のテンプレートを保持する `user_templates` テーブルを追加 (要件 §6.7)。
/// payload はテンプレート全体を serde で JSON 化したもの。
fn migrate_v4_to_v5(conn: &Connection) -> LibraryResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS user_templates (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          payload TEXT NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_user_templates_name ON user_templates(name);
        "#,
    )?;
    Ok(())
}

/// v4: Setlist と Setlist エントリを保持するテーブルを追加 (要件 §6.11)。
/// 遷移仕様は entry に inline (NULL カラム = no transition)。
fn migrate_v3_to_v4(conn: &Connection) -> LibraryResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS setlists (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS setlist_entries (
          id TEXT PRIMARY KEY,
          setlist_id TEXT NOT NULL REFERENCES setlists(id) ON DELETE CASCADE,
          position INTEGER NOT NULL,
          track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
          play_from_cue TEXT,
          play_until_cue TEXT,
          transition_template_id TEXT,
          transition_tempo_mode TEXT,
          transition_entry_cue TEXT,
          transition_exit_cue TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_setlist_entries_setlist
          ON setlist_entries(setlist_id, position);
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

    /// v1 のレガシー DB を最新版にマイグレーションできること。
    #[test]
    fn migrates_from_v1_to_latest() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_meta (id INTEGER PRIMARY KEY, version INTEGER);
             INSERT INTO schema_meta (id, version) VALUES (1, 1);",
        )
        .unwrap();
        // v1 のテーブルを最低限作っておく（外部キー制約のため tracks のみ）
        conn.execute_batch(
            "CREATE TABLE tracks (
               id TEXT PRIMARY KEY, path TEXT NOT NULL UNIQUE,
               title TEXT, artist TEXT, album TEXT, genre TEXT,
               duration_sec REAL, bpm REAL, key_camelot_number INTEGER,
               key_mode INTEGER, energy REAL, beatgrid_verified INTEGER,
               analyzed_at TEXT, created_at TEXT, updated_at TEXT
             );",
        )
        .unwrap();

        initialize(&conn).unwrap();

        let v: u32 = conn
            .query_row("SELECT version FROM schema_meta WHERE id = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(v, CURRENT_SCHEMA_VERSION);

        // 最新で導入された両テーブルが存在することを SELECT で検証。
        let wf_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM waveforms", [], |r| r.get(0))
            .unwrap();
        assert_eq!(wf_count, 0);
        let hc_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM hot_cues", [], |r| r.get(0))
            .unwrap();
        assert_eq!(hc_count, 0);
        let sl_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM setlists", [], |r| r.get(0))
            .unwrap();
        assert_eq!(sl_count, 0);
        let se_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM setlist_entries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(se_count, 0);
        let ut_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM user_templates", [], |r| r.get(0))
            .unwrap();
        assert_eq!(ut_count, 0);
    }
}
