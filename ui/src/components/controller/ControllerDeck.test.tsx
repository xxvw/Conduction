import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { DeckSnapshot } from "@/types/mixer";
import type { ControllerAction } from "@/types/performance";
import { ControllerDeck } from "./ControllerDeck";

vi.mock("@/hooks/useBeats", () => ({ useBeats: () => [
  { position_sec: 0, is_downbeat: true }, { position_sec: 0.5, is_downbeat: false },
] }));
vi.mock("@/hooks/useWaveform", () => ({ useWaveform: () => null }));
vi.mock("@/hooks/useInterpolatedPosition", () => ({ useInterpolatedPosition: (deck: DeckSnapshot) => deck.position_sec }));
vi.mock("@/components/waveform/WaveformView", () => ({ WaveformView: () => <div aria-label="Overview waveform" /> }));
vi.mock("@/components/waveform/WaveformZoomView", () => ({ WaveformZoomView: () => <div aria-label="Zoom waveform" /> }));
vi.mock("@/lib/ipc", () => ({ ipc: { seek: vi.fn(), setTempoRange: vi.fn(), setKeyLock: vi.fn() } }));

function snapshot(): DeckSnapshot {
  return {
    id: "A", state: "play", loaded_path: "/music/test.wav", track_id: "track-a",
    channel_volume: 1, effective_volume: 0.5, tempo_range_percent: 6,
    tempo_adjust: 0, playback_speed: 1, position_sec: 30, duration_sec: 240,
    loop_start_sec: 10, loop_end_sec: 14, loop_active: true,
    eq_low_db: 0, eq_mid_db: 0, eq_high_db: 0, filter: 0,
    echo_wet: 0, echo_time_ms: 250, echo_feedback: 0.3, reverb_wet: 0, reverb_room: 0.5,
    cue_send: 0, has_cue_output: true, key_lock: false, pitch_offset_semitones: 0,
    loading: false, load_generation: 1, load_error: null, bpm: 120, original_bpm: 120,
    beat_position: 60, beat_phase: 0, hot_cues: [10, null, null, null, null, null, null, null],
    transport_cue_sec: 0, cue_pressed: false, jog_touched: false, nudge: 0,
    sync_enabled: false, sync_source: null, sync_lost: false,
  };
}

let animationFrames: Map<number, FrameRequestCallback>;
beforeEach(() => {
  class TestPointerEvent extends MouseEvent {
    readonly pointerId: number;
    constructor(type: string, init: MouseEventInit & { pointerId?: number } = {}) {
      super(type, init);
      this.pointerId = init.pointerId ?? 1;
    }
  }
  vi.stubGlobal("PointerEvent", TestPointerEvent);
  Object.defineProperty(HTMLElement.prototype, "setPointerCapture", { configurable: true, value: vi.fn() });
  animationFrames = new Map();
  let frameId = 0;
  vi.stubGlobal("requestAnimationFrame", vi.fn((callback: FrameRequestCallback) => {
    animationFrames.set(++frameId, callback);
    return frameId;
  }));
  vi.stubGlobal("cancelAnimationFrame", vi.fn((id: number) => { animationFrames.delete(id); }));
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("ControllerDeck performance controls", () => {
  it.each(["pointer cancel", "unmount"] as const)(
    "flushes accumulated jog movement before releasing touch on %s",
    async (releaseCause) => {
      let finishTouch!: () => void;
      const touched = new Promise<void>((resolve) => { finishTouch = resolve; });
      const dispatch = vi.fn((action: ControllerAction) =>
        action.action === "jog_touch" && action.touched ? touched : Promise.resolve(),
      );
      const onError = vi.fn();
      const onSelectDeck = vi.fn();
      const view = render(<ControllerDeck snapshot={snapshot()} track={null} dispatch={dispatch} onError={onError} onSelectDeck={onSelectDeck} />);
      const wheel = screen.getByRole("button", { name: /Deck A jog wheel/ });
      vi.spyOn(wheel, "getBoundingClientRect").mockReturnValue(new DOMRect(0, 0, 100, 100));

      fireEvent.focus(wheel);
      expect(onSelectDeck).toHaveBeenCalledWith("A");
      onSelectDeck.mockClear();
      fireEvent.pointerDown(wheel, { button: 0, pointerId: 1, clientX: 100, clientY: 50 });
      expect(onSelectDeck).toHaveBeenCalledWith("A");
      await waitFor(() => expect(dispatch.mock.calls).toEqual([[{ action: "jog_touch", deck: "A", touched: true }]]));

      // Two quarter turns arrive before the animation frame and before touch is acknowledged.
      fireEvent.pointerMove(wheel, { pointerId: 1, clientX: 50, clientY: 100 });
      fireEvent.pointerMove(wheel, { pointerId: 1, clientX: 0, clientY: 50 });
      expect(animationFrames.size).toBe(1);
      const pendingFrame = [...animationFrames.values()][0]!;
      if (releaseCause === "pointer cancel") fireEvent.pointerCancel(wheel, { pointerId: 1 });
      else view.unmount();

      expect(animationFrames.size).toBe(0);
      expect(dispatch).toHaveBeenCalledTimes(1);
      await act(async () => { finishTouch(); });
      await waitFor(() => expect(dispatch.mock.calls).toEqual([
        [{ action: "jog_touch", deck: "A", touched: true }],
        [{ action: "jog", deck: "A", delta: 450 }],
        [{ action: "jog_touch", deck: "A", touched: false }],
      ]));

      // A late frame or duplicate release must never scratch after touch is released.
      await act(async () => { pendingFrame(16); });
      fireEvent.pointerMove(wheel, { pointerId: 1, clientX: 50, clientY: 0 });
      fireEvent.pointerUp(wheel, { pointerId: 1 });
      view.unmount();
      await act(async () => { await Promise.resolve(); });
      expect(dispatch).toHaveBeenCalledTimes(3);
      expect(onError).not.toHaveBeenCalled();
    },
  );

  it("jogs with arrow keys without bubbling into transport shortcuts or toggling play", async () => {
    const dispatch = vi.fn(async (_action: ControllerAction) => {});
    const shortcut = vi.fn();
    render(<div onKeyDown={shortcut}>
      <ControllerDeck snapshot={snapshot()} track={null} dispatch={dispatch} onError={vi.fn()} />
    </div>);
    const wheel = screen.getByRole("button", { name: /Deck A jog wheel/ });
    fireEvent.keyDown(wheel, { key: "ArrowLeft" });
    fireEvent.keyDown(wheel, { key: "ArrowRight", shiftKey: true });
    fireEvent.click(wheel);

    await waitFor(() => expect(dispatch.mock.calls).toEqual([
      [{ action: "jog", deck: "A", delta: -25 }],
      [{ action: "jog", deck: "A", delta: 250 }],
    ]));
    expect(shortcut).not.toHaveBeenCalled();
  });

  it("locks performance controls during decode, then uses zero-based hot-cue slots and selected pad operations", async () => {
    const dispatch = vi.fn(async (_action: ControllerAction) => {});
    const loading = { ...snapshot(), loading: true };
    const view = render(<ControllerDeck snapshot={loading} track={null} dispatch={dispatch} onError={vi.fn()} />);
    for (const name of [/Deck A jog wheel/, "Deck A pause", "Deck A Transport CUE", "Deck A nudge slower", "Deck A nudge faster", "EXIT", /Deck A Hot Cue 1 /]) {
      expect(screen.getByRole<HTMLButtonElement>("button", { name }).disabled).toBe(true);
    }
    expect(screen.getByRole<HTMLInputElement>("slider", { name: "Deck A tempo" }).disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Deck A pause" }));
    fireEvent.click(screen.getByRole("button", { name: /Deck A Hot Cue 1 / }));
    expect(dispatch).not.toHaveBeenCalled();

    view.rerender(<ControllerDeck snapshot={snapshot()} track={null} dispatch={dispatch} onError={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: /Deck A Hot Cue 1 / }));
    fireEvent.click(screen.getByRole("button", { name: "SET" }));
    fireEvent.click(screen.getByRole("button", { name: /Deck A Hot Cue 8 / }));
    fireEvent.click(screen.getByRole("button", { name: "CLEAR" }));
    fireEvent.click(screen.getByRole("button", { name: /Deck A Hot Cue 1 / }));

    expect(screen.getByRole<HTMLButtonElement>("button", { name: /Deck A Hot Cue 8 / }).disabled).toBe(true);
    await waitFor(() => expect(dispatch.mock.calls).toEqual([
      [{ action: "hot_cue", deck: "A", slot: 0, operation: "trigger_or_set" }],
      [{ action: "hot_cue", deck: "A", slot: 7, operation: "set" }],
      [{ action: "hot_cue", deck: "A", slot: 0, operation: "clear" }],
    ]));
  });
});
