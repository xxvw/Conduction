use super::*;
use crate::audio_engine::parse_deck;
use conduction_core::{SetlistId, Track, TrackId};

impl PerformanceHandle {
    pub async fn perform(&self, action: ControllerAction) -> anyhow::Result<()> {
        let snapshot = self.inner.audio.snapshot();
        let get_deck = |name: &str| -> anyhow::Result<_> {
            let id = parse_deck(name).map_err(anyhow::Error::msg)?;
            Ok((
                id,
                if id == DeckId::A {
                    &snapshot.deck_a
                } else {
                    &snapshot.deck_b
                },
            ))
        };
        let command = match action {
            ControllerAction::PlayPause { deck } => {
                let (id, state) = get_deck(&deck)?;
                if state.cue_pressed {
                    AudioCommand::Play(id)
                } else if state.state == "play" {
                    AudioCommand::Pause(id)
                } else {
                    AudioCommand::Play(id)
                }
            }
            ControllerAction::Cue { deck, pressed } => {
                let (id, _) = get_deck(&deck)?;
                if pressed {
                    AudioCommand::CuePress(id)
                } else {
                    AudioCommand::CueRelease(id)
                }
            }
            ControllerAction::JogTouch { deck, touched } => AudioCommand::JogTouch {
                deck: get_deck(&deck)?.0,
                touched,
            },
            ControllerAction::Jog { deck, delta } => {
                anyhow::ensure!(
                    delta.is_finite() && delta.abs() <= 10000.0,
                    "Invalid jog movement"
                );
                AudioCommand::Jog {
                    deck: get_deck(&deck)?.0,
                    delta_sec: f64::from(delta) * 0.002,
                }
            }
            ControllerAction::Nudge { deck, amount } => {
                anyhow::ensure!(
                    amount.is_finite() && (-1.0..=1.0).contains(&amount),
                    "Nudge must be between -1 and 1"
                );
                AudioCommand::Nudge {
                    deck: get_deck(&deck)?.0,
                    value: amount,
                }
            }
            ControllerAction::Tempo { deck, value } => AudioCommand::SetTempoAdjust {
                deck: get_deck(&deck)?.0,
                adjust: normalized(value)? * 2.0 - 1.0,
            },
            ControllerAction::Sync { deck } => {
                let (id, state) = get_deck(&deck)?;
                let use_link = self.inner.link.lock().as_ref().is_some_and(|l| {
                    let s = l.snapshot();
                    s.running && s.master_number.is_some() && s.master_number != s.player_number
                });
                AudioCommand::SetSync {
                    deck: id,
                    enabled: !state.sync_enabled,
                    source: if use_link {
                        "link".into()
                    } else {
                        "local".into()
                    },
                }
            }
            ControllerAction::Eq { deck, band, value } => {
                let id = get_deck(&deck)?.0;
                let db = eq_db(normalized(value)?);
                match band.as_str() {
                    "low" => AudioCommand::SetEqLow { deck: id, db },
                    "mid" => AudioCommand::SetEqMid { deck: id, db },
                    "high" => AudioCommand::SetEqHigh { deck: id, db },
                    _ => anyhow::bail!("Unknown EQ band"),
                }
            }
            ControllerAction::Filter { deck, value } => AudioCommand::SetFilter {
                deck: get_deck(&deck)?.0,
                value: normalized(value)? * 2.0 - 1.0,
            },
            ControllerAction::Fader { deck, value } => AudioCommand::SetChannelVolume {
                deck: get_deck(&deck)?.0,
                volume: normalized(value)? * 2.0,
            },
            ControllerAction::Master { value } => {
                AudioCommand::SetMasterVolume(normalized(value)? * 2.0)
            }
            ControllerAction::Crossfader { value } => {
                AudioCommand::SetCrossfader(normalized(value)? * 2.0 - 1.0)
            }
            ControllerAction::HeadphoneVolume { value } => {
                AudioCommand::SetHeadphoneVolume(normalized(value)? * 2.0)
            }
            ControllerAction::HeadphoneMix { value } => {
                AudioCommand::SetHeadphoneMix(normalized(value)?)
            }
            ControllerAction::HeadphoneCue { deck } => {
                let (id, state) = get_deck(&deck)?;
                anyhow::ensure!(state.has_cue_output, "Configure a headphone output first");
                AudioCommand::SetCueSend {
                    deck: id,
                    value: if state.cue_send > 0.5 { 0.0 } else { 1.0 },
                }
            }
            ControllerAction::HotCue {
                deck,
                slot,
                operation,
            } => {
                let (id, state) = get_deck(&deck)?;
                anyhow::ensure!((1..=8).contains(&slot), "Hot Cue slot must be 1–8");
                let exists = state
                    .hot_cues
                    .get(usize::from(slot - 1))
                    .copied()
                    .flatten()
                    .is_some();
                let set = match operation.as_str() {
                    "trigger_or_set" => !exists,
                    "set" => true,
                    "clear" => false,
                    _ => anyhow::bail!("Unknown Hot Cue operation"),
                };
                if operation == "clear" || set {
                    let track_id = state.track_id.as_ref().ok_or_else(|| {
                        anyhow::anyhow!("Import this track into the library to save Hot Cues")
                    })?;
                    let track_id = TrackId::from_uuid(uuid::Uuid::parse_str(track_id)?);
                    let path = state
                        .loaded_path
                        .as_deref()
                        .ok_or_else(|| anyhow::anyhow!("No track loaded"))?;
                    // Serialize metadata writes and their enqueue with the bridge's
                    // refresh. Wait for the host only after releasing the DB guard.
                    let applied = self
                        .inner
                        .library
                        .with_library(|lib| -> anyhow::Result<_> {
                            if operation == "clear" {
                                lib.delete_hot_cue(track_id, slot)?;
                            } else {
                                lib.set_hot_cue(track_id, slot, state.position_sec)?;
                            }
                            let metadata = metadata_from_library(lib, Path::new(path))?;
                            Ok(self
                                .inner
                                .audio
                                .execute(AudioCommand::RefreshTrackMetadata {
                                    deck: id,
                                    path: PathBuf::from(path),
                                    load_generation: state.load_generation,
                                    metadata,
                                }))
                        })?;
                    return applied.await;
                } else {
                    AudioCommand::HotCue {
                        deck: id,
                        slot: slot - 1,
                        set: false,
                    }
                }
            }
            ControllerAction::Loop { deck, operation } => {
                let (id, state) = get_deck(&deck)?;
                let snap_position =
                    self.snap_position(state.track_id.as_deref(), state.position_sec);
                match operation.as_str() {
                    "in" => AudioCommand::LoopIn {
                        deck: id,
                        position_sec: snap_position,
                    },
                    "out" => AudioCommand::LoopOut {
                        deck: id,
                        position_sec: snap_position,
                    },
                    "toggle" => AudioCommand::LoopToggle(id),
                    "exit" => {
                        if !state.loop_active {
                            return Ok(());
                        }
                        AudioCommand::LoopToggle(id)
                    }
                    "reloop" => {
                        if state.loop_active {
                            return Ok(());
                        }
                        AudioCommand::LoopToggle(id)
                    }
                    "halve" | "double" => {
                        let start = state
                            .loop_start_sec
                            .ok_or_else(|| anyhow::anyhow!("Set Loop IN first"))?;
                        let end = state
                            .loop_end_sec
                            .ok_or_else(|| anyhow::anyhow!("Set Loop OUT first"))?;
                        let factor = if operation == "halve" { 0.5 } else { 2.0 };
                        let length = (end - start) * factor;
                        anyhow::ensure!(length >= 0.02, "Loop is too short");
                        let new_end =
                            (start + length).min(state.duration_sec.unwrap_or(start + length));
                        AudioCommand::LoopOut {
                            deck: id,
                            position_sec: new_end,
                        }
                    }
                    _ => anyhow::bail!("Unknown loop operation"),
                }
            }
            ControllerAction::Browse { delta } => {
                let mut browser = self.inner.browser.lock();
                let tracks = self.browser_tracks(&browser)?;
                if tracks.is_empty() {
                    browser.selected_track_id = None;
                    return Ok(());
                }
                let current = tracks
                    .iter()
                    .position(|t| browser.selected_track_id.as_deref() == Some(&t.id.to_string()));
                let next = browse_index(current, delta, tracks.len());
                browser.selected_track_id = Some(tracks[next].id.to_string());
                return Ok(());
            }
            ControllerAction::LoadSelected { deck } => self.selected_load_command(&deck)?,
            ControllerAction::Fx {
                deck,
                parameter,
                value,
            } => {
                let (id, state) = get_deck(&deck)?;
                let value = normalized(value)?;
                let index = usize::from(id == DeckId::B);
                let wet = {
                    let mut selections = self.inner.fx.lock();
                    let selection = &mut selections[index];
                    match parameter.as_str() {
                        "select:echo" => selection.reverb = false,
                        "select:reverb" => selection.reverb = true,
                        "select" => selection.reverb = value >= 0.5,
                        "select_next" | "select_previous" => selection.reverb = !selection.reverb,
                        "mix" => selection.mix = value,
                        "enabled" => selection.enabled = value > 0.0,
                        _ => {}
                    }
                    (
                        selection.reverb,
                        if selection.enabled {
                            selection.mix
                        } else {
                            0.0
                        },
                    )
                };
                match parameter.as_str() {
                    "echo_wet" => AudioCommand::SetEcho {
                        deck: id,
                        wet: value,
                        time_ms: state.echo_time_ms,
                        feedback: state.echo_feedback,
                    },
                    "echo_time_ms" => AudioCommand::SetEcho {
                        deck: id,
                        wet: state.echo_wet,
                        time_ms: 1.0 + value * 1999.0,
                        feedback: state.echo_feedback,
                    },
                    "echo_feedback" => AudioCommand::SetEcho {
                        deck: id,
                        wet: state.echo_wet,
                        time_ms: state.echo_time_ms,
                        feedback: value * 0.95,
                    },
                    "reverb_wet" => AudioCommand::SetReverb {
                        deck: id,
                        wet: value,
                        room: state.reverb_room,
                    },
                    "reverb_room" => AudioCommand::SetReverb {
                        deck: id,
                        wet: state.reverb_wet,
                        room: value,
                    },
                    "mix" | "enabled" | "select" | "select:echo" | "select:reverb"
                    | "select_next" | "select_previous" => {
                        if wet.0 {
                            self.inner
                                .audio
                                .execute(AudioCommand::SetEcho {
                                    deck: id,
                                    wet: 0.0,
                                    time_ms: state.echo_time_ms,
                                    feedback: state.echo_feedback,
                                })
                                .await?;
                            AudioCommand::SetReverb {
                                deck: id,
                                wet: wet.1,
                                room: state.reverb_room,
                            }
                        } else {
                            self.inner
                                .audio
                                .execute(AudioCommand::SetReverb {
                                    deck: id,
                                    wet: 0.0,
                                    room: state.reverb_room,
                                })
                                .await?;
                            AudioCommand::SetEcho {
                                deck: id,
                                wet: wet.1,
                                time_ms: state.echo_time_ms,
                                feedback: state.echo_feedback,
                            }
                        }
                    }
                    "beat_halve" | "beat_double" => {
                        let multiplier = if parameter == "beat_halve" { 0.5 } else { 2.0 };
                        AudioCommand::SetEcho {
                            deck: id,
                            wet: state.echo_wet,
                            time_ms: (state.echo_time_ms * multiplier).clamp(1.0, 2000.0),
                            feedback: state.echo_feedback,
                        }
                    }
                    _ => anyhow::bail!("Unknown effect control: {parameter}"),
                }
            }
        };
        self.inner.audio.execute(command).await
    }

    pub(super) fn selected_load_command(&self, deck: &str) -> anyhow::Result<AudioCommand> {
        let id = parse_deck(deck).map_err(anyhow::Error::msg)?;
        let snapshot = self.inner.audio.snapshot();
        let state = if id == DeckId::A {
            &snapshot.deck_a
        } else {
            &snapshot.deck_b
        };
        anyhow::ensure!(
            state.state != "play",
            "Pause the deck before loading another track"
        );
        let browser = self.inner.browser.lock().clone();
        let tracks = self.browser_tracks(&browser)?;
        let track = tracks
            .iter()
            .find(|t| browser.selected_track_id.as_deref() == Some(&t.id.to_string()))
            .or_else(|| tracks.first())
            .ok_or_else(|| anyhow::anyhow!("No matching tracks"))?;
        let path = track.path.clone();
        let metadata = track_metadata(&self.inner.library, &path)?;
        Ok(AudioCommand::LoadWithMetadata {
            deck: id,
            path,
            metadata,
        })
    }

    fn browser_tracks(&self, browser: &PerformanceBrowser) -> anyhow::Result<Vec<Track>> {
        self.inner
            .library
            .with_library(|library| -> anyhow::Result<_> {
                let mut tracks = library.list_tracks()?;
                if let Some(playlist) = &browser.playlist_id {
                    let id = SetlistId::from_uuid(uuid::Uuid::parse_str(playlist)?);
                    let list = library
                        .get_setlist(id)?
                        .ok_or_else(|| anyhow::anyhow!("Setlist no longer exists"))?;
                    tracks = list
                        .entries
                        .iter()
                        .filter_map(|entry| tracks.iter().find(|t| t.id == entry.track_id).cloned())
                        .collect();
                }
                let query = browser.query.trim().to_lowercase();
                if !query.is_empty() {
                    tracks.retain(|t| {
                        format!("{} {} {}", t.title, t.artist, t.album)
                            .to_lowercase()
                            .contains(&query)
                    });
                }
                Ok(tracks)
            })
    }

    fn snap_position(&self, track_id: Option<&str>, position: f64) -> f64 {
        let Some(id) = track_id.and_then(|id| uuid::Uuid::parse_str(id).ok()) else {
            return position;
        };
        self.inner.library.with_library(|lib| {
            lib.load_beatgrid(TrackId::from_uuid(id))
                .ok()
                .and_then(|beats| {
                    beats
                        .iter()
                        .min_by(|a, b| {
                            (a.position_sec - position)
                                .abs()
                                .total_cmp(&(b.position_sec - position).abs())
                        })
                        .map(|b| b.position_sec)
                })
                .unwrap_or(position)
        })
    }
}

fn normalized(value: f32) -> anyhow::Result<f32> {
    anyhow::ensure!(
        value.is_finite() && (0.0..=1.0).contains(&value),
        "Control value must be between 0 and 1"
    );
    Ok(value)
}
fn eq_db(value: f32) -> f32 {
    if value <= 0.5 {
        (value - 0.5) * 48.0
    } else {
        (value - 0.5) * 24.0
    }
}
fn browse_index(current: Option<usize>, delta: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let position = match current {
        Some(index) => index as i64 + i64::from(delta),
        None => {
            if delta < 0 {
                len as i64 - 1
            } else {
                0
            }
        }
    };
    position.clamp(0, len as i64 - 1) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eq_center_is_unity() {
        assert_eq!(eq_db(0.0), -24.0);
        assert_eq!(eq_db(0.5), 0.0);
        assert_eq!(eq_db(1.0), 12.0);
    }
    #[test]
    fn invalid_numeric_controls_are_rejected() {
        for v in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert!(normalized(v).is_err());
        }
    }
    #[test]
    fn library_navigation_is_bounded_and_starts_at_first_match() {
        assert_eq!(browse_index(None, 1, 5), 0);
        assert_eq!(browse_index(Some(0), -10, 5), 0);
        assert_eq!(browse_index(Some(1), i32::MAX, 5), 4);
        assert_eq!(browse_index(Some(4), -1, 5), 3);
    }
}
