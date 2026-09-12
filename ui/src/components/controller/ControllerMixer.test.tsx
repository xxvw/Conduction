import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ipc } from "@/lib/ipc";
import type { DeckId, DeckSnapshot, MixerSnapshot } from "@/types/mixer";
import type { ControllerAction } from "@/types/performance";
import { ControllerMixer } from "./ControllerMixer";

vi.mock("@/lib/ipc", () => ({ ipc: { overrideParam: vi.fn(), resumeParam: vi.fn() } }));

function deck(id: DeckId): DeckSnapshot {
  return {
    id, state: "play", loaded_path: `/music/${id}.wav`, track_id: id,
    channel_volume: 1, effective_volume: 0.5, tempo_range_percent: 6,
    tempo_adjust: 0, playback_speed: 1, position_sec: 30, duration_sec: 240,
    loop_start_sec: null, loop_end_sec: null, loop_active: false,
    eq_low_db: 0, eq_mid_db: 0, eq_high_db: 0, filter: 0,
    echo_wet: 0, echo_time_ms: 250, echo_feedback: 0.3, reverb_wet: 0, reverb_room: 0.5,
    cue_send: 0, has_cue_output: true, key_lock: false, pitch_offset_semitones: 0,
    loading: false, load_generation: 1, load_error: null, bpm: 120, original_bpm: 120,
    beat_position: 60, beat_phase: 0, hot_cues: Array<number | null>(8).fill(null),
    transport_cue_sec: 0, cue_pressed: false, jog_touched: false, nudge: 0,
    sync_enabled: false, sync_source: null, sync_lost: false,
  };
}

function snapshot(): MixerSnapshot {
  return {
    crossfader: 0, master_volume: 1, deck_a: deck("A"), deck_b: deck("B"), template: null,
    headphone_mix: 0, headphone_volume: 1, output_error: null,
    audio_config: {
      device_name: "DJ interface", cue_device_name: null, sample_rate: 48000, mode: "internal",
      main_pair: 0, cue_pair: 2, deck_a_pair: 0, deck_b_pair: 2, headphone_mix: 0, headphone_volume: 1,
    },
    audio: {
      sample_rate: 48000, output_channels: 4, elapsed_frames: 1440000, underruns: 0,
      device_lost: false, error: null, estimated_latency_ms: 10,
      peak_main: [0.5, 0.25], peak_cue: [0, 0.1], peak_decks: [[1, 0.5], [0.01, 0]],
    },
  };
}

const slider = (name: string) => screen.getByRole<HTMLInputElement>("slider", { name });

beforeEach(() => {
  vi.mocked(ipc.overrideParam).mockReset().mockResolvedValue(undefined);
  vi.mocked(ipc.resumeParam).mockReset().mockResolvedValue(undefined);
});
afterEach(cleanup);

describe("ControllerMixer", () => {
  it("renders actual audio peaks even when channel gain is unchanged", () => {
    const status = snapshot();
    const dispatch = vi.fn(async (_action: ControllerAction) => {});
    const view = render(<ControllerMixer status={status} external={false} dispatch={dispatch} onError={vi.fn()} />);

    expect(screen.getByRole("img", { name: "Main output: -6.0 dBFS, -12.0 dBFS" })).toBeTruthy();
    expect(screen.getByRole("img", { name: "Headphone output: silent, -20.0 dBFS" })).toBeTruthy();
    expect(screen.getByRole("img", { name: "Deck A audio: 0.0 dBFS, -6.0 dBFS" })).toBeTruthy();
    expect(screen.getByRole("img", { name: "Deck B audio: -40.0 dBFS, silent" })).toBeTruthy();

    status.audio.peak_main = [0, 0];
    view.rerender(<ControllerMixer status={status} external={false} dispatch={dispatch} onError={vi.fn()} />);
    expect(screen.getByRole("img", { name: "Main output: silent, silent" })).toBeTruthy();
    expect(slider("MASTER").value).toBe("100");
  });

  it("bypasses software mixing in External mode while keeping deck FX usable", async () => {
    const dispatch = vi.fn(async (_action: ControllerAction) => {});
    render(<ControllerMixer status={snapshot()} external dispatch={dispatch} onError={vi.fn()} />);

    for (const name of ["A HI", "A MID", "A LOW", "A LPF ↔ HPF", "A LEVEL", "B HI", "B MID", "B LOW", "B LPF ↔ HPF", "B LEVEL", "CROSSFADER A ↔ B", "MASTER"]) {
      expect(slider(name).disabled, name).toBe(true);
    }
    expect(slider("A FX WET").disabled).toBe(false);
    expect(slider("B FX WET").disabled).toBe(false);
    expect(slider("HEADPHONE LEVEL").disabled).toBe(false);

    fireEvent.change(screen.getByRole("combobox", { name: "Deck B effect" }), { target: { value: "reverb_wet" } });
    fireEvent.change(slider("B FX WET"), { target: { value: "60" } });
    await waitFor(() => expect(dispatch).toHaveBeenCalledWith({ action: "fx", deck: "B", parameter: "reverb_wet", value: 0.6 }));
  });

  it("sends the shared normalized controller contract, preserving EQ unity at midpoint", async () => {
    const dispatch = vi.fn(async (_action: ControllerAction) => {});
    render(<ControllerMixer status={snapshot()} external={false} dispatch={dispatch} onError={vi.fn()} />);
    const high = slider("A HI");

    fireEvent.change(high, { target: { value: "-24" } });
    fireEvent.change(high, { target: { value: "12" } });
    fireEvent.change(high, { target: { value: "-12" } });
    fireEvent.change(high, { target: { value: "6" } });
    // A controlled slider stays at the confirmed value until a new snapshot arrives.
    expect(high.value).toBe("0");

    const nonUnity = snapshot();
    nonUnity.deck_a.eq_high_db = 6;
    cleanup();
    render(<ControllerMixer status={nonUnity} external={false} dispatch={dispatch} onError={vi.fn()} />);
    fireEvent.change(slider("A HI"), { target: { value: "0" } });
    fireEvent.change(slider("A LEVEL"), { target: { value: "200" } });
    fireEvent.change(slider("MASTER"), { target: { value: "0" } });
    fireEvent.change(slider("CROSSFADER A ↔ B"), { target: { value: "-1" } });
    fireEvent.change(slider("B LPF ↔ HPF"), { target: { value: "1" } });
    fireEvent.change(slider("HEADPHONE LEVEL"), { target: { value: "150" } });

    await waitFor(() => expect(dispatch.mock.calls).toEqual([
      [{ action: "eq", deck: "A", band: "high", value: 0 }],
      [{ action: "eq", deck: "A", band: "high", value: 1 }],
      [{ action: "eq", deck: "A", band: "high", value: 0.25 }],
      [{ action: "eq", deck: "A", band: "high", value: 0.75 }],
      [{ action: "eq", deck: "A", band: "high", value: 0.5 }],
      [{ action: "fader", deck: "A", value: 1 }],
      [{ action: "master", value: 0 }],
      [{ action: "crossfader", value: 0 }],
      [{ action: "filter", deck: "B", value: 1 }],
      [{ action: "headphone_volume", value: 0.75 }],
    ]));
  });

  it("keeps headphone Cue at confirmed state and reports controller errors", async () => {
    const dispatch = vi.fn(async (_action: ControllerAction) => { throw new Error("Output lost"); });
    const onError = vi.fn();
    render(<ControllerMixer status={snapshot()} external={false} dispatch={dispatch} onError={onError} />);
    const cue = screen.getByRole("button", { name: "Deck A headphone cue" });
    fireEvent.click(cue);

    await waitFor(() => expect(onError).toHaveBeenCalledWith("Error: Output lost"));
    expect(dispatch).toHaveBeenCalledWith({ action: "headphone_cue", deck: "A" });
    expect(cue.getAttribute("aria-pressed")).toBe("false");
  });

  it("overrides active automation, resumes manual parameters and reports individual failures", async () => {
    const status = snapshot();
    status.template = {
      id: "transition", name: "Long blend", progress: 0.5, elapsed_beats: 8, duration_beats: 16,
      beats_remaining: 8, override_count: 1,
      automation_modes: [
        { target_key: "crossfader", mode: "automated" },
        { target_key: "deck_eq_low.A", mode: "resuming" },
        { target_key: "deck_echo_wet.B", mode: "overridden" },
        { target_key: "deck_filter.B", mode: "idle" },
      ],
    };
    vi.mocked(ipc.overrideParam).mockImplementation(async (key) => {
      if (key === "deck_eq_low.A") throw new Error("Template ended");
    });
    const onError = vi.fn();
    render(<ControllerMixer status={status} external={false} dispatch={vi.fn(async () => {})} onError={onError} />);

    expect(screen.getByRole<HTMLProgressElement>("progressbar", { name: "Template progress" }).value).toBe(0.5);
    fireEvent.click(screen.getByRole("button", { name: "Override" }));
    await waitFor(() => expect(onError).toHaveBeenCalledWith("deck_eq_low.A: Error: Template ended"));
    expect(vi.mocked(ipc.overrideParam).mock.calls).toEqual([["crossfader"], ["deck_eq_low.A"]]);
    fireEvent.click(screen.getByRole("button", { name: "Resume · 4 beats" }));
    await waitFor(() => expect(ipc.resumeParam).toHaveBeenCalledWith("deck_echo_wet.B", 4));
  });
});
