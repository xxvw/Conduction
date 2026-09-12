import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { FxPad } from "@/components/fx/FxPad";
import { ipc } from "@/lib/ipc";
import type { DeckSnapshot } from "@/types/mixer";

vi.mock("@/lib/ipc", () => ({ ipc: {
  setEq: vi.fn(async () => {}), setFilter: vi.fn(async () => {}),
  setEcho: vi.fn(async () => {}), setReverb: vi.fn(async () => {}),
} }));

function snapshot(): DeckSnapshot {
  return {
    id: "A", state: "play", loaded_path: "/music/test.wav", track_id: "track-a",
    channel_volume: 1, effective_volume: 0.5, tempo_range_percent: 6,
    tempo_adjust: 0, playback_speed: 1, position_sec: 30, duration_sec: 240,
    loop_start_sec: null, loop_end_sec: null, loop_active: false,
    eq_low_db: 0, eq_mid_db: 0, eq_high_db: 0, filter: 0.2,
    echo_wet: 0.1, echo_time_ms: 250, echo_feedback: 0.3, reverb_wet: 0.2, reverb_room: 0.5,
    cue_send: 0, has_cue_output: true, key_lock: false, pitch_offset_semitones: 0,
    loading: false, load_generation: 1, load_error: null, bpm: 120, original_bpm: 120,
    beat_position: 60, beat_phase: 0, hot_cues: Array<number | null>(8).fill(null),
    transport_cue_sec: 0, cue_pressed: false, jog_touched: false, nudge: 0,
    sync_enabled: false, sync_source: null, sync_lost: false,
  };
}

const slider = (name: string) => screen.getByRole<HTMLInputElement>("slider", { name });
beforeEach(() => vi.clearAllMocks());
afterEach(cleanup);

describe("Legacy FxPad External mixer compatibility", () => {
  it("disables EQ/filter and their reset/kill controls while keeping echo and reverb active", () => {
    render(<FxPad deckId="A" snapshot={snapshot()} mixerStatus={null} focusedTarget="" onFocus={vi.fn()} externalMixer />);

    for (const band of ["high", "mid", "low"]) {
      expect(slider(`Deck A EQ ${band}`).disabled).toBe(true);
      const kill = screen.getByRole<HTMLButtonElement>("button", { name: `Deck A EQ ${band} kill` });
      expect(kill.disabled).toBe(true);
      fireEvent.click(kill);
    }
    expect(slider("Deck A filter").disabled).toBe(true);
    const reset = screen.getByRole<HTMLButtonElement>("button", { name: "Deck A reset filter" });
    expect(reset.disabled).toBe(true);
    fireEvent.click(reset);
    expect(ipc.setEq).not.toHaveBeenCalled();
    expect(ipc.setFilter).not.toHaveBeenCalled();
    expect(screen.getByText("EQ · EXTERNAL BYPASS")).toBeTruthy();
    expect(screen.getByText("FILTER · EXTERNAL BYPASS")).toBeTruthy();

    for (const effect of ["echo wet", "echo time", "echo feedback", "reverb wet", "reverb room"]) {
      expect(slider(`Deck A ${effect}`).disabled).toBe(false);
    }
    fireEvent.change(slider("Deck A echo wet"), { target: { value: "0.65" } });
    fireEvent.change(slider("Deck A reverb room"), { target: { value: "0.75" } });
    expect(ipc.setEcho).toHaveBeenCalledWith("A", 0.65, 250, 0.3);
    expect(ipc.setReverb).toHaveBeenCalledWith("A", 0.2, 0.75);
  });

  it("defaults to enabled Internal controls and preserves legacy IPC units", () => {
    const status = snapshot();
    const view = render(<FxPad deckId="A" snapshot={status} mixerStatus={null} focusedTarget="" onFocus={vi.fn()} />);
    for (const control of screen.getAllByRole<HTMLInputElement>("slider")) expect(control.disabled).toBe(false);
    for (const control of screen.getAllByRole<HTMLButtonElement>("button")) expect(control.disabled).toBe(false);

    fireEvent.change(slider("Deck A EQ high"), { target: { value: "-12" } });
    fireEvent.click(screen.getByRole("button", { name: "Deck A EQ low kill" }));
    fireEvent.change(slider("Deck A filter"), { target: { value: "-0.5" } });
    fireEvent.click(screen.getByRole("button", { name: "Deck A reset filter" }));
    fireEvent.change(slider("Deck A echo time"), { target: { value: "450" } });
    expect(vi.mocked(ipc.setEq).mock.calls).toEqual([["A", "high", -12], ["A", "low", -40]]);
    expect(vi.mocked(ipc.setFilter).mock.calls).toEqual([["A", -0.5], ["A", 0]]);
    expect(ipc.setEcho).toHaveBeenCalledWith("A", 0.1, 450, 0.3);

    view.rerender(<FxPad deckId="A" snapshot={{ ...status, eq_low_db: -40 }} mixerStatus={null} focusedTarget="" onFocus={vi.fn()} />);
    const lowKill = screen.getByRole("button", { name: "Deck A EQ low kill" });
    expect(lowKill.getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(lowKill);
    expect(ipc.setEq).toHaveBeenLastCalledWith("A", "low", 0);
  });
});
