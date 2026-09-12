use rusqlite::{Connection, OptionalExtension};

use crate::error::{LibraryError, LibraryResult};

/// 現在のスキーマバージョン。マイグレーションを追加する際にインクリメント。
pub const CURRENT_SCHEMA_VERSION: u32 = 6;

/// スキーマメタテーブル + 全テーブルを作成する（バージョン判定 + マイグレーション）。
pub fn initialize(conn: &Connection) -> LibraryResult<()> {
    // Connection-level pragmas cannot be changed inside the migration transaction.
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_meta (
           id INTEGER PRIMARY KEY CHECK (id = 1),
           version INTEGER NOT NULL
         );",
    )?;

    let current: Option<u32> = tx
        .query_row("SELECT version FROM schema_meta WHERE id = 1", [], |row| {
            row.get(0)
        })
        .optional()?;

    let mut version = match current {
        Some(v) if (1..=CURRENT_SCHEMA_VERSION).contains(&v) => v,
        Some(other) => {
            return Err(LibraryError::Schema(format!(
                "unexpected schema version {other} (expected {CURRENT_SCHEMA_VERSION})"
            )));
        }
        None => {
            create_v1_tables(&tx)?;
            1
        }
    };

    // The schema, backfilled Link IDs, and version advance atomically. An error
    // rolls the entire chain back, making retry safe even for a legacy database.
    while version < CURRENT_SCHEMA_VERSION {
        match version {
            1 => migrate_v1_to_v2(&tx)?,
            2 => migrate_v2_to_v3(&tx)?,
            3 => migrate_v3_to_v4(&tx)?,
            4 => migrate_v4_to_v5(&tx)?,
            5 => migrate_v5_to_v6(&tx)?,
            _ => unreachable!("schema version checked above"),
        }
        version += 1;
    }
    set_version(&tx, CURRENT_SCHEMA_VERSION)?;
    tx.commit()?;
    Ok(())
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

/// v6: Permanent, nonzero 32-bit identifiers used by Pro DJ Link clients.
///
/// Separate sequences reflect the protocol's separate track and playlist ID
/// namespaces. AUTOINCREMENT keeps a deleted record's ID from being reassigned;
/// the CHECK also makes exhaustion fail atomically rather than wrap to zero.
fn migrate_v5_to_v6(conn: &Connection) -> LibraryResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE link_track_ids (
          link_id INTEGER PRIMARY KEY AUTOINCREMENT
            CHECK (link_id BETWEEN 1 AND 4294967295),
          track_id TEXT NOT NULL UNIQUE REFERENCES tracks(id) ON DELETE CASCADE
        );
        CREATE TABLE link_setlist_ids (
          link_id INTEGER PRIMARY KEY AUTOINCREMENT
            CHECK (link_id BETWEEN 1 AND 4294967295),
          setlist_id TEXT NOT NULL UNIQUE REFERENCES setlists(id) ON DELETE CASCADE
        );

        INSERT INTO link_track_ids (track_id)
          SELECT id FROM tracks ORDER BY created_at, id;
        INSERT INTO link_setlist_ids (setlist_id)
          SELECT id FROM setlists ORDER BY created_at, id;

        CREATE TRIGGER tracks_assign_link_id AFTER INSERT ON tracks
        BEGIN
          INSERT INTO link_track_ids (track_id) VALUES (NEW.id);
        END;
        CREATE TRIGGER setlists_assign_link_id AFTER INSERT ON setlists
        BEGIN
          INSERT INTO link_setlist_ids (setlist_id) VALUES (NEW.id);
        END;
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
            .query_row("SELECT version FROM schema_meta WHERE id = 1", [], |r| {
                r.get(0)
            })
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
        assert!(matches!(err, LibraryError::Schema(_)));
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

    fn legacy_database(version: u32) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE schema_meta (id INTEGER PRIMARY KEY, version INTEGER);",
        )
        .unwrap();
        create_v1_tables(&conn).unwrap();
        if version >= 2 {
            migrate_v1_to_v2(&conn).unwrap();
        }
        if version >= 3 {
            migrate_v2_to_v3(&conn).unwrap();
        }
        if version >= 4 {
            migrate_v3_to_v4(&conn).unwrap();
        }
        if version >= 5 {
            migrate_v4_to_v5(&conn).unwrap();
        }
        set_version(&conn, version).unwrap();
        conn
    }

    fn insert_legacy_track(conn: &Connection, id: &str, path: &str) {
        conn.execute(
            "INSERT INTO tracks (id, path, title, artist, created_at, updated_at)
             VALUES (?1, ?2, '夜明けの音', '東京のDJ', '2025-01-01', '2025-01-02')",
            [id, path],
        )
        .unwrap();
    }

    #[test]
    fn migrates_each_legacy_version_and_preserves_existing_records() {
        for version in 1..CURRENT_SCHEMA_VERSION {
            let conn = legacy_database(version);
            insert_legacy_track(&conn, "track-before-migration", "/音楽/夜明け.wav");
            if version >= 4 {
                conn.execute_batch(
                    "INSERT INTO setlists (id, name, created_at, updated_at)
                     VALUES ('set-before-migration', '深夜のセット', '2025-01-01', '2025-01-02');",
                )
                .unwrap();
            }
            initialize(&conn).unwrap();
            let track: (String, String, String, String, u32) = conn
                .query_row(
                    "SELECT t.id, t.path, t.title, t.artist, m.link_id FROM tracks t
                     JOIN link_track_ids m ON m.track_id = t.id",
                    [],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .unwrap();
            assert_eq!(track.0, "track-before-migration");
            assert_eq!(track.1, "/音楽/夜明け.wav");
            assert_eq!(track.2, "夜明けの音");
            assert_eq!(track.3, "東京のDJ");
            assert!(track.4 > 0);
            if version >= 4 {
                let setlist: (String, String, u32) = conn
                    .query_row(
                        "SELECT s.id, s.name, m.link_id FROM setlists s
                         JOIN link_setlist_ids m ON m.setlist_id = s.id",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .unwrap();
                assert_eq!(setlist.0, "set-before-migration");
                assert_eq!(setlist.1, "深夜のセット");
                assert!(setlist.2 > 0);
            }

            initialize(&conn).unwrap();
            let after_reopen: u32 = conn
                .query_row("SELECT link_id FROM link_track_ids", [], |row| row.get(0))
                .unwrap();
            assert_eq!(after_reopen, track.4);
            insert_legacy_track(&conn, "track-after-migration", "/音楽/新曲.wav");
            let next: u32 = conn
                .query_row(
                    "SELECT link_id FROM link_track_ids WHERE track_id = 'track-after-migration'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(next > track.4);
        }
    }

    #[test]
    fn failed_migration_rolls_back_all_schema_changes_and_version() {
        let conn = legacy_database(1);
        insert_legacy_track(&conn, "retained-track", "/original.wav");
        // Deliberate conflict late in the migration chain.
        conn.execute_batch("CREATE TABLE link_setlist_ids (sentinel TEXT);")
            .unwrap();
        assert!(initialize(&conn).is_err());
        let version: u32 = conn
            .query_row("SELECT version FROM schema_meta", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
        for table in [
            "waveforms",
            "hot_cues",
            "setlists",
            "user_templates",
            "link_track_ids",
        ] {
            let exists: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(!exists, "partial migration leaked {table}");
        }
        let title: String = conn
            .query_row(
                "SELECT title FROM tracks WHERE id = 'retained-track'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(title, "夜明けの音");
        conn.execute_batch("DROP TABLE link_setlist_ids;").unwrap();
        initialize(&conn).unwrap();
    }

    #[test]
    fn link_id_range_and_exhaustion_are_enforced_atomically() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        insert_legacy_track(&conn, "existing", "/existing.wav");
        conn.execute("DELETE FROM link_track_ids WHERE track_id = 'existing'", [])
            .unwrap();
        for invalid in [0_i64, -1, i64::from(u32::MAX) + 1] {
            assert!(conn
                .execute(
                    "INSERT INTO link_track_ids (link_id, track_id) VALUES (?1, 'existing')",
                    [invalid],
                )
                .is_err());
        }
        conn.execute(
            "INSERT INTO link_track_ids (link_id, track_id) VALUES (?1, 'existing')",
            [u32::MAX],
        )
        .unwrap();
        let result = conn.execute(
            "INSERT INTO tracks (id, path, created_at, updated_at)
             VALUES ('overflow', '/overflow.wav', '', '')",
            [],
        );
        assert!(result.is_err());
        let rows: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM tracks WHERE id = 'overflow'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            rows, 0,
            "failed ID allocation must also roll back the track insert"
        );

        conn.execute_batch(
            "INSERT INTO setlists (id, name, created_at, updated_at) VALUES ('set', 'set', '', '');
             UPDATE sqlite_sequence SET seq = 4294967295 WHERE name = 'link_setlist_ids';",
        )
        .unwrap();
        assert!(conn.execute(
            "INSERT INTO setlists (id, name, created_at, updated_at) VALUES ('overflow', 'overflow', '', '')",
            [],
        ).is_err());
        let rows: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM setlists WHERE id = 'overflow'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rows, 0);
    }
}
