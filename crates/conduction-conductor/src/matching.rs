//! Cue 動的マッチング (要件 §6.5)。
//!
//! アクティブデッキの再生状態 (BPM, key, energy) を「クエリ」として、
//! ライブラリ全体の Entry 可能な Cue を BPM 距離 × Camelot 互換 × Energy 互換で
//! スコアリングする。スコア順に N 件返す。

use conduction_core::{Cue, Key, MixRole, Track};
use serde::Serialize;

/// アクティブデッキの状態に相当するマッチングクエリ。
#[derive(Debug, Clone, Copy)]
pub struct MatchQuery {
    pub bpm: f32,
    pub key: Key,
    pub energy: f32,
    /// 許容 BPM 差。これを越えるとスコア 0 で除外。
    pub max_bpm_diff: f32,
}

impl MatchQuery {
    pub fn new(bpm: f32, key: Key, energy: f32) -> Self {
        Self {
            bpm,
            key,
            energy,
            max_bpm_diff: 8.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, utoipa::ToSchema)]
pub struct MatchScore {
    pub bpm_score: f32,
    pub key_score: f32,
    pub energy_score: f32,
    pub overall: f32,
}

/// 単一 (cue, track) ペアに対するスコア計算。クエリに対して mix できないなら `None`。
pub fn score(query: &MatchQuery, cue: &Cue, _track: &Track) -> Option<MatchScore> {
    if !cue.can_be(MixRole::Entry) {
        return None;
    }

    // BPM 距離
    let bpm_diff = (query.bpm - cue.bpm_at_cue).abs();
    if bpm_diff > query.max_bpm_diff {
        return None;
    }
    let bpm_score = (1.0 - bpm_diff / query.max_bpm_diff).clamp(0.0, 1.0);

    // Camelot 互換
    let key_score = if !query.key.is_compatible(cue.key_at_cue) {
        return None;
    } else if query.key == cue.key_at_cue {
        1.0
    } else if query.key.camelot_number == cue.key_at_cue.camelot_number {
        // 平行調 (B↔A 同番号) は強い互換
        0.85
    } else {
        // 隣接キー
        0.7
    };

    // Energy 互換
    let energy_score = if cue.compatible_energy.contains(&query.energy) {
        1.0 - (query.energy - cue.energy_level).abs().clamp(0.0, 1.0)
    } else {
        // 範囲外でも段階的に減衰させる (完全 0 にはしない)
        let dist = (query.energy - cue.energy_level).abs().min(1.0);
        (1.0 - dist) * 0.4
    };

    let overall = 0.4 * bpm_score + 0.4 * key_score + 0.2 * energy_score;
    Some(MatchScore {
        bpm_score,
        key_score,
        energy_score,
        overall,
    })
}

#[derive(Debug, Clone)]
pub struct ScoredCue<'a> {
    pub cue: &'a Cue,
    pub track: &'a Track,
    pub score: MatchScore,
}

/// 全 (cue, track) ペアからクエリにマッチする上位 `limit` 件をスコア順に返す。
pub fn find_candidates<'a>(
    query: &MatchQuery,
    pool: &'a [(Cue, Track)],
    limit: usize,
) -> Vec<ScoredCue<'a>> {
    let mut scored: Vec<ScoredCue<'_>> = pool
        .iter()
        .filter_map(|(cue, track)| {
            score(query, cue, track).map(|s| ScoredCue {
                cue,
                track,
                score: s,
            })
        })
        .collect();
    scored.sort_by(|a, b| {
        b.score
            .overall
            .partial_cmp(&a.score.overall)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(limit);
    scored
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use conduction_core::{Cue, CueType, Key, KeyMode, Track, TrackId};

    use super::*;

    fn make_cue(
        track_id: TrackId,
        bpm: f32,
        key: Key,
        energy: f32,
        roles: &[MixRole],
    ) -> Cue {
        let mut c = Cue::new(track_id, 32.0, CueType::IntroStart, bpm, key, energy, 32).unwrap();
        c.mixable_as = BTreeSet::from_iter(roles.iter().copied());
        c
    }

    #[test]
    fn rejects_non_entry_cue() {
        let track = Track::placeholder("/tmp/x".into(), Key::new(8, KeyMode::Minor).unwrap());
        let cue = make_cue(track.id, 128.0, Key::new(8, KeyMode::Minor).unwrap(), 0.5, &[]);
        let q = MatchQuery::new(128.0, Key::new(8, KeyMode::Minor).unwrap(), 0.5);
        assert!(score(&q, &cue, &track).is_none());
    }

    #[test]
    fn perfect_match_scores_high() {
        let track = Track::placeholder("/tmp/x".into(), Key::new(8, KeyMode::Minor).unwrap());
        let cue = make_cue(
            track.id,
            128.0,
            Key::new(8, KeyMode::Minor).unwrap(),
            0.5,
            &[MixRole::Entry],
        );
        let q = MatchQuery::new(128.0, Key::new(8, KeyMode::Minor).unwrap(), 0.5);
        let s = score(&q, &cue, &track).expect("must score");
        assert!(s.overall > 0.95, "perfect match should be ~1.0, got {}", s.overall);
    }

    #[test]
    fn rejects_bpm_too_far() {
        let track = Track::placeholder("/tmp/x".into(), Key::new(8, KeyMode::Minor).unwrap());
        let cue = make_cue(
            track.id,
            128.0,
            Key::new(8, KeyMode::Minor).unwrap(),
            0.5,
            &[MixRole::Entry],
        );
        let q = MatchQuery::new(140.0, Key::new(8, KeyMode::Minor).unwrap(), 0.5);
        assert!(score(&q, &cue, &track).is_none());
    }

    #[test]
    fn rejects_incompatible_key() {
        let track = Track::placeholder("/tmp/x".into(), Key::new(8, KeyMode::Minor).unwrap());
        let cue = make_cue(
            track.id,
            128.0,
            Key::new(2, KeyMode::Minor).unwrap(),
            0.5,
            &[MixRole::Entry],
        );
        let q = MatchQuery::new(128.0, Key::new(8, KeyMode::Minor).unwrap(), 0.5);
        assert!(score(&q, &cue, &track).is_none());
    }

    #[test]
    fn ranks_better_match_higher() {
        let track = Track::placeholder("/tmp/x".into(), Key::new(8, KeyMode::Minor).unwrap());
        let exact = make_cue(
            track.id,
            128.0,
            Key::new(8, KeyMode::Minor).unwrap(),
            0.5,
            &[MixRole::Entry],
        );
        let near = make_cue(
            track.id,
            130.0,
            Key::new(9, KeyMode::Minor).unwrap(),
            0.6,
            &[MixRole::Entry],
        );
        let pool = vec![(near.clone(), track.clone()), (exact.clone(), track.clone())];
        let q = MatchQuery::new(128.0, Key::new(8, KeyMode::Minor).unwrap(), 0.5);
        let cands = find_candidates(&q, &pool, 10);
        assert_eq!(cands.len(), 2);
        assert_eq!(cands[0].cue.id, exact.id, "exact should rank first");
        assert_eq!(cands[1].cue.id, near.id);
    }
}
