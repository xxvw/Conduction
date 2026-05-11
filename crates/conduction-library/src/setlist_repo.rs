//! Setlist の SQLite 永続化。
//!
//! `Library` の impl block を拡張する形で、setlist CRUD を提供する。
//! 並び替えはトランザクション内で position を一括書き換え。

use std::path::PathBuf;

use chrono::Utc;
use conduction_core::{
    CueId, Setlist, SetlistEntry, SetlistEntryId, SetlistId, TempoMode, TrackId,
    TransitionSpec,
};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{LibraryError, LibraryResult};
use crate::library::Library;
use crate::mapping::dt_to_str;

// --- TempoMode <-> string ---

fn tempo_mode_to_str(m: TempoMode) -> &'static str {
    match m {
        TempoMode::HoldSource => "hold_source",
        TempoMode::MatchTarget => "match_target",
        TempoMode::LinearBlend => "linear_blend",
        TempoMode::MasterTempo => "master_tempo",
    }
}

fn tempo_mode_from_str(s: &str) -> LibraryResult<TempoMode> {
    match s {
        "hold_source" => Ok(TempoMode::HoldSource),
        "match_target" => Ok(TempoMode::MatchTarget),
        "linear_blend" => Ok(TempoMode::LinearBlend),
        "master_tempo" => Ok(TempoMode::MasterTempo),
        other => Err(LibraryError::Unsupported(format!("tempo_mode={other}"))),
    }
}

fn cue_id_from_opt_str(s: Option<String>) -> LibraryResult<Option<CueId>> {
    match s {
        Some(s) => {
            let u = Uuid::parse_str(&s)
                .map_err(|e| LibraryError::Unsupported(format!("cue_id parse: {e}")))?;
            Ok(Some(CueId::from_uuid(u)))
        }
        None => Ok(None),
    }
}

fn entry_from_row(row: &Row<'_>) -> LibraryResult<SetlistEntry> {
    let id_str: String = row.get("id")?;
    let track_id_str: String = row.get("track_id")?;

    let id = SetlistEntryId::from_uuid(
        Uuid::parse_str(&id_str)
            .map_err(|e| LibraryError::Unsupported(format!("entry id: {e}")))?,
    );
    let track_id = TrackId::from_uuid(
        Uuid::parse_str(&track_id_str)
            .map_err(|e| LibraryError::Unsupported(format!("track_id: {e}")))?,
    );

    let play_from_cue = cue_id_from_opt_str(row.get("play_from_cue")?)?;
    let play_until_cue = cue_id_from_opt_str(row.get("play_until_cue")?)?;

    let tmpl: Option<String> = row.get("transition_template_id")?;
    let transition_to_next = match tmpl {
        Some(template_id) => {
            let tempo_mode_str: String = row
                .get::<_, Option<String>>("transition_tempo_mode")?
                .unwrap_or_else(|| "linear_blend".into());
            Some(TransitionSpec {
                template_id,
                tempo_mode: tempo_mode_from_str(&tempo_mode_str)?,
                entry_cue: cue_id_from_opt_str(row.get("transition_entry_cue")?)?,
                exit_cue: cue_id_from_opt_str(row.get("transition_exit_cue")?)?,
            })
        }
        None => None,
    };

    Ok(SetlistEntry {
        id,
        track_id,
        play_from_cue,
        play_until_cue,
        transition_to_next,
    })
}

/// 1 つの setlist について entries を position 昇順で読み込む内部ヘルパ。
fn load_entries(conn: &Connection, setlist_id: SetlistId) -> LibraryResult<Vec<SetlistEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, setlist_id, position, track_id, play_from_cue, play_until_cue,
                transition_template_id, transition_tempo_mode,
                transition_entry_cue, transition_exit_cue
         FROM setlist_entries
         WHERE setlist_id = ?1
         ORDER BY position ASC",
    )?;
    let entries = stmt
        .query_map(params![setlist_id.as_uuid().to_string()], |row| {
            Ok(entry_from_row(row))
        })?
        .collect::<Result<Result<Vec<_>, _>, _>>()??;
    Ok(entries)
}

impl Library {
    // -------- Setlist --------

    pub fn list_setlists(&self) -> LibraryResult<Vec<Setlist>> {
        let conn = self.raw_conn();
        let mut stmt = conn.prepare(
            "SELECT id, name FROM setlists ORDER BY created_at ASC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                let id_str: String = row.get(0)?;
                let name: String = row.get(1)?;
                Ok((id_str, name))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut out = Vec::with_capacity(rows.len());
        for (id_str, name) in rows {
            let id = SetlistId::from_uuid(
                Uuid::parse_str(&id_str)
                    .map_err(|e| LibraryError::Unsupported(format!("setlist id: {e}")))?,
            );
            let entries = load_entries(conn, id)?;
            out.push(Setlist { id, name, entries });
        }
        Ok(out)
    }

    pub fn get_setlist(&self, id: SetlistId) -> LibraryResult<Option<Setlist>> {
        let conn = self.raw_conn();
        let row = conn
            .query_row(
                "SELECT name FROM setlists WHERE id = ?1 LIMIT 1",
                params![id.as_uuid().to_string()],
                |row| row.get::<_, String>(0),
            )
            .ok();
        let Some(name) = row else { return Ok(None) };
        let entries = load_entries(conn, id)?;
        Ok(Some(Setlist { id, name, entries }))
    }

    pub fn create_setlist(&self, name: String) -> LibraryResult<Setlist> {
        let setlist = Setlist::new(name);
        let now = dt_to_str(Utc::now());
        let conn = self.raw_conn();
        conn.execute(
            "INSERT INTO setlists (id, name, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?3)",
            params![setlist.id.as_uuid().to_string(), setlist.name, now],
        )?;
        Ok(setlist)
    }

    pub fn delete_setlist(&self, id: SetlistId) -> LibraryResult<()> {
        let conn = self.raw_conn();
        let affected = conn.execute(
            "DELETE FROM setlists WHERE id = ?1",
            params![id.as_uuid().to_string()],
        )?;
        if affected == 0 {
            return Err(LibraryError::Unsupported(format!(
                "setlist not found: {id}"
            )));
        }
        Ok(())
    }

    pub fn rename_setlist(&self, id: SetlistId, name: String) -> LibraryResult<Setlist> {
        let now = dt_to_str(Utc::now());
        let conn = self.raw_conn();
        let affected = conn.execute(
            "UPDATE setlists SET name = ?2, updated_at = ?3 WHERE id = ?1",
            params![id.as_uuid().to_string(), name, now],
        )?;
        if affected == 0 {
            return Err(LibraryError::Unsupported(format!(
                "setlist not found: {id}"
            )));
        }
        self.get_setlist(id)?.ok_or_else(|| {
            LibraryError::Unsupported(format!("setlist disappeared: {id}"))
        })
    }

    pub fn add_setlist_entry(
        &mut self,
        id: SetlistId,
        track_id: TrackId,
    ) -> LibraryResult<SetlistEntry> {
        let tx = self.raw_conn_mut().transaction()?;
        // setlist 存在確認
        let exists: bool = tx
            .query_row(
                "SELECT 1 FROM setlists WHERE id = ?1",
                params![id.as_uuid().to_string()],
                |_| Ok(true),
            )
            .ok()
            .unwrap_or(false);
        if !exists {
            return Err(LibraryError::Unsupported(format!(
                "setlist not found: {id}"
            )));
        }

        let next_pos: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM setlist_entries
             WHERE setlist_id = ?1",
            params![id.as_uuid().to_string()],
            |row| row.get(0),
        )?;
        let entry = SetlistEntry::new(track_id);
        let now = dt_to_str(Utc::now());
        tx.execute(
            "INSERT INTO setlist_entries
               (id, setlist_id, position, track_id,
                play_from_cue, play_until_cue,
                transition_template_id, transition_tempo_mode,
                transition_entry_cue, transition_exit_cue,
                created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, NULL, NULL, NULL, NULL, NULL, NULL, ?5, ?5)",
            params![
                entry.id.as_uuid().to_string(),
                id.as_uuid().to_string(),
                next_pos,
                track_id.as_uuid().to_string(),
                now,
            ],
        )?;
        tx.execute(
            "UPDATE setlists SET updated_at = ?2 WHERE id = ?1",
            params![id.as_uuid().to_string(), now],
        )?;
        tx.commit()?;
        Ok(entry)
    }

    pub fn remove_setlist_entry(
        &mut self,
        id: SetlistId,
        entry_id: SetlistEntryId,
    ) -> LibraryResult<()> {
        let tx = self.raw_conn_mut().transaction()?;
        let affected = tx.execute(
            "DELETE FROM setlist_entries WHERE id = ?1 AND setlist_id = ?2",
            params![entry_id.as_uuid().to_string(), id.as_uuid().to_string()],
        )?;
        if affected == 0 {
            return Err(LibraryError::Unsupported(format!(
                "setlist entry not found: {entry_id}"
            )));
        }
        // 残り entry の position を 0 から振り直す
        renumber_positions(&tx, id)?;
        let now = dt_to_str(Utc::now());
        tx.execute(
            "UPDATE setlists SET updated_at = ?2 WHERE id = ?1",
            params![id.as_uuid().to_string(), now],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn move_setlist_entry(
        &mut self,
        id: SetlistId,
        entry_id: SetlistEntryId,
        new_index: i64,
    ) -> LibraryResult<Setlist> {
        let tx = self.raw_conn_mut().transaction()?;
        let mut entry_ids: Vec<String> = {
            let mut stmt = tx.prepare(
                "SELECT id FROM setlist_entries
                 WHERE setlist_id = ?1 ORDER BY position ASC",
            )?;
            let v = stmt
                .query_map(params![id.as_uuid().to_string()], |row| {
                    row.get::<_, String>(0)
                })?
                .collect::<Result<Vec<_>, _>>()?;
            v
        };
        let from = entry_ids
            .iter()
            .position(|s| s == &entry_id.as_uuid().to_string())
            .ok_or_else(|| {
                LibraryError::Unsupported(format!("entry not found: {entry_id}"))
            })?;
        let max = entry_ids.len().saturating_sub(1) as i64;
        let to = new_index.clamp(0, max) as usize;
        let item = entry_ids.remove(from);
        entry_ids.insert(to, item);
        for (pos, eid) in entry_ids.iter().enumerate() {
            tx.execute(
                "UPDATE setlist_entries SET position = ?2 WHERE id = ?1",
                params![eid, pos as i64],
            )?;
        }
        let now = dt_to_str(Utc::now());
        tx.execute(
            "UPDATE setlists SET updated_at = ?2 WHERE id = ?1",
            params![id.as_uuid().to_string(), now],
        )?;
        tx.commit()?;
        self.get_setlist(id)?.ok_or_else(|| {
            LibraryError::Unsupported(format!("setlist disappeared: {id}"))
        })
    }

    pub fn set_setlist_transition(
        &mut self,
        id: SetlistId,
        entry_id: SetlistEntryId,
        spec: Option<TransitionSpec>,
    ) -> LibraryResult<SetlistEntry> {
        let conn = self.raw_conn();
        let now = dt_to_str(Utc::now());
        let affected = match spec.as_ref() {
            Some(s) => conn.execute(
                "UPDATE setlist_entries SET
                   transition_template_id = ?3,
                   transition_tempo_mode = ?4,
                   transition_entry_cue = ?5,
                   transition_exit_cue = ?6,
                   updated_at = ?7
                 WHERE id = ?1 AND setlist_id = ?2",
                params![
                    entry_id.as_uuid().to_string(),
                    id.as_uuid().to_string(),
                    s.template_id,
                    tempo_mode_to_str(s.tempo_mode),
                    s.entry_cue.map(|c| c.as_uuid().to_string()),
                    s.exit_cue.map(|c| c.as_uuid().to_string()),
                    now,
                ],
            )?,
            None => conn.execute(
                "UPDATE setlist_entries SET
                   transition_template_id = NULL,
                   transition_tempo_mode = NULL,
                   transition_entry_cue = NULL,
                   transition_exit_cue = NULL,
                   updated_at = ?3
                 WHERE id = ?1 AND setlist_id = ?2",
                params![
                    entry_id.as_uuid().to_string(),
                    id.as_uuid().to_string(),
                    now,
                ],
            )?,
        };
        if affected == 0 {
            return Err(LibraryError::Unsupported(format!(
                "setlist entry not found: {entry_id}"
            )));
        }
        // 更新した行を再読み込み
        let mut stmt = conn.prepare(
            "SELECT id, setlist_id, position, track_id, play_from_cue, play_until_cue,
                    transition_template_id, transition_tempo_mode,
                    transition_entry_cue, transition_exit_cue
             FROM setlist_entries WHERE id = ?1 LIMIT 1",
        )?;
        let entry = stmt.query_row(params![entry_id.as_uuid().to_string()], |row| {
            Ok(entry_from_row(row))
        })??;
        Ok(entry)
    }
}

fn renumber_positions(conn: &Connection, setlist_id: SetlistId) -> LibraryResult<()> {
    let ids: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM setlist_entries
             WHERE setlist_id = ?1 ORDER BY position ASC",
        )?;
        let v = stmt
            .query_map(params![setlist_id.as_uuid().to_string()], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        v
    };
    for (pos, eid) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE setlist_entries SET position = ?2 WHERE id = ?1",
            params![eid, pos as i64],
        )?;
    }
    Ok(())
}

// ========== .cset Export / Import (Phase C2) ==========
//
// 形式: 単一 JSON ファイル (拡張子 .cset)。
// UUID 系 (setlist/entry/cue id) はマシン跨ぎで意味を持たないので落とす。
// track は path → (title, artist) の順で resolve、いずれもヒットしなければ skip。

const CSET_FORMAT: &str = "conduction.cset";
const CSET_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsetEnvelope {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub setlist: CsetSetlist,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsetSetlist {
    pub name: String,
    pub entries: Vec<CsetEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsetEntry {
    pub track_meta: CsetTrackMeta,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_to_next: Option<CsetTransition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsetTrackMeta {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub bpm: f32,
    /// Camelot 表記 ("8A" 等)。
    pub key: String,
    pub duration_sec: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsetTransition {
    pub template_id: String,
    pub tempo_mode: TempoMode,
    // cue id 参照はマシン跨ぎで意味がないため落とす。
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetlistImportReport {
    pub setlist_id: String,
    pub setlist_name: String,
    pub total_entries: usize,
    pub resolved_entries: usize,
    pub missing_tracks: Vec<CsetTrackMeta>,
}

impl Library {
    /// 指定 setlist を `.cset` JSON 文字列にシリアライズする。
    pub fn export_setlist_json(&self, id: SetlistId) -> LibraryResult<String> {
        let setlist = self
            .get_setlist(id)?
            .ok_or_else(|| LibraryError::Unsupported(format!("setlist not found: {id}")))?;

        let mut entries = Vec::with_capacity(setlist.entries.len());
        for e in &setlist.entries {
            // track_id → track 解決。見つからなければ skip。
            let Some(track) = self.get_track(e.track_id)? else { continue };
            entries.push(CsetEntry {
                track_meta: CsetTrackMeta {
                    path: track.path.to_string_lossy().into_owned(),
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    bpm: track.bpm,
                    key: track.key.to_camelot(),
                    duration_sec: track.duration.as_secs_f64(),
                },
                transition_to_next: e.transition_to_next.as_ref().map(|t| CsetTransition {
                    template_id: t.template_id.clone(),
                    tempo_mode: t.tempo_mode,
                }),
            });
        }

        let env = CsetEnvelope {
            format: CSET_FORMAT.to_string(),
            version: CSET_VERSION,
            exported_at: dt_to_str(Utc::now()),
            setlist: CsetSetlist {
                name: setlist.name,
                entries,
            },
        };
        Ok(serde_json::to_string_pretty(&env)
            .map_err(|e| LibraryError::Unsupported(format!("json serialize: {e}")))?)
    }

    /// `.cset` JSON を読み、新しい setlist を作成する。
    /// track の解決は path → (title, artist) の順。いずれもヒットしない entry は missing_tracks に積む。
    pub fn import_setlist_json(&mut self, payload: &str) -> LibraryResult<SetlistImportReport> {
        let env: CsetEnvelope = serde_json::from_str(payload)
            .map_err(|e| LibraryError::Unsupported(format!("json parse: {e}")))?;
        if env.format != CSET_FORMAT {
            return Err(LibraryError::Unsupported(format!(
                "unexpected format: {}",
                env.format
            )));
        }
        if env.version != CSET_VERSION {
            return Err(LibraryError::Unsupported(format!(
                "unsupported version: {}",
                env.version
            )));
        }

        let setlist = self.create_setlist(env.setlist.name.clone())?;
        let mut resolved_entries = 0usize;
        let mut missing_tracks = Vec::new();

        let total_entries = env.setlist.entries.len();
        // (entry_id, transition) を後で適用するためのキュー
        let mut pending_transitions: Vec<(SetlistEntryId, CsetTransition)> = Vec::new();

        for csv_entry in env.setlist.entries {
            let track_id = match self.resolve_track(&csv_entry.track_meta)? {
                Some(id) => id,
                None => {
                    missing_tracks.push(csv_entry.track_meta);
                    continue;
                }
            };
            let entry = self.add_setlist_entry(setlist.id, track_id)?;
            resolved_entries += 1;
            if let Some(tx) = csv_entry.transition_to_next {
                pending_transitions.push((entry.id, tx));
            }
        }

        for (entry_id, tx) in pending_transitions {
            self.set_setlist_transition(
                setlist.id,
                entry_id,
                Some(TransitionSpec {
                    template_id: tx.template_id,
                    tempo_mode: tx.tempo_mode,
                    entry_cue: None,
                    exit_cue: None,
                }),
            )?;
        }

        Ok(SetlistImportReport {
            setlist_id: setlist.id.as_uuid().to_string(),
            setlist_name: setlist.name,
            total_entries,
            resolved_entries,
            missing_tracks,
        })
    }

    fn resolve_track(&self, meta: &CsetTrackMeta) -> LibraryResult<Option<TrackId>> {
        // 1) path 完全一致
        if let Some(t) = self.get_track_by_path(&PathBuf::from(&meta.path))? {
            return Ok(Some(t.id));
        }
        // 2) (title, artist) で fuzzy 一致 (大文字小文字無視)
        let title_l = meta.title.to_lowercase();
        let artist_l = meta.artist.to_lowercase();
        for t in self.list_tracks()? {
            if !title_l.is_empty()
                && !artist_l.is_empty()
                && t.title.to_lowercase() == title_l
                && t.artist.to_lowercase() == artist_l
            {
                return Ok(Some(t.id));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use conduction_core::{Key, KeyMode, TempoMode, Track, TransitionSpec};

    use super::*;

    fn sample_track(path: &str) -> Track {
        let key = Key::new(8, KeyMode::Minor).unwrap();
        let mut t = Track::placeholder(PathBuf::from(path), key);
        t.bpm = 120.0;
        t
    }

    #[test]
    fn create_list_and_delete_setlist() {
        let lib = Library::in_memory().unwrap();
        let s1 = lib.create_setlist("Set A".into()).unwrap();
        let s2 = lib.create_setlist("Set B".into()).unwrap();

        let all = lib.list_setlists().unwrap();
        assert_eq!(all.len(), 2);
        let names: Vec<_> = all.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Set A"));
        assert!(names.contains(&"Set B"));

        lib.delete_setlist(s1.id).unwrap();
        let all = lib.list_setlists().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, s2.id);
    }

    #[test]
    fn rename_setlist() {
        let lib = Library::in_memory().unwrap();
        let s = lib.create_setlist("Old".into()).unwrap();
        let renamed = lib.rename_setlist(s.id, "New".into()).unwrap();
        assert_eq!(renamed.name, "New");
    }

    #[test]
    fn add_remove_entries_keep_position_contiguous() {
        let mut lib = Library::in_memory().unwrap();
        let t1 = sample_track("/tmp/a.mp3");
        let t2 = sample_track("/tmp/b.mp3");
        let t3 = sample_track("/tmp/c.mp3");
        lib.insert_track(&t1).unwrap();
        lib.insert_track(&t2).unwrap();
        lib.insert_track(&t3).unwrap();

        let s = lib.create_setlist("X".into()).unwrap();
        let e1 = lib.add_setlist_entry(s.id, t1.id).unwrap();
        let e2 = lib.add_setlist_entry(s.id, t2.id).unwrap();
        let _e3 = lib.add_setlist_entry(s.id, t3.id).unwrap();

        let got = lib.get_setlist(s.id).unwrap().unwrap();
        assert_eq!(got.entries.len(), 3);
        assert_eq!(got.entries[0].track_id, t1.id);
        assert_eq!(got.entries[2].track_id, t3.id);

        // 中央を削除すると残りの順序が維持され、position が詰まる
        lib.remove_setlist_entry(s.id, e2.id).unwrap();
        let got = lib.get_setlist(s.id).unwrap().unwrap();
        assert_eq!(got.entries.len(), 2);
        assert_eq!(got.entries[0].id, e1.id);
        assert_eq!(got.entries[1].track_id, t3.id);
    }

    #[test]
    fn move_entry_reorders() {
        let mut lib = Library::in_memory().unwrap();
        let t1 = sample_track("/tmp/a.mp3");
        let t2 = sample_track("/tmp/b.mp3");
        let t3 = sample_track("/tmp/c.mp3");
        lib.insert_track(&t1).unwrap();
        lib.insert_track(&t2).unwrap();
        lib.insert_track(&t3).unwrap();
        let s = lib.create_setlist("X".into()).unwrap();
        let e1 = lib.add_setlist_entry(s.id, t1.id).unwrap();
        let _e2 = lib.add_setlist_entry(s.id, t2.id).unwrap();
        let _e3 = lib.add_setlist_entry(s.id, t3.id).unwrap();

        // e1 を末尾へ
        let after = lib.move_setlist_entry(s.id, e1.id, 2).unwrap();
        let ids: Vec<_> = after.entries.iter().map(|e| e.track_id).collect();
        assert_eq!(ids, vec![t2.id, t3.id, t1.id]);

        // 範囲外は端へクランプ
        let after = lib.move_setlist_entry(s.id, e1.id, 99).unwrap();
        assert_eq!(after.entries.last().unwrap().id, e1.id);
    }

    #[test]
    fn set_transition_roundtrip() {
        let mut lib = Library::in_memory().unwrap();
        let t1 = sample_track("/tmp/a.mp3");
        let t2 = sample_track("/tmp/b.mp3");
        lib.insert_track(&t1).unwrap();
        lib.insert_track(&t2).unwrap();
        let s = lib.create_setlist("X".into()).unwrap();
        let e1 = lib.add_setlist_entry(s.id, t1.id).unwrap();
        let _e2 = lib.add_setlist_entry(s.id, t2.id).unwrap();

        let spec = TransitionSpec {
            template_id: "preset.long_eq_mix".into(),
            tempo_mode: TempoMode::MatchTarget,
            entry_cue: None,
            exit_cue: None,
        };
        let updated = lib
            .set_setlist_transition(s.id, e1.id, Some(spec.clone()))
            .unwrap();
        let tx = updated.transition_to_next.as_ref().unwrap();
        assert_eq!(tx.template_id, "preset.long_eq_mix");
        assert_eq!(tx.tempo_mode, TempoMode::MatchTarget);

        // クリア
        let cleared = lib
            .set_setlist_transition(s.id, e1.id, None)
            .unwrap();
        assert!(cleared.transition_to_next.is_none());
    }

    #[test]
    fn delete_track_cascades_setlist_entries() {
        let mut lib = Library::in_memory().unwrap();
        let t1 = sample_track("/tmp/a.mp3");
        lib.insert_track(&t1).unwrap();
        let s = lib.create_setlist("X".into()).unwrap();
        lib.add_setlist_entry(s.id, t1.id).unwrap();
        assert_eq!(lib.get_setlist(s.id).unwrap().unwrap().entries.len(), 1);
        lib.delete_track(t1.id).unwrap();
        assert_eq!(lib.get_setlist(s.id).unwrap().unwrap().entries.len(), 0);
    }

    #[test]
    fn cset_export_import_roundtrip_via_path() {
        let mut lib_a = Library::in_memory().unwrap();
        let mut t1 = sample_track("/tmp/a.mp3");
        t1.title = "Alpha".into();
        let mut t2 = sample_track("/tmp/b.mp3");
        t2.title = "Beta".into();
        lib_a.insert_track(&t1).unwrap();
        lib_a.insert_track(&t2).unwrap();
        let s = lib_a.create_setlist("Night".into()).unwrap();
        let e1 = lib_a.add_setlist_entry(s.id, t1.id).unwrap();
        lib_a.add_setlist_entry(s.id, t2.id).unwrap();
        lib_a
            .set_setlist_transition(
                s.id,
                e1.id,
                Some(TransitionSpec {
                    template_id: "preset.long_eq_mix".into(),
                    tempo_mode: TempoMode::MatchTarget,
                    entry_cue: None,
                    exit_cue: None,
                }),
            )
            .unwrap();

        let json = lib_a.export_setlist_json(s.id).unwrap();

        // 受信側のライブラリ: 同じ path のトラックが入っている。
        let mut lib_b = Library::in_memory().unwrap();
        lib_b.insert_track(&t1).unwrap();
        lib_b.insert_track(&t2).unwrap();
        let report = lib_b.import_setlist_json(&json).unwrap();
        assert_eq!(report.total_entries, 2);
        assert_eq!(report.resolved_entries, 2);
        assert!(report.missing_tracks.is_empty());

        let setlists = lib_b.list_setlists().unwrap();
        assert_eq!(setlists.len(), 1);
        let imported = &setlists[0];
        assert_eq!(imported.name, "Night");
        assert_eq!(imported.entries.len(), 2);
        let tx = imported.entries[0].transition_to_next.as_ref().unwrap();
        assert_eq!(tx.template_id, "preset.long_eq_mix");
        assert_eq!(tx.tempo_mode, TempoMode::MatchTarget);
    }

    #[test]
    fn cset_missing_track_recorded_in_report() {
        let mut lib_a = Library::in_memory().unwrap();
        let t1 = sample_track("/tmp/a.mp3");
        lib_a.insert_track(&t1).unwrap();
        let s = lib_a.create_setlist("X".into()).unwrap();
        lib_a.add_setlist_entry(s.id, t1.id).unwrap();
        let json = lib_a.export_setlist_json(s.id).unwrap();

        // 受信側: 該当 track 無し。
        let mut lib_b = Library::in_memory().unwrap();
        let report = lib_b.import_setlist_json(&json).unwrap();
        assert_eq!(report.total_entries, 1);
        assert_eq!(report.resolved_entries, 0);
        assert_eq!(report.missing_tracks.len(), 1);
        // setlist は空でも作成される
        assert_eq!(lib_b.list_setlists().unwrap().len(), 1);
    }

    #[test]
    fn cset_rejects_wrong_format_or_version() {
        let mut lib = Library::in_memory().unwrap();
        assert!(lib.import_setlist_json("{ \"format\": \"x\", \"version\": 1, \"exported_at\": \"x\", \"setlist\": { \"name\": \"n\", \"entries\": [] } }").is_err());
        assert!(lib.import_setlist_json("{ \"format\": \"conduction.cset\", \"version\": 99, \"exported_at\": \"x\", \"setlist\": { \"name\": \"n\", \"entries\": [] } }").is_err());
    }
}
