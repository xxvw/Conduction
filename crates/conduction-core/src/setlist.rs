//! Setlist データモデル (要件 §6.11)。
//!
//! Phase A1 最小: 曲を順番に並べる + 各エントリに「次曲への遷移仕様」を持たせるところまで。
//! 永続化 (SQLite / .cset エクスポート) と Rehearse モードは別 slice。

use serde::{Deserialize, Serialize};

use crate::ids::{CueId, SetlistEntryId, SetlistId, TrackId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Setlist {
    pub id: SetlistId,
    pub name: String,
    pub entries: Vec<SetlistEntry>,
}

impl Setlist {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: SetlistId::new(),
            name: name.into(),
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetlistEntry {
    pub id: SetlistEntryId,
    pub track_id: TrackId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub play_from_cue: Option<CueId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub play_until_cue: Option<CueId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_to_next: Option<TransitionSpec>,
}

impl SetlistEntry {
    pub fn new(track_id: TrackId) -> Self {
        Self {
            id: SetlistEntryId::new(),
            track_id,
            play_from_cue: None,
            play_until_cue: None,
            transition_to_next: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionSpec {
    /// テンプレートの preset id (例: "preset.long_eq_mix")。後で `TemplateId` (Uuid) に置き換える可能性あり。
    pub template_id: String,
    pub tempo_mode: TempoMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_cue: Option<CueId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_cue: Option<CueId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TempoMode {
    /// 元の BPM をそのまま保持。
    HoldSource,
    /// 遷移後に次曲の BPM へジャンプ。
    MatchTarget,
    /// 遷移中に線形補間。
    LinearBlend,
    /// マスターテンポを基準にする。
    MasterTempo,
}

impl Default for TempoMode {
    fn default() -> Self {
        Self::LinearBlend
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_remove_entries() {
        let mut s = Setlist::new("test");
        let e1 = SetlistEntry::new(TrackId::new());
        let e2 = SetlistEntry::new(TrackId::new());
        s.entries.push(e1.clone());
        s.entries.push(e2.clone());
        assert_eq!(s.entries.len(), 2);
        s.entries.retain(|e| e.id != e1.id);
        assert_eq!(s.entries.len(), 1);
        assert_eq!(s.entries[0].id, e2.id);
    }

    #[test]
    fn serde_roundtrip() {
        let mut s = Setlist::new("night set");
        let mut entry = SetlistEntry::new(TrackId::new());
        entry.transition_to_next = Some(TransitionSpec {
            template_id: "preset.long_eq_mix".into(),
            tempo_mode: TempoMode::LinearBlend,
            entry_cue: None,
            exit_cue: None,
        });
        s.entries.push(entry);

        let json = serde_json::to_string(&s).unwrap();
        let back: Setlist = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "night set");
        assert_eq!(back.entries.len(), 1);
        let tx = back.entries[0].transition_to_next.as_ref().unwrap();
        assert_eq!(tx.template_id, "preset.long_eq_mix");
        assert_eq!(tx.tempo_mode, TempoMode::LinearBlend);
    }
}
