use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// 長調 / 短調。Camelot 記法の B / A に対応。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyMode {
    /// 長調。Camelot では `B` サフィックス。
    Major,
    /// 短調。Camelot では `A` サフィックス。
    Minor,
}

/// 音楽キー。内部的には Camelot ホイール上の番号（1〜12）とモードで保持する。
///
/// 要件 15.2: "Key は Camelot記法 (`8A`, `5B`) を第一候補、クラシカル (`Gm`, `Bb`) を従"。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Key {
    /// 1〜12。
    pub camelot_number: u8,
    pub mode: KeyMode,
}

impl Key {
    pub fn new(camelot_number: u8, mode: KeyMode) -> Result<Self, CoreError> {
        if !(1..=12).contains(&camelot_number) {
            return Err(CoreError::InvalidKey(format!(
                "camelot number {camelot_number} out of range 1..=12"
            )));
        }
        Ok(Self {
            camelot_number,
            mode,
        })
    }

    /// Camelot 文字列（例: `"8A"`, `"12B"`）に変換する。
    pub fn to_camelot(self) -> String {
        let suffix = match self.mode {
            KeyMode::Major => 'B',
            KeyMode::Minor => 'A',
        };
        format!("{}{}", self.camelot_number, suffix)
    }

    /// Camelot 互換ルールに基づき "繋げられる" キーかを判定する。
    ///
    /// 許容：
    /// - 完全一致（同じ番号・同じモード）
    /// - 平行調（同じ番号・反対モード）
    /// - 隣接キー（番号 ±1、同じモード、12 → 1 のラップを考慮）
    pub fn is_compatible(self, other: Self) -> bool {
        if self == other {
            return true;
        }
        if self.camelot_number == other.camelot_number && self.mode != other.mode {
            return true;
        }
        if self.mode == other.mode {
            let diff = (self.camelot_number as i16 - other.camelot_number as i16).rem_euclid(12);
            return diff == 1 || diff == 11;
        }
        false
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_camelot())
    }
}

impl FromStr for Key {
    type Err = CoreError;

    /// Camelot 記法のみをパース（例: `8A`, `12B`）。
    /// クラシカル表記（`Gm`, `Bb`）は Phase 3 のキー検出実装時に対応予定。
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.len() < 2 {
            return Err(CoreError::InvalidKey(s.to_string()));
        }
        let (num_part, mode_part) = s.split_at(s.len() - 1);
        let number: u8 = num_part
            .parse()
            .map_err(|_| CoreError::InvalidKey(s.to_string()))?;
        let mode = match mode_part {
            "A" | "a" => KeyMode::Minor,
            "B" | "b" => KeyMode::Major,
            _ => return Err(CoreError::InvalidKey(s.to_string())),
        };
        Self::new(number, mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camelot_roundtrip() {
        for n in 1..=12u8 {
            for mode in [KeyMode::Major, KeyMode::Minor] {
                let key = Key::new(n, mode).unwrap();
                let s = key.to_camelot();
                let parsed: Key = s.parse().unwrap();
                assert_eq!(parsed, key);
            }
        }
    }

    #[test]
    fn rejects_out_of_range() {
        assert!(Key::new(0, KeyMode::Major).is_err());
        assert!(Key::new(13, KeyMode::Minor).is_err());
    }

    #[test]
    fn rejects_bad_notation() {
        assert!("13A".parse::<Key>().is_err());
        assert!("8X".parse::<Key>().is_err());
        assert!("".parse::<Key>().is_err());
        assert!("A".parse::<Key>().is_err());
    }

    #[test]
    fn compatibility_same_key() {
        let k = Key::new(8, KeyMode::Minor).unwrap();
        assert!(k.is_compatible(k));
    }

    #[test]
    fn compatibility_relative() {
        let a_min = Key::new(8, KeyMode::Minor).unwrap();
        let c_maj = Key::new(8, KeyMode::Major).unwrap();
        assert!(a_min.is_compatible(c_maj));
    }

    #[test]
    fn compatibility_adjacent() {
        let k8 = Key::new(8, KeyMode::Minor).unwrap();
        let k9 = Key::new(9, KeyMode::Minor).unwrap();
        let k7 = Key::new(7, KeyMode::Minor).unwrap();
        assert!(k8.is_compatible(k9));
        assert!(k8.is_compatible(k7));
    }

    #[test]
    fn compatibility_wrap() {
        let k12 = Key::new(12, KeyMode::Major).unwrap();
        let k1 = Key::new(1, KeyMode::Major).unwrap();
        assert!(k12.is_compatible(k1));
        assert!(k1.is_compatible(k12));
    }

    #[test]
    fn incompatibility_distant() {
        let k = Key::new(8, KeyMode::Minor).unwrap();
        let far = Key::new(2, KeyMode::Minor).unwrap();
        assert!(!k.is_compatible(far));
    }
}
