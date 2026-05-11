//! conduction-script — Lua スクリプトから Template へのコンパイラ (要件 §6.7)。
//!
//! 設計方針:
//! - Lua はテンプレート定義を組み立てるための「コードジェネレータ」。
//!   オーディオスレッドでは走らせず、保存時 / 編集時に 1 度だけ評価して
//!   `Template` (= AutomationTrack の集合) に変換する。
//! - 既存の Visual / Node エディタとは独立に動く。Compile 結果の Template を
//!   AutomationTrack[] として返すので、Visual / Node / 再生パイプの全てが
//!   そのまま使える。
//! - Lua の sandbox は math / string / table のみ許可。io / os / package /
//!   debug は無効化 (ファイル I/O やプロセス操作を遮断)。
//!
//! 提供される API (Lua グローバル):
//! - `duration_beats` (number, 初期値は compile 引数で渡す)
//! - `set_duration(n)`         — duration_beats を変更
//! - `add_keyframe(target, beat, value [, curve])` — 該当ターゲットに keyframe 追加
//! - `add_track(target, table)`                    — keyframes 配列をまとめて指定
//!
//! `target` 文字列の例: "crossfader", "master_volume", "deck_volume.A",
//! "deck_eq_low.B", "deck_filter.A" 等。
//!
//! `curve` 文字列の例: "linear" (default), "ease_in", "ease_out",
//! "ease_in_out", "step", "hold"。

#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use conduction_conductor::template::{
    AutomationTrack, BuiltInTarget, CurveType, DeckSlot, Keyframe, Template, TimePosition,
};
use mlua::{Function, Lua, LuaOptions, StdLib, Table, Value};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Lua から組み立てたテンプレートの状態。Rc<RefCell<>> で Lua クロージャ群から共有する。
#[derive(Debug, Default)]
struct ScriptState {
    duration_beats: f64,
    /// target_key → AutomationTrack。同じ target に複数回 add_keyframe したら 1 track に集約。
    tracks: HashMap<String, AutomationTrack>,
    /// 挿入順序を保つための target_key の出現順。
    order: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ScriptError {
    #[error("Lua runtime error: {0}")]
    Lua(String),
    #[error("invalid target: {0}")]
    InvalidTarget(String),
    #[error("invalid curve: {0}")]
    InvalidCurve(String),
    #[error("invalid beat: {0}")]
    InvalidBeat(String),
}

impl From<mlua::Error> for ScriptError {
    fn from(e: mlua::Error) -> Self {
        ScriptError::Lua(e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileOptions {
    pub default_duration_beats: f64,
    pub template_id: Option<String>,
    pub template_name: Option<String>,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            default_duration_beats: 32.0,
            template_id: None,
            template_name: None,
        }
    }
}

/// Lua スクリプトを評価して Template を返す。
pub fn compile_lua_to_template(
    source: &str,
    opts: CompileOptions,
) -> Result<Template, ScriptError> {
    // Sandbox: math / string / table のみを有効化。io/os/package/debug は外す。
    let lua = Lua::new_with(
        StdLib::MATH | StdLib::STRING | StdLib::TABLE,
        LuaOptions::default(),
    )?;

    let state = Rc::new(RefCell::new(ScriptState {
        duration_beats: opts.default_duration_beats,
        tracks: HashMap::new(),
        order: Vec::new(),
    }));

    // duration_beats をグローバルにセット。
    lua.globals().set("duration_beats", opts.default_duration_beats)?;

    // set_duration(n)
    {
        let state = Rc::clone(&state);
        let f = lua.create_function(move |lua, beats: f64| {
            if !beats.is_finite() || beats <= 0.0 {
                return Err(mlua::Error::external(ScriptError::InvalidBeat(format!(
                    "set_duration({beats})"
                ))));
            }
            state.borrow_mut().duration_beats = beats;
            lua.globals().set("duration_beats", beats)?;
            Ok(())
        })?;
        lua.globals().set("set_duration", f)?;
    }

    // add_keyframe(target, beat, value [, curve])
    {
        let state = Rc::clone(&state);
        let f = lua.create_function(
            move |_, (target_key, beat, value, curve): (String, f64, f64, Option<String>)| {
                let target = parse_target(&target_key)
                    .map_err(mlua::Error::external)?;
                if !beat.is_finite() || beat < 0.0 {
                    return Err(mlua::Error::external(ScriptError::InvalidBeat(format!(
                        "add_keyframe(beat={beat})"
                    ))));
                }
                let curve = parse_curve(curve.as_deref().unwrap_or("linear"))
                    .map_err(mlua::Error::external)?;
                let kf = Keyframe {
                    position: TimePosition::Beats(beat),
                    value: value as f32,
                    curve,
                };
                push_keyframe(&mut state.borrow_mut(), target_key, target, kf);
                Ok(())
            },
        )?;
        lua.globals().set("add_keyframe", f)?;
    }

    // add_track(target, { {beat, value, curve}, ... })
    {
        let state = Rc::clone(&state);
        let f: Function = lua.create_function(
            move |_, (target_key, kfs_table): (String, Table)| {
                let target = parse_target(&target_key)
                    .map_err(mlua::Error::external)?;
                let mut collected: Vec<Keyframe> = Vec::new();
                for pair in kfs_table.sequence_values::<Table>() {
                    let kf_t: Table = pair?;
                    // 受け入れる形:
                    //  1) {beat = ..., value = ..., curve = ...}
                    //  2) {[1] = beat, [2] = value, [3] = curve}
                    let beat: f64 = kf_t
                        .get::<Value>("beat")
                        .ok()
                        .and_then(|v| if let Value::Nil = v { None } else { Some(v) })
                        .map(|v| value_to_f64(v))
                        .unwrap_or_else(|| {
                            kf_t.get::<f64>(1).unwrap_or(0.0)
                        });
                    let value: f64 = kf_t
                        .get::<Value>("value")
                        .ok()
                        .and_then(|v| if let Value::Nil = v { None } else { Some(v) })
                        .map(|v| value_to_f64(v))
                        .unwrap_or_else(|| {
                            kf_t.get::<f64>(2).unwrap_or(0.0)
                        });
                    let curve_str: String = kf_t
                        .get::<Value>("curve")
                        .ok()
                        .and_then(|v| match v {
                            Value::String(s) => Some(s.to_str().ok()?.to_owned()),
                            _ => None,
                        })
                        .or_else(|| kf_t.get::<String>(3).ok())
                        .unwrap_or_else(|| "linear".to_string());

                    if !beat.is_finite() || beat < 0.0 {
                        return Err(mlua::Error::external(ScriptError::InvalidBeat(format!(
                            "add_track(beat={beat})"
                        ))));
                    }
                    let curve =
                        parse_curve(&curve_str).map_err(mlua::Error::external)?;
                    collected.push(Keyframe {
                        position: TimePosition::Beats(beat),
                        value: value as f32,
                        curve,
                    });
                }
                let mut st = state.borrow_mut();
                for kf in collected {
                    push_keyframe(&mut st, target_key.clone(), target, kf);
                }
                Ok(())
            },
        )?;
        lua.globals().set("add_track", f)?;
    }

    // 評価。
    lua.load(source).exec()?;

    // 結果を取り出す。
    let st = state.borrow();
    let mut tracks: Vec<AutomationTrack> = Vec::with_capacity(st.order.len());
    for key in &st.order {
        if let Some(track) = st.tracks.get(key) {
            // beat 昇順に並べ替えてから push (Visual エディタの描画と整合する)。
            let mut sorted = track.clone();
            sorted.keyframes.sort_by(|a, b| {
                let ab = match a.position {
                    TimePosition::Beats(b) => b,
                    _ => 0.0,
                };
                let bb = match b.position {
                    TimePosition::Beats(b) => b,
                    _ => 0.0,
                };
                ab.partial_cmp(&bb).unwrap_or(std::cmp::Ordering::Equal)
            });
            tracks.push(sorted);
        }
    }

    Ok(Template {
        id: opts.template_id.unwrap_or_default(),
        name: opts.template_name.unwrap_or_else(|| "Untitled Script".into()),
        duration_beats: st.duration_beats,
        tracks,
        source: Some(source.to_string()),
    })
}

fn value_to_f64(v: Value) -> f64 {
    match v {
        Value::Integer(i) => i as f64,
        Value::Number(n) => n,
        Value::Boolean(b) => {
            if b {
                1.0
            } else {
                0.0
            }
        }
        _ => 0.0,
    }
}

fn push_keyframe(
    state: &mut ScriptState,
    target_key: String,
    target: BuiltInTarget,
    kf: Keyframe,
) {
    let entry = state.tracks.entry(target_key.clone()).or_insert_with(|| {
        AutomationTrack {
            target,
            keyframes: Vec::new(),
        }
    });
    entry.keyframes.push(kf);
    if !state.order.contains(&target_key) {
        state.order.push(target_key);
    }
}

fn parse_target(key: &str) -> Result<BuiltInTarget, ScriptError> {
    match key {
        "crossfader" => Ok(BuiltInTarget::Crossfader),
        "master_volume" => Ok(BuiltInTarget::MasterVolume),
        s => {
            // "deck_xxx.A" / "deck_xxx.B" 形式
            let Some((kind, deck_s)) = s.split_once('.') else {
                return Err(ScriptError::InvalidTarget(key.to_string()));
            };
            let deck = match deck_s {
                "A" | "a" => DeckSlot::A,
                "B" | "b" => DeckSlot::B,
                _ => return Err(ScriptError::InvalidTarget(key.to_string())),
            };
            Ok(match kind {
                "deck_volume" => BuiltInTarget::DeckVolume { deck },
                "deck_eq_low" => BuiltInTarget::DeckEqLow { deck },
                "deck_eq_mid" => BuiltInTarget::DeckEqMid { deck },
                "deck_eq_high" => BuiltInTarget::DeckEqHigh { deck },
                "deck_filter" => BuiltInTarget::DeckFilter { deck },
                "deck_echo_wet" => BuiltInTarget::DeckEchoWet { deck },
                "deck_reverb_wet" => BuiltInTarget::DeckReverbWet { deck },
                _ => return Err(ScriptError::InvalidTarget(key.to_string())),
            })
        }
    }
}

fn parse_curve(s: &str) -> Result<CurveType, ScriptError> {
    match s {
        "linear" => Ok(CurveType::Linear),
        "ease_in" => Ok(CurveType::EaseIn),
        "ease_out" => Ok(CurveType::EaseOut),
        "ease_in_out" => Ok(CurveType::EaseInOut),
        "step" => Ok(CurveType::Step),
        "hold" => Ok(CurveType::Hold),
        other => Err(ScriptError::InvalidCurve(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_script_returns_template_with_no_tracks() {
        let t = compile_lua_to_template(
            "",
            CompileOptions {
                default_duration_beats: 16.0,
                template_id: Some("user.x".into()),
                template_name: Some("X".into()),
            },
        )
        .unwrap();
        assert_eq!(t.duration_beats, 16.0);
        assert_eq!(t.tracks.len(), 0);
        assert_eq!(t.source.as_deref(), Some(""));
    }

    #[test]
    fn add_keyframe_basic() {
        let src = r#"
            set_duration(32)
            add_keyframe("crossfader", 0, -1, "linear")
            add_keyframe("crossfader", 32, 1)
        "#;
        let t =
            compile_lua_to_template(src, CompileOptions::default()).unwrap();
        assert_eq!(t.duration_beats, 32.0);
        assert_eq!(t.tracks.len(), 1);
        let track = &t.tracks[0];
        assert!(matches!(track.target, BuiltInTarget::Crossfader));
        assert_eq!(track.keyframes.len(), 2);
        assert!(matches!(track.keyframes[0].curve, CurveType::Linear));
    }

    #[test]
    fn add_keyframe_deck_target() {
        let src = r#"
            add_keyframe("deck_eq_low.A", 0, 0, "hold")
            add_keyframe("deck_eq_low.A", 8, -26, "ease_in")
        "#;
        let t = compile_lua_to_template(src, CompileOptions::default()).unwrap();
        assert_eq!(t.tracks.len(), 1);
        assert!(matches!(
            t.tracks[0].target,
            BuiltInTarget::DeckEqLow { deck: DeckSlot::A }
        ));
        assert_eq!(t.tracks[0].keyframes.len(), 2);
    }

    #[test]
    fn add_track_with_table_named_keys() {
        let src = r#"
            add_track("crossfader", {
              {beat = 0, value = -1, curve = "linear"},
              {beat = 16, value = 1, curve = "ease_out"},
            })
        "#;
        let t = compile_lua_to_template(src, CompileOptions::default()).unwrap();
        assert_eq!(t.tracks.len(), 1);
        assert_eq!(t.tracks[0].keyframes.len(), 2);
    }

    #[test]
    fn keyframes_sorted_by_beat() {
        let src = r#"
            add_keyframe("crossfader", 16, 0)
            add_keyframe("crossfader", 0, -1)
            add_keyframe("crossfader", 8, -0.5)
        "#;
        let t = compile_lua_to_template(src, CompileOptions::default()).unwrap();
        let kf = &t.tracks[0].keyframes;
        let positions: Vec<f64> = kf
            .iter()
            .map(|k| match k.position {
                TimePosition::Beats(b) => b,
                _ => -1.0,
            })
            .collect();
        assert_eq!(positions, vec![0.0, 8.0, 16.0]);
    }

    #[test]
    fn loop_generates_many_keyframes() {
        let src = r#"
            for i = 0, 8 do
              add_keyframe("deck_filter.A", i, math.sin(i) * 0.5)
            end
        "#;
        let t = compile_lua_to_template(src, CompileOptions::default()).unwrap();
        assert_eq!(t.tracks.len(), 1);
        assert_eq!(t.tracks[0].keyframes.len(), 9);
    }

    #[test]
    fn invalid_target_returns_error() {
        let src = r#"add_keyframe("nope", 0, 0)"#;
        let err =
            compile_lua_to_template(src, CompileOptions::default()).unwrap_err();
        assert!(matches!(err, ScriptError::Lua(_)));
    }

    #[test]
    fn invalid_curve_returns_error() {
        let src = r#"add_keyframe("crossfader", 0, 0, "wobble")"#;
        let err =
            compile_lua_to_template(src, CompileOptions::default()).unwrap_err();
        assert!(matches!(err, ScriptError::Lua(_)));
    }

    /// io/os が無効化されているサンドボックス確認。
    #[test]
    fn io_and_os_are_disabled() {
        // io.open は Sandbox により nil。
        let err = compile_lua_to_template(
            r#"io.open("/etc/passwd", "r")"#,
            CompileOptions::default(),
        )
        .unwrap_err();
        assert!(matches!(err, ScriptError::Lua(_)));
        let err = compile_lua_to_template(
            r#"os.execute("ls")"#,
            CompileOptions::default(),
        )
        .unwrap_err();
        assert!(matches!(err, ScriptError::Lua(_)));
    }

    #[test]
    fn math_is_available() {
        let src = r#"
            for i = 0, 4 do
              add_keyframe("master_volume", i, math.cos(i * math.pi / 4))
            end
        "#;
        let t = compile_lua_to_template(src, CompileOptions::default()).unwrap();
        assert_eq!(t.tracks[0].keyframes.len(), 5);
    }

    #[test]
    fn syntax_error_propagates() {
        let err = compile_lua_to_template(
            r#"add_keyframe("crossfader", 0, 0"#,
            CompileOptions::default(),
        )
        .unwrap_err();
        assert!(matches!(err, ScriptError::Lua(_)));
    }
}
