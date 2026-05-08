import type { DeckId } from "@/types/mixer";

interface LoopPadProps {
  deckId: DeckId;
  loopState: { startSec: number; endSec: number | null; active: boolean } | null;
  bpm: number;
  currentPositionSec: number;
  onIn: () => void;
  onOut: () => void;
  onToggle: () => void;
  onShrink: () => void;
  onExtend: () => void;
  onClear: () => void;
}

export function LoopPad({
  deckId,
  loopState,
  bpm,
  currentPositionSec: _currentPositionSec,
  onIn,
  onOut,
  onToggle,
  onShrink,
  onExtend,
  onClear,
}: LoopPadProps) {
  const hasIn = loopState != null;
  const hasFull = loopState != null && loopState.endSec != null;
  const lengthBars =
    hasFull && bpm > 0
      ? ((loopState!.endSec! - loopState!.startSec) * bpm) / 60 / 4
      : null;

  return (
    <div className="loop-pad" data-deck={deckId} aria-label={`Loop controls for Deck ${deckId}`}>
      <div className="loop-pad-row">
        <button
          className="loop-btn"
          data-tone="in"
          data-armed={hasIn && !hasFull || undefined}
          title="Loop In ([)"
          onClick={onIn}
        >
          IN
        </button>
        <button
          className="loop-btn"
          data-tone="out"
          data-armed={hasFull || undefined}
          disabled={!hasIn}
          title="Loop Out (])"
          onClick={onOut}
        >
          OUT
        </button>
        <button
          className="loop-btn"
          data-tone="toggle"
          data-armed={loopState?.active || undefined}
          disabled={!hasFull}
          title={`Loop ${loopState?.active ? "ON" : "OFF"} (\\)`}
          onClick={onToggle}
        >
          {loopState?.active ? "ON" : "OFF"}
        </button>
      </div>
      <div className="loop-pad-row">
        <button
          className="loop-btn"
          disabled={!hasFull || bpm <= 0}
          title="Shrink loop by 1 bar (,)"
          onClick={onShrink}
        >
          −1 bar
        </button>
        <span className="loop-length">
          {lengthBars != null
            ? `${formatBars(lengthBars)}`
            : hasIn
              ? "—"
              : ""}
        </span>
        <button
          className="loop-btn"
          disabled={!hasFull || bpm <= 0}
          title="Extend loop by 1 bar (.)"
          onClick={onExtend}
        >
          +1 bar
        </button>
      </div>
      <div className="loop-pad-row loop-pad-clear">
        <button
          className="loop-btn loop-btn-clear"
          disabled={!hasIn}
          title="Clear loop"
          onClick={onClear}
        >
          Clear
        </button>
      </div>
    </div>
  );
}

function formatBars(bars: number): string {
  if (Math.abs(bars - Math.round(bars)) < 0.05) {
    return `${Math.round(bars)} bar${Math.round(bars) === 1 ? "" : "s"}`;
  }
  return `${bars.toFixed(2)} bars`;
}
