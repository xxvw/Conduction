use std::path::{Path, PathBuf};

use chrono::Utc;
use conduction_analysis::WaveformPreview;
use conduction_core::{Beat, Cue, CueId, Track, TrackId};
use directories::ProjectDirs;
use rusqlite::{params, Connection};
use tracing::{debug, info};

use crate::error::{LibraryError, LibraryResult};
use crate::mapping::{
    cue_from_row, cue_type_to_str, dt_to_str, key_mode_to_i64, mix_roles_to_csv, track_from_row,
};
use crate::schema;

/// SQLite 上のライブラリ。楽曲・Cue・ビートグリッドの永続化を担う。
///
/// API は同期 I/O。オーディオスレッドから直接呼ばず、UI スレッドまたは
/// 専用のファイル I/O スレッドから呼ぶこと。
pub struct Library {
    conn: Connection,
}

impl Library {
    /// 指定パスの SQLite ファイルを開く（なければ作成）。
    pub fn open(path: impl AsRef<Path>) -> LibraryResult<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open(path)?;
        schema::initialize(&conn)?;
        info!(db = %path.display(), "library opened");
        Ok(Self { conn })
    }

    /// OS 規約のユーザーデータディレクトリ配下に `library.db` を置く。
    pub fn open_default() -> LibraryResult<Self> {
        let dirs = ProjectDirs::from("com", "xxvw", "Conduction").ok_or_else(|| {
            LibraryError::Schema("no user data directory available from OS".into())
        })?;
        let mut path = PathBuf::from(dirs.data_dir());
        std::fs::create_dir_all(&path)?;
        path.push("library.db");
        Self::open(&path)
    }

    /// メモリ上のみに存在する一時 DB。テストや preview 用。
    pub fn in_memory() -> LibraryResult<Self> {
        let conn = Connection::open_in_memory()?;
        schema::initialize(&conn)?;
        Ok(Self { conn })
    }

    // -------- Track --------

    /// Track を新規挿入する。既に同一 path が存在する場合はエラー。
    pub fn insert_track(&self, track: &Track) -> LibraryResult<()> {
        let now = dt_to_str(Utc::now());
        let analyzed_at = track.analyzed_at.map(dt_to_str);
        self.conn.execute(
            "INSERT INTO tracks (
               id, path, title, artist, album, genre, duration_sec,
               bpm, key_camelot_number, key_mode, energy,
               beatgrid_verified, analyzed_at, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                track.id.as_uuid().to_string(),
                track.path.to_string_lossy().to_string(),
                track.title,
                track.artist,
                track.album,
                track.genre,
                track.duration.as_secs_f64(),
                track.bpm as f64,
                track.key.camelot_number as i64,
                key_mode_to_i64(track.key.mode),
                track.energy as f64,
                track.beatgrid_verified as i64,
                analyzed_at,
                now,
                now,
            ],
        )?;
        debug!(track_id = %track.id, "track inserted");
        Ok(())
    }

    /// 既存 Track を path で UPSERT（置換）する。戻り値は最終的な ID。
    ///
    /// 楽曲再インポートで ID が変わらないよう、既存レコードがあればその ID を維持する。
    pub fn upsert_track_by_path(&self, track: &Track) -> LibraryResult<TrackId> {
        if let Some(existing) = self.get_track_by_path(&track.path)? {
            let mut updated = track.clone();
            updated.id = existing.id;
            self.update_track(&updated)?;
            Ok(existing.id)
        } else {
            self.insert_track(track)?;
            Ok(track.id)
        }
    }

    pub fn update_track(&self, track: &Track) -> LibraryResult<()> {
        let now = dt_to_str(Utc::now());
        let analyzed_at = track.analyzed_at.map(dt_to_str);
        let affected = self.conn.execute(
            "UPDATE tracks SET
               path = ?2, title = ?3, artist = ?4, album = ?5, genre = ?6,
               duration_sec = ?7, bpm = ?8, key_camelot_number = ?9, key_mode = ?10,
               energy = ?11, beatgrid_verified = ?12, analyzed_at = ?13, updated_at = ?14
             WHERE id = ?1",
            params![
                track.id.as_uuid().to_string(),
                track.path.to_string_lossy().to_string(),
                track.title,
                track.artist,
                track.album,
                track.genre,
                track.duration.as_secs_f64(),
                track.bpm as f64,
                track.key.camelot_number as i64,
                key_mode_to_i64(track.key.mode),
                track.energy as f64,
                track.beatgrid_verified as i64,
                analyzed_at,
                now,
            ],
        )?;
        if affected == 0 {
            return Err(LibraryError::TrackNotFound(track.path.clone()));
        }
        Ok(())
    }

    pub fn get_track(&self, id: TrackId) -> LibraryResult<Option<Track>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM tracks WHERE id = ?1 LIMIT 1")?;
        let mut rows = stmt.query(params![id.as_uuid().to_string()])?;
        match rows.next()? {
            Some(row) => Ok(Some(track_from_row(row)?)),
            None => Ok(None),
        }
    }

    pub fn get_track_by_path(&self, path: &Path) -> LibraryResult<Option<Track>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM tracks WHERE path = ?1 LIMIT 1")?;
        let mut rows = stmt.query(params![path.to_string_lossy().to_string()])?;
        match rows.next()? {
            Some(row) => Ok(Some(track_from_row(row)?)),
            None => Ok(None),
        }
    }

    /// 全トラック一覧（作成順）。フィルタ機能は将来の TrackQuery で拡張する。
    pub fn list_tracks(&self) -> LibraryResult<Vec<Track>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM tracks ORDER BY updated_at DESC, created_at DESC",
        )?;
        let tracks = stmt
            .query_map([], |row| Ok(track_from_row(row)))?
            .collect::<Result<Result<Vec<_>, _>, _>>()??;
        Ok(tracks)
    }

    pub fn delete_track(&self, id: TrackId) -> LibraryResult<()> {
        self.conn
            .execute("DELETE FROM tracks WHERE id = ?1", params![id.as_uuid().to_string()])?;
        Ok(())
    }

    // -------- Cue --------

    pub fn insert_cue(&self, cue: &Cue) -> LibraryResult<()> {
        let now = dt_to_str(Utc::now());
        let (section_start, section_end) = match &cue.section {
            Some(r) => (Some(r.start), Some(r.end)),
            None => (None, None),
        };
        self.conn.execute(
            "INSERT INTO cues (
               id, track_id, position_beats, cue_type,
               section_start, section_end,
               bpm_at_cue, key_camelot_number, key_mode, energy_level, phrase_length,
               mixable_as, compatible_energy_start, compatible_energy_end,
               created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                cue.id.as_uuid().to_string(),
                cue.track_id.as_uuid().to_string(),
                cue.position_beats,
                cue_type_to_str(cue.cue_type),
                section_start,
                section_end,
                cue.bpm_at_cue as f64,
                cue.key_at_cue.camelot_number as i64,
                key_mode_to_i64(cue.key_at_cue.mode),
                cue.energy_level as f64,
                cue.phrase_length as i64,
                mix_roles_to_csv(&cue.mixable_as),
                cue.compatible_energy.start as f64,
                cue.compatible_energy.end as f64,
                now,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn delete_cue(&self, id: CueId) -> LibraryResult<()> {
        self.conn
            .execute("DELETE FROM cues WHERE id = ?1", params![id.as_uuid().to_string()])?;
        Ok(())
    }

    pub fn list_cues_for_track(&self, track_id: TrackId) -> LibraryResult<Vec<Cue>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM cues WHERE track_id = ?1 ORDER BY position_beats ASC",
        )?;
        let cues = stmt
            .query_map(params![track_id.as_uuid().to_string()], |row| {
                Ok(cue_from_row(row))
            })?
            .collect::<Result<Result<Vec<_>, _>, _>>()??;
        Ok(cues)
    }

    // -------- Beatgrid --------

    /// BPM とビートグリッドを同時に保存し、`tracks.bpm` / `analyzed_at` を更新する。
    pub fn save_track_analysis(
        &mut self,
        track_id: TrackId,
        bpm: f32,
        beats: &[Beat],
    ) -> LibraryResult<()> {
        let now = dt_to_str(Utc::now());
        let affected = self.conn.execute(
            "UPDATE tracks SET bpm = ?2, analyzed_at = ?3, updated_at = ?3 WHERE id = ?1",
            params![track_id.as_uuid().to_string(), bpm as f64, now],
        )?;
        if affected == 0 {
            return Err(LibraryError::Unsupported(format!(
                "track not found for analysis: {track_id}"
            )));
        }
        self.replace_beatgrid(track_id, beats)?;
        Ok(())
    }

    /// Krumhansl-Schmuckler などのキー検出結果を保存する。BPM 解析とは独立。
    pub fn save_track_key(
        &mut self,
        track_id: TrackId,
        key: conduction_core::Key,
    ) -> LibraryResult<()> {
        let now = dt_to_str(Utc::now());
        let affected = self.conn.execute(
            "UPDATE tracks SET key_camelot_number = ?2, key_mode = ?3, updated_at = ?4
             WHERE id = ?1",
            params![
                track_id.as_uuid().to_string(),
                key.camelot_number as i64,
                key_mode_to_i64(key.mode),
                now,
            ],
        )?;
        if affected == 0 {
            return Err(LibraryError::Unsupported(format!(
                "track not found for key save: {track_id}"
            )));
        }
        Ok(())
    }

    /// 既存のビートグリッドを差し替える（一括 upsert）。
    pub fn replace_beatgrid(
        &mut self,
        track_id: TrackId,
        beats: &[Beat],
    ) -> LibraryResult<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM beats WHERE track_id = ?1",
            params![track_id.as_uuid().to_string()],
        )?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO beats (track_id, position_sec, instantaneous_bpm, is_downbeat)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for beat in beats {
                stmt.execute(params![
                    track_id.as_uuid().to_string(),
                    beat.position_sec,
                    beat.instantaneous_bpm.map(|b| b as f64),
                    beat.is_downbeat as i64,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn load_beatgrid(&self, track_id: TrackId) -> LibraryResult<Vec<Beat>> {
        let mut stmt = self.conn.prepare(
            "SELECT position_sec, instantaneous_bpm, is_downbeat
             FROM beats WHERE track_id = ?1 ORDER BY position_sec ASC",
        )?;
        let beats = stmt
            .query_map(params![track_id.as_uuid().to_string()], |row| {
                let instantaneous_bpm: Option<f64> = row.get(1)?;
                Ok(Beat {
                    position_sec: row.get(0)?,
                    instantaneous_bpm: instantaneous_bpm.map(|b| b as f32),
                    is_downbeat: row.get::<_, i64>(2)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(beats)
    }

    // -------- Hot Cues --------

    /// Hot Cue (slot 1..=8) を upsert で保存。
    pub fn set_hot_cue(
        &self,
        track_id: TrackId,
        slot: u8,
        position_sec: f64,
    ) -> LibraryResult<()> {
        if !(1..=8).contains(&slot) {
            return Err(LibraryError::Unsupported(format!(
                "hot cue slot {slot} out of range 1..=8"
            )));
        }
        let now = dt_to_str(Utc::now());
        self.conn.execute(
            "INSERT INTO hot_cues (track_id, slot, position_sec, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(track_id, slot) DO UPDATE SET
                position_sec = excluded.position_sec,
                created_at = excluded.created_at",
            params![
                track_id.as_uuid().to_string(),
                slot as i64,
                position_sec,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn delete_hot_cue(&self, track_id: TrackId, slot: u8) -> LibraryResult<()> {
        self.conn.execute(
            "DELETE FROM hot_cues WHERE track_id = ?1 AND slot = ?2",
            params![track_id.as_uuid().to_string(), slot as i64],
        )?;
        Ok(())
    }

    pub fn list_hot_cues(&self, track_id: TrackId) -> LibraryResult<Vec<(u8, f64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT slot, position_sec FROM hot_cues WHERE track_id = ?1 ORDER BY slot ASC",
        )?;
        let rows = stmt
            .query_map(params![track_id.as_uuid().to_string()], |row| {
                let slot: i64 = row.get(0)?;
                let pos: f64 = row.get(1)?;
                Ok((slot as u8, pos))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    // -------- Waveform preview --------

    pub fn save_waveform(
        &self,
        track_id: TrackId,
        wf: &WaveformPreview,
    ) -> LibraryResult<()> {
        if wf.low.len() != wf.sample_count as usize
            || wf.mid.len() != wf.sample_count as usize
            || wf.high.len() != wf.sample_count as usize
        {
            return Err(LibraryError::Unsupported(format!(
                "waveform dimensions inconsistent: declared {}, got low={} mid={} high={}",
                wf.sample_count,
                wf.low.len(),
                wf.mid.len(),
                wf.high.len()
            )));
        }

        let now = dt_to_str(Utc::now());
        self.conn.execute(
            "INSERT INTO waveforms
                (track_id, sample_count, low_blob, mid_blob, high_blob, generated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(track_id) DO UPDATE SET
                sample_count = excluded.sample_count,
                low_blob = excluded.low_blob,
                mid_blob = excluded.mid_blob,
                high_blob = excluded.high_blob,
                generated_at = excluded.generated_at",
            params![
                track_id.as_uuid().to_string(),
                wf.sample_count as i64,
                f32_slice_to_bytes(&wf.low),
                f32_slice_to_bytes(&wf.mid),
                f32_slice_to_bytes(&wf.high),
                now,
            ],
        )?;
        Ok(())
    }

    pub fn load_waveform(&self, track_id: TrackId) -> LibraryResult<Option<WaveformPreview>> {
        let mut stmt = self.conn.prepare(
            "SELECT sample_count, low_blob, mid_blob, high_blob
             FROM waveforms WHERE track_id = ?1 LIMIT 1",
        )?;
        let mut rows = stmt.query(params![track_id.as_uuid().to_string()])?;
        match rows.next()? {
            Some(row) => {
                let sample_count: i64 = row.get(0)?;
                let low: Vec<u8> = row.get(1)?;
                let mid: Vec<u8> = row.get(2)?;
                let high: Vec<u8> = row.get(3)?;
                Ok(Some(WaveformPreview {
                    sample_count: sample_count as u32,
                    low: bytes_to_f32_vec(&low)?,
                    mid: bytes_to_f32_vec(&mid)?,
                    high: bytes_to_f32_vec(&high)?,
                }))
            }
            None => Ok(None),
        }
    }
}

fn f32_slice_to_bytes(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for &f in v {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

fn bytes_to_f32_vec(b: &[u8]) -> LibraryResult<Vec<f32>> {
    if b.len() % 4 != 0 {
        return Err(LibraryError::Unsupported(format!(
            "blob length {} not a multiple of 4 (expected f32 LE bytes)",
            b.len()
        )));
    }
    let mut out = Vec::with_capacity(b.len() / 4);
    for chunk in b.chunks_exact(4) {
        out.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use conduction_core::{CueType, Key, KeyMode, MixRole};

    use super::*;

    fn sample_key() -> Key {
        Key::new(8, KeyMode::Minor).unwrap()
    }

    fn sample_track() -> Track {
        let mut t = Track::placeholder(PathBuf::from("/tmp/song.mp3"), sample_key());
        t.title = "Midnight Drive".into();
        t.artist = "Kaoru".into();
        t.album = "Neon".into();
        t.genre = "Deep".into();
        t.duration = std::time::Duration::from_secs_f64(250.0);
        t.bpm = 128.0;
        t.energy = 0.7;
        t
    }

    #[test]
    fn open_in_memory_initializes_schema() {
        let lib = Library::in_memory().unwrap();
        let tracks = lib.list_tracks().unwrap();
        assert!(tracks.is_empty());
    }

    #[test]
    fn insert_and_get_track() {
        let lib = Library::in_memory().unwrap();
        let track = sample_track();
        lib.insert_track(&track).unwrap();

        let got = lib.get_track(track.id).unwrap().unwrap();
        assert_eq!(got.title, "Midnight Drive");
        assert_eq!(got.bpm, 128.0);
        assert_eq!(got.key.to_camelot(), "8A");
    }

    #[test]
    fn get_track_by_path() {
        let lib = Library::in_memory().unwrap();
        let track = sample_track();
        lib.insert_track(&track).unwrap();

        let got = lib.get_track_by_path(&track.path).unwrap().unwrap();
        assert_eq!(got.id, track.id);

        let missing = lib.get_track_by_path(Path::new("/nope")).unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn upsert_preserves_existing_id() {
        let lib = Library::in_memory().unwrap();
        let mut a = sample_track();
        lib.insert_track(&a).unwrap();

        // 同じ path で別 ID / 別データで upsert。既存 ID が維持される。
        let mut b = Track::placeholder(a.path.clone(), sample_key());
        b.title = "Neon Reverie".into();
        let id = lib.upsert_track_by_path(&b).unwrap();
        assert_eq!(id, a.id);
        let got = lib.get_track(a.id).unwrap().unwrap();
        assert_eq!(got.title, "Neon Reverie");

        // ID ベース update で他フィールドも更新できる。
        a.title = "Updated".into();
        lib.update_track(&a).unwrap();
        let got = lib.get_track(a.id).unwrap().unwrap();
        assert_eq!(got.title, "Updated");
    }

    #[test]
    fn delete_track_cascades_cues_and_beats() {
        let mut lib = Library::in_memory().unwrap();
        let track = sample_track();
        lib.insert_track(&track).unwrap();

        let cue = conduction_core::Cue::new(
            track.id, 32.0, CueType::Drop, 128.0, sample_key(), 0.8, 32,
        )
        .unwrap()
        .with_mix_roles([MixRole::Entry]);
        lib.insert_cue(&cue).unwrap();

        let beats = vec![
            Beat::new(0.0, true),
            Beat::new(0.47, false),
            Beat::new(0.94, false),
        ];
        lib.replace_beatgrid(track.id, &beats).unwrap();

        lib.delete_track(track.id).unwrap();

        assert!(lib.list_cues_for_track(track.id).unwrap().is_empty());
        assert!(lib.load_beatgrid(track.id).unwrap().is_empty());
    }

    #[test]
    fn insert_and_list_cues() {
        let lib = Library::in_memory().unwrap();
        let track = sample_track();
        lib.insert_track(&track).unwrap();

        let cue1 = conduction_core::Cue::new(
            track.id, 16.0, CueType::IntroStart, 128.0, sample_key(), 0.4, 16,
        )
        .unwrap();
        let cue2 = conduction_core::Cue::new(
            track.id, 64.0, CueType::Drop, 128.0, sample_key(), 0.9, 32,
        )
        .unwrap()
        .with_mix_roles([MixRole::Entry, MixRole::Exit]);

        lib.insert_cue(&cue1).unwrap();
        lib.insert_cue(&cue2).unwrap();

        let cues = lib.list_cues_for_track(track.id).unwrap();
        assert_eq!(cues.len(), 2);
        // position 昇順
        assert_eq!(cues[0].position_beats, 16.0);
        assert_eq!(cues[1].position_beats, 64.0);
        // MixRole の往復
        assert_eq!(cues[1].mixable_as, BTreeSet::from([MixRole::Entry, MixRole::Exit]));
    }

    #[test]
    fn replace_beatgrid_round_trip() {
        let mut lib = Library::in_memory().unwrap();
        let track = sample_track();
        lib.insert_track(&track).unwrap();

        let beats = vec![
            Beat {
                position_sec: 0.0,
                instantaneous_bpm: Some(128.0),
                is_downbeat: true,
            },
            Beat::new(0.47, false),
        ];
        lib.replace_beatgrid(track.id, &beats).unwrap();

        let got = lib.load_beatgrid(track.id).unwrap();
        assert_eq!(got.len(), 2);
        assert!(got[0].is_downbeat);
        assert_eq!(got[0].instantaneous_bpm, Some(128.0));
        assert!(!got[1].is_downbeat);

        // 再度 replace で差し替わる。
        lib.replace_beatgrid(track.id, &[Beat::new(1.0, true)])
            .unwrap();
        let got = lib.load_beatgrid(track.id).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].position_sec, 1.0);
    }

    #[test]
    fn update_nonexistent_track_errors() {
        let lib = Library::in_memory().unwrap();
        let track = sample_track();
        let err = lib.update_track(&track).unwrap_err();
        matches!(err, LibraryError::TrackNotFound(_));
    }

    #[test]
    fn duplicate_path_insert_errors() {
        let lib = Library::in_memory().unwrap();
        let track = sample_track();
        lib.insert_track(&track).unwrap();
        let err = lib.insert_track(&track).unwrap_err();
        matches!(err, LibraryError::Sqlite(_));
    }

    #[test]
    fn waveform_round_trip() {
        let mut lib = Library::in_memory().unwrap();
        let track = sample_track();
        lib.insert_track(&track).unwrap();

        let bins = 8;
        let wf = WaveformPreview {
            sample_count: bins as u32,
            low: (0..bins).map(|i| i as f32 / bins as f32).collect(),
            mid: (0..bins).map(|i| (bins - i) as f32 / bins as f32).collect(),
            high: vec![0.5; bins],
        };
        lib.save_waveform(track.id, &wf).unwrap();

        let got = lib.load_waveform(track.id).unwrap().unwrap();
        assert_eq!(got.sample_count, wf.sample_count);
        assert_eq!(got.low, wf.low);
        assert_eq!(got.mid, wf.mid);
        assert_eq!(got.high, wf.high);

        // upsert で差し替え。
        let wf2 = WaveformPreview::empty(bins);
        lib.save_waveform(track.id, &wf2).unwrap();
        let got = lib.load_waveform(track.id).unwrap().unwrap();
        assert!(got.low.iter().all(|v| *v == 0.0));

        // delete_track で cascade 削除。
        lib.delete_track(track.id).unwrap();
        assert!(lib.load_waveform(track.id).unwrap().is_none());

        // 仮の追加: 一貫性チェックも確認。
        // (削除後は track 自体が無いので、別のトラックで再インサート)
        let t2 = {
            let mut t = sample_track();
            t.path = PathBuf::from("/tmp/song2.mp3");
            t.id = conduction_core::TrackId::new();
            t
        };
        lib.insert_track(&t2).unwrap();
        let inconsistent = WaveformPreview {
            sample_count: 4,
            low: vec![0.0; 4],
            mid: vec![0.0; 3], // 1 short
            high: vec![0.0; 4],
        };
        let err = lib.save_waveform(t2.id, &inconsistent).unwrap_err();
        assert!(matches!(err, LibraryError::Unsupported(_)));
    }

    #[test]
    fn load_waveform_missing_returns_none() {
        let lib = Library::in_memory().unwrap();
        let id = conduction_core::TrackId::new();
        assert!(lib.load_waveform(id).unwrap().is_none());
    }

    #[test]
    fn hot_cues_upsert_and_list() {
        let lib = Library::in_memory().unwrap();
        let track = sample_track();
        lib.insert_track(&track).unwrap();

        lib.set_hot_cue(track.id, 1, 12.5).unwrap();
        lib.set_hot_cue(track.id, 3, 30.0).unwrap();
        lib.set_hot_cue(track.id, 1, 15.0).unwrap(); // upsert

        let cues = lib.list_hot_cues(track.id).unwrap();
        assert_eq!(cues, vec![(1u8, 15.0), (3u8, 30.0)]);

        lib.delete_hot_cue(track.id, 1).unwrap();
        let cues = lib.list_hot_cues(track.id).unwrap();
        assert_eq!(cues, vec![(3u8, 30.0)]);

        // 範囲外の slot は弾く
        let err = lib.set_hot_cue(track.id, 9, 0.0).unwrap_err();
        assert!(matches!(err, LibraryError::Unsupported(_)));
    }

    #[test]
    fn hot_cues_cascade_on_track_delete() {
        let lib = Library::in_memory().unwrap();
        let track = sample_track();
        lib.insert_track(&track).unwrap();
        lib.set_hot_cue(track.id, 1, 0.0).unwrap();
        lib.delete_track(track.id).unwrap();
        assert!(lib.list_hot_cues(track.id).unwrap().is_empty());
    }

    #[test]
    fn save_track_analysis_updates_bpm_and_beats() {
        let mut lib = Library::in_memory().unwrap();
        let track = sample_track();
        lib.insert_track(&track).unwrap();

        let beats = vec![
            Beat::new(0.0, true),
            Beat::new(0.5, false),
            Beat::new(1.0, false),
            Beat::new(1.5, false),
        ];
        lib.save_track_analysis(track.id, 120.5, &beats).unwrap();

        let got = lib.get_track(track.id).unwrap().unwrap();
        assert!((got.bpm - 120.5).abs() < 0.01);
        assert!(got.analyzed_at.is_some());

        let stored = lib.load_beatgrid(track.id).unwrap();
        assert_eq!(stored.len(), 4);
        assert!(stored[0].is_downbeat);
    }
}
